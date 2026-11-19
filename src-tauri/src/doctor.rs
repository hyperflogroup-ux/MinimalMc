//! Crash Doctor.
//!
//! Reads the output of the last launch (and the newest crash report, when it belongs to that launch)
//! and explains in plain words what most likely went wrong, with a one-click fix where one exists.
//! It looks for well-known failure messages; it does not guess beyond them.

use crate::compat::{self, Fix};
use crate::instances::{self, Instance};
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

#[derive(Serialize)]
pub struct Diagnosis {
    /// "error" (probable cause), "warn" (possible cause) or "info"
    pub level: String,
    pub title: String,
    pub detail: String,
    pub fix: Option<Fix>,
}

#[derive(Serialize)]
pub struct DoctorReport {
    pub has_data: bool,
    /// What was read, for display.
    pub source: String,
    pub diagnoses: Vec<Diagnosis>,
    /// The last lines of the launch output, for reference.
    pub tail: String,
}

const NOT_A_CULPRIT: [&str; 7] = ["minecraft", "java", "fabricloader", "fabric-loader", "quilt_loader", "forge", "neoforge"];

// ───────────────────────── Reading the files ─────────────────────────

fn read_tail(path: &Path, max: usize) -> Option<(String, SystemTime)> {
    let bytes = fs::read(path).ok()?;
    let modified = fs::metadata(path).ok()?.modified().ok()?;
    let start = bytes.len().saturating_sub(max);
    Some((String::from_utf8_lossy(&bytes[start..]).to_string(), modified))
}

/// The newest crash-*.txt in a folder: (text, modified time, file name).
fn newest_crash(dir: &Path) -> Option<(String, SystemTime, String)> {
    let mut best: Option<(SystemTime, PathBuf, String)> = None;
    for e in fs::read_dir(dir).ok()?.filter_map(|e| e.ok()) {
        let name = e.file_name().to_string_lossy().to_string();
        if !(name.starts_with("crash-") && name.ends_with(".txt")) {
            continue;
        }
        let Ok(meta) = e.metadata() else { continue };
        let Ok(modified) = meta.modified() else { continue };
        if best.as_ref().map(|b| modified > b.0).unwrap_or(true) {
            best = Some((modified, e.path(), name));
        }
    }
    let (modified, path, name) = best?;
    let (text, _) = read_tail(&path, 200_000)?;
    Some((text, modified, name))
}

fn within(a: SystemTime, b: SystemTime, secs: u64) -> bool {
    let d = match a.duration_since(b) {
        Ok(d) => d,
        Err(e) => e.duration(),
    };
    d.as_secs() <= secs
}

fn last_lines(text: &str, n: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(n);
    lines[start..].join("\n")
}

// ───────────────────────── Small text helpers ─────────────────────────

/// The text inside the first pair of single quotes after `key`.
fn quoted_after(line: &str, key: &str) -> Option<String> {
    let rest = &line[line.find(key)? + key.len()..];
    let a = rest.find('\'')? + 1;
    let b = rest[a..].find('\'')? + a;
    Some(rest[a..b].to_string())
}

/// The id in the last "(...)" of something like "Sodium (sodium)".
fn last_paren(s: &str) -> Option<String> {
    let a = s.rfind('(')?;
    let b = s[a..].find(')')? + a;
    Some(s[a + 1..b].to_string())
}

fn word_after(text: &str, key: &str) -> Option<String> {
    let rest = &text[text.find(key)? + key.len()..];
    let w: String = rest.chars().take_while(|c| !c.is_whitespace()).collect();
    if w.is_empty() {
        None
    } else {
        Some(w)
    }
}

fn number_after(text: &str, key: &str) -> Option<u32> {
    let rest = &text[text.find(key)? + key.len()..];
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

fn diag(level: &str, title: String, detail: &str, fix: Option<Fix>) -> Diagnosis {
    Diagnosis { level: level.into(), title, detail: detail.into(), fix }
}

fn fix(kind: &str, label: String, target: String) -> Option<Fix> {
    Some(Fix { kind: kind.into(), label, target })
}

// ───────────────────────── Finding the causes ─────────────────────────

/// Fabric prints "A potential solution has been determined" followed by lines starting with "- ".
fn fabric_solutions(text: &str) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    let mut lines = text.lines().skip_while(|l| !l.contains("A potential solution has been determined"));
    lines.next();
    for l in lines {
        let t = l.trim();
        if let Some(rest) = t.strip_prefix("- ") {
            out.push(rest.to_string());
        } else if !out.is_empty() {
            break;
        }
    }
    out
}

fn mixin_mods(text: &str) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for l in text.lines() {
        if let Some(id) = word_after(l, "Mixin apply for mod ") {
            if !out.contains(&id) {
                out.push(id);
            }
        }
    }
    out
}

/// The "Suspected Mods" section of a crash report.
fn suspected(text: &str) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    let lines: Vec<&str> = text.lines().collect();
    for (i, l) in lines.iter().enumerate() {
        let Some(rest) = l.trim().strip_prefix("Suspected Mods:") else { continue };
        let rest = rest.trim();
        if rest.eq_ignore_ascii_case("none") {
            continue;
        }
        let mut cand: Vec<&str> = vec![];
        if !rest.is_empty() {
            cand.push(rest);
        }
        for next in lines.iter().skip(i + 1) {
            if next.starts_with(char::is_whitespace) && !next.trim().is_empty() {
                cand.push(next.trim());
            } else {
                break;
            }
        }
        for c in cand {
            for part in c.split(", ") {
                if let Some(id) = last_paren(part) {
                    if !out.contains(&id) {
                        out.push(id);
                    }
                }
            }
        }
    }
    out
}

fn analyze(app: &tauri::AppHandle, inst: &Instance, text: &str) -> Vec<Diagnosis> {
    let mut out: Vec<Diagnosis> = vec![];
    let mut blamed: Vec<String> = vec![];

    // Fabric / Quilt: the loader usually states the fix itself.
    for s in fabric_solutions(text) {
        if let Some(rest) = s.strip_prefix("Install ") {
            let id: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')).collect();
            if !id.is_empty() {
                out.push(diag(
                    "error",
                    format!("A required mod is missing: {id}"),
                    "The mod loader reported this and suggests installing it.",
                    fix("install", format!("Install {id}"), id),
                ));
                continue;
            }
        }
        if s.starts_with("Remove mod") {
            if let Some(id) = last_paren(&s) {
                let file = compat::file_for_mod_id(app, inst.id, &id);
                blamed.push(id.clone());
                out.push(diag(
                    "error",
                    format!("A mod has to be removed: {id}"),
                    "The mod loader says this mod can't be used with the others you have.",
                    file.map(|f| Fix { kind: "disable".into(), label: "Disable this mod".into(), target: f }),
                ));
                continue;
            }
        }
        out.push(diag("warn", format!("The mod loader suggests: {s}"), "Use the Mods tab to make this change.", None));
    }

    // Forge / NeoForge: "Mod ID: 'x', Requested by: 'y', Expected range: '[1,)', Actual version: '[MISSING]'"
    for l in text.lines() {
        if !(l.contains("Mod ID:") && l.contains("Requested by:")) {
            continue;
        }
        let (Some(dep), Some(by)) = (quoted_after(l, "Mod ID:"), quoted_after(l, "Requested by:")) else { continue };
        if NOT_A_CULPRIT.contains(&dep.to_lowercase().as_str()) {
            continue;
        }
        let range = quoted_after(l, "Expected range:").unwrap_or_default();
        let actual = quoted_after(l, "Actual version:").unwrap_or_default();
        if actual.contains("MISSING") {
            out.push(diag(
                "error",
                format!("{by} needs {dep}, which isn't installed"),
                "The mod loader refused to start because a required mod is missing.",
                fix("install", format!("Install {dep}"), dep),
            ));
        } else {
            out.push(diag(
                "error",
                format!("{by} needs {dep} {range}, but you have {actual}"),
                "Update or replace that mod so the versions match.",
                None,
            ));
        }
    }

    // A mod that failed while patching the game, and mods named in the crash report.
    let mut culprits: Vec<(String, &str)> = vec![];
    for id in mixin_mods(text) {
        culprits.push((id, "It failed while changing the game's code. This usually means it doesn't match this Minecraft version or another mod."));
    }
    for id in suspected(text) {
        culprits.push((id, "The crash report names it as the most likely cause."));
    }
    for (id, why) in culprits {
        if NOT_A_CULPRIT.contains(&id.to_lowercase().as_str()) || blamed.contains(&id) {
            continue;
        }
        blamed.push(id.clone());
        let file = compat::file_for_mod_id(app, inst.id, &id);
        out.push(diag(
            "error",
            format!("{id} is the most likely cause"),
            why,
            file.map(|f| Fix { kind: "disable".into(), label: "Disable this mod".into(), target: f }),
        ));
    }

    // Memory.
    if text.contains("OutOfMemoryError") || text.contains("Java heap space") || text.contains("GC overhead limit exceeded") {
        let more = (inst.ram + 2).min(16);
        let f = if inst.ram < 16 { fix("memory", format!("Use {more} GB"), more.to_string()) } else { None };
        out.push(diag(
            "error",
            format!("The game ran out of memory ({} GB)", inst.ram),
            "Give it more memory, or remove some mods.",
            f,
        ));
    }
    if text.contains("Could not reserve enough space") || text.contains("Invalid maximum heap size") {
        let f = if inst.ram > 4 { fix("memory", "Use 4 GB".into(), "4".into()) } else { None };
        out.push(diag(
            "error",
            "Java couldn't set aside that much memory".into(),
            "This computer can't give the game that amount. Choose a smaller number in the instance's Settings.",
            f,
        ));
    }

    // Java arguments typed in the instance settings.
    if text.contains("Unrecognized VM option") || text.contains("Unrecognized option:") {
        let f = if inst.jvm_args.trim().is_empty() { None } else { fix("clear_args", "Clear Java arguments".into(), String::new()) };
        out.push(diag(
            "error",
            "A Java argument isn't valid".into(),
            "Java rejected one of the extra arguments set for this instance.",
            f,
        ));
    }

    // A mod built for a newer Java than the one running.
    if let (Some(need), Some(have)) = (
        number_after(text, "class file version "),
        number_after(text, "only recognizes class file versions up to "),
    ) {
        if need > have && need > 44 {
            out.push(diag(
                "error",
                format!("A mod needs Java {}, but the game is running Java {}", need - 44, have.saturating_sub(44)),
                "MinimalMc picks the Java that Minecraft itself needs. Use an older version of that mod, or a newer Minecraft version.",
                None,
            ));
        }
    }

    // Graphics drivers.
    if text.contains("GLFW error 65542")
        || text.contains("does not appear to support OpenGL")
        || text.contains("Pixel format not accelerated")
        || text.contains("GLFW error 65543")
    {
        out.push(diag(
            "error",
            "Your graphics driver isn't supported".into(),
            "The game couldn't start OpenGL. Update your graphics driver from your computer or graphics card maker's website.",
            None,
        ));
    }

    if out.is_empty() && (text.contains("NoClassDefFoundError") || text.contains("ClassNotFoundException")) {
        out.push(diag(
            "warn",
            "A mod is missing code it needs".into(),
            "This usually means a required mod is missing or the wrong version. Open the Mods tab and check for problems.",
            None,
        ));
    }

    if out.is_empty() {
        if text.contains("Exception") || text.contains("Crash report") || text.contains("ERROR]") {
            out.push(diag(
                "info",
                "Something went wrong, but it isn't a problem I recognise".into(),
                "The last lines of the log are below. Searching for the first error line often helps.",
                None,
            ));
        } else {
            out.push(diag("info", "No signs of a crash in the last launch".into(), "", None));
        }
    }
    out
}

fn run(app: &tauri::AppHandle, id: u64) -> Result<DoctorReport, String> {
    let inst = instances::load(app)?
        .into_iter()
        .find(|i| i.id == id)
        .ok_or("Instance not found")?;
    let game = instances::dir_of(app, id)?.join(".minecraft");
    let log = read_tail(&game.join("logs").join("minimalmc-launch.log"), 300_000);

    let mut text = String::new();
    let mut source = String::new();
    let mut log_text = String::new();
    let mut log_time: Option<SystemTime> = None;
    if let Some((t, m)) = log {
        text.push_str(&t);
        log_text = t;
        log_time = Some(m);
        source = "the last launch log".to_string();
    }
    if let Some((t, m, name)) = newest_crash(&game.join("crash-reports")) {
        // Only a crash report written around that launch belongs to it.
        let belongs = match log_time {
            Some(lt) => within(lt, m, 300),
            None => true,
        };
        if belongs {
            text.push('\n');
            text.push_str(&t);
            source = if source.is_empty() { name } else { format!("the last launch log and {name}") };
            if log_text.is_empty() {
                log_text = t;
            }
        }
    }
    if text.trim().is_empty() {
        return Ok(DoctorReport { has_data: false, source: String::new(), diagnoses: vec![], tail: String::new() });
    }
    let diagnoses = analyze(app, &inst, &text);
    Ok(DoctorReport { has_data: true, source, diagnoses, tail: last_lines(&log_text, 40) })
}

#[tauri::command]
pub async fn diagnose(app: tauri::AppHandle, id: u64) -> Result<DoctorReport, String> {
    tauri::async_runtime::spawn_blocking(move || run(&app, id))
        .await
        .map_err(|e| e.to_string())?
}
