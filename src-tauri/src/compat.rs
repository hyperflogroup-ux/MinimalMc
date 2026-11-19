//! The mod compatibility checker.
//!
//! It reads the details every mod declares inside its own jar (Fabric, Quilt, Forge and NeoForge
//! formats) and compares them with the instance and with each other. Nothing is downloaded, so it
//! works offline. It can only catch problems that mods declare; two mods that crash each other
//! without saying so won't show up here.

use crate::instances;
use serde::Serialize;
use serde_json::Value;
use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    io::{Read, Seek},
    path::Path,
};

#[derive(Serialize, Clone)]
pub struct Fix {
    /// "disable" or "install"
    pub kind: String,
    pub label: String,
    /// The file to disable, or the mod id to install.
    pub target: String,
}

#[derive(Serialize)]
pub struct Finding {
    /// "error" (the game probably won't start) or "warn"
    pub level: String,
    pub file: String,
    pub mod_name: String,
    pub message: String,
    pub fix: Option<Fix>,
}

#[derive(Serialize)]
pub struct CompatReport {
    pub checked: usize,
    pub findings: Vec<Finding>,
}

/// Ids that the loader or the game itself provides.
const BUILTIN: [&str; 13] = [
    "minecraft",
    "java",
    "fabricloader",
    "fabric-loader",
    "fabric_loader",
    "quilt_loader",
    "quilt-loader",
    "forge",
    "neoforge",
    "fml",
    "javafml",
    "lowcodefml",
    "mclanguage",
];

#[derive(Default)]
struct Deps {
    required: Vec<String>,
    /// Version ranges the mod accepts for Minecraft (any one may match).
    mc: Vec<String>,
    /// Mods this one says it can never be used with.
    breaks: Vec<String>,
}

struct Parsed {
    id: String,
    name: String,
    provides: Vec<String>,
    deps: Deps,
    nested: Vec<String>,
}

struct ModInfo {
    file: String,
    id: String,
    name: String,
    /// "fabric", "quilt", "forge" or "neoforge". One jar can support several.
    kinds: Vec<&'static str>,
    /// Other ids this jar satisfies (aliases and bundled mods).
    provides: Vec<String>,
    deps: HashMap<&'static str, Deps>,
}

// ───────────────────────── Reading mod metadata ─────────────────────────

fn read_entry<R: Read + Seek>(z: &mut zip::ZipArchive<R>, name: &str) -> Option<String> {
    let mut e = z.by_name(name).ok()?;
    let mut s = String::new();
    e.read_to_string(&mut s).ok()?;
    Some(s.trim_start_matches('\u{feff}').to_string())
}

fn str_list(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::String(s)) => vec![s.clone()],
        Some(Value::Array(a)) => a.iter().filter_map(|x| x.as_str().map(String::from)).collect(),
        _ => vec![],
    }
}

fn parse_fabric(text: &str) -> Option<Parsed> {
    let v: Value = serde_json::from_str(text).ok()?;
    let id = v.get("id")?.as_str()?.to_string();
    let name = v.get("name").and_then(|x| x.as_str()).unwrap_or(id.as_str()).to_string();
    let provides = str_list(v.get("provides"));
    let mut deps = Deps::default();
    if let Some(o) = v.get("depends").and_then(|d| d.as_object()) {
        for (k, val) in o {
            if k == "minecraft" {
                deps.mc = str_list(Some(val));
            } else {
                deps.required.push(k.clone());
            }
        }
    }
    if let Some(o) = v.get("breaks").and_then(|d| d.as_object()) {
        for (k, val) in o {
            if str_list(Some(val)).iter().any(|r| r.trim() == "*") {
                deps.breaks.push(k.clone());
            }
        }
    }
    let nested: Vec<String> = v
        .get("jars")
        .and_then(|j| j.as_array())
        .map(|a| a.iter().filter_map(|x| x.get("file").and_then(|f| f.as_str()).map(String::from)).collect())
        .unwrap_or_default();
    Some(Parsed { id, name, provides, deps, nested })
}

fn parse_quilt(text: &str) -> Option<Parsed> {
    let v: Value = serde_json::from_str(text).ok()?;
    let q = v.get("quilt_loader")?;
    let id = q.get("id")?.as_str()?.to_string();
    let name = v
        .get("metadata")
        .and_then(|m| m.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or(id.as_str())
        .to_string();
    let provides: Vec<String> = q
        .get("provides")
        .and_then(|p| p.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().or_else(|| x.get("id").and_then(|i| i.as_str())).map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let mut deps = Deps::default();
    if let Some(a) = q.get("depends").and_then(|d| d.as_array()) {
        for d in a {
            let (dep_id, optional, versions) = match d {
                Value::String(s) => (s.clone(), false, vec![]),
                Value::Object(_) => (
                    d.get("id").and_then(|i| i.as_str()).unwrap_or("").to_string(),
                    d.get("optional").and_then(|o| o.as_bool()).unwrap_or(false),
                    str_list(d.get("versions")),
                ),
                _ => continue,
            };
            if dep_id.is_empty() || optional {
                continue;
            }
            if dep_id == "minecraft" {
                deps.mc = versions;
            } else {
                deps.required.push(dep_id);
            }
        }
    }
    let nested: Vec<String> = q
        .get("jars")
        .and_then(|j| j.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
        .unwrap_or_default();
    Some(Parsed { id, name, provides, deps, nested })
}

/// mods.toml (Forge) and neoforge.mods.toml (NeoForge). Returns the kind it looks like.
fn parse_toml_mod(text: &str, neo_file: bool) -> Option<(Parsed, &'static str)> {
    let v: toml::Value = toml::from_str(text).ok()?;
    let mods = v.get("mods")?.as_array()?;
    let first = mods.first()?;
    let id = first.get("modId")?.as_str()?.to_string();
    let name = first.get("displayName").and_then(|n| n.as_str()).unwrap_or(id.as_str()).to_string();
    let provides: Vec<String> = mods
        .iter()
        .skip(1)
        .filter_map(|m| m.get("modId").and_then(|x| x.as_str()).map(String::from))
        .collect();
    let mut deps = Deps::default();
    let mut saw_neoforge = neo_file;
    if let Some(list) = v.get("dependencies").and_then(|d| d.get(id.as_str())).and_then(|d| d.as_array()) {
        for d in list {
            let dep_id = d.get("modId").and_then(|x| x.as_str()).unwrap_or("").to_string();
            if dep_id.is_empty() {
                continue;
            }
            let ty = d.get("type").and_then(|t| t.as_str()).map(|s| s.to_lowercase());
            let required = match ty.as_deref() {
                Some("required") => true,
                Some(_) => false,
                None => d.get("mandatory").and_then(|m| m.as_bool()).unwrap_or(false),
            };
            let incompatible = ty.as_deref() == Some("incompatible");
            let range = d.get("versionRange").and_then(|r| r.as_str()).unwrap_or("").to_string();
            if dep_id == "neoforge" {
                saw_neoforge = true;
            }
            if dep_id == "minecraft" {
                if required {
                    deps.mc.push(range);
                }
                continue;
            }
            if incompatible {
                if range.trim().is_empty() || range.trim() == "*" {
                    deps.breaks.push(dep_id);
                }
                continue;
            }
            if required {
                deps.required.push(dep_id);
            }
        }
    }
    let kind = if saw_neoforge { "neoforge" } else { "forge" };
    Some((Parsed { id, name, provides, deps, nested: vec![] }, kind))
}

/// Ids of the mods bundled inside a Fabric or Quilt jar (Fabric API, for example, carries dozens).
fn nested_ids<R: Read + Seek>(z: &mut zip::ZipArchive<R>, files: &[String]) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for f in files {
        let mut buf: Vec<u8> = vec![];
        {
            let Ok(mut e) = z.by_name(f) else { continue };
            if e.read_to_end(&mut buf).is_err() {
                continue;
            }
        }
        let Ok(mut nz) = zip::ZipArchive::new(std::io::Cursor::new(buf)) else { continue };
        if let Some(t) = read_entry(&mut nz, "fabric.mod.json") {
            if let Some(p) = parse_fabric(&t) {
                out.push(p.id);
                out.extend(p.provides);
            }
        } else if let Some(t) = read_entry(&mut nz, "quilt.mod.json") {
            if let Some(p) = parse_quilt(&t) {
                out.push(p.id);
                out.extend(p.provides);
            }
        }
    }
    out
}

/// Collects what each metadata format in one jar says.
#[derive(Default)]
struct Acc {
    id: String,
    name: String,
    kinds: Vec<&'static str>,
    provides: Vec<String>,
    nested: Vec<String>,
    deps: HashMap<&'static str, Deps>,
}

impl Acc {
    fn take(&mut self, kind: &'static str, p: Parsed) {
        if self.id.is_empty() {
            self.id = p.id;
            self.name = p.name;
        }
        self.kinds.push(kind);
        self.provides.extend(p.provides);
        self.nested.extend(p.nested);
        self.deps.insert(kind, p.deps);
    }
}

/// Ok(None) means the jar is a plain library without mod metadata. Err means it can't be read at all.
fn parse_jar(path: &Path, file: &str) -> Result<Option<ModInfo>, String> {
    let f = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut z = zip::ZipArchive::new(f).map_err(|e| e.to_string())?;
    let mut acc = Acc::default();

    if let Some(p) = read_entry(&mut z, "fabric.mod.json").and_then(|t| parse_fabric(&t)) {
        acc.take("fabric", p);
    }
    if let Some(p) = read_entry(&mut z, "quilt.mod.json").and_then(|t| parse_quilt(&t)) {
        acc.take("quilt", p);
    }
    if let Some((p, kind)) = read_entry(&mut z, "META-INF/neoforge.mods.toml").and_then(|t| parse_toml_mod(&t, true)) {
        acc.take(kind, p);
    }
    // Mods built for both Forge and NeoForge carry both files, so both are read.
    if let Some((p, kind)) = read_entry(&mut z, "META-INF/mods.toml").and_then(|t| parse_toml_mod(&t, false)) {
        if !acc.kinds.contains(&kind) {
            acc.take(kind, p);
        }
    }
    if acc.kinds.is_empty() {
        return Ok(None);
    }
    if !acc.nested.is_empty() {
        let ids = nested_ids(&mut z, &acc.nested);
        acc.provides.extend(ids);
    }
    Ok(Some(ModInfo {
        file: file.to_string(),
        id: acc.id,
        name: acc.name,
        kinds: acc.kinds,
        provides: acc.provides,
        deps: acc.deps,
    }))
}

// ───────────────────────── Version ranges ─────────────────────────
//
// Anything that can't be understood returns None, and the check is skipped. A wrong "error" is
// worse than a missed one.

fn parse_ver(s: &str) -> Option<Vec<u64>> {
    let parts: Option<Vec<u64>> = s.trim().split('.').map(|p| p.parse::<u64>().ok()).collect();
    parts.filter(|p| !p.is_empty())
}

fn cmp_ver(a: &[u64], b: &[u64]) -> Ordering {
    for i in 0..a.len().max(b.len()) {
        let x = a.get(i).copied().unwrap_or(0);
        let y = b.get(i).copied().unwrap_or(0);
        match x.cmp(&y) {
            Ordering::Equal => {}
            o => return o,
        }
    }
    Ordering::Equal
}

fn fabric_token(tok: &str, mc: &[u64]) -> Option<bool> {
    let tok = tok.trim();
    if tok.is_empty() || tok == "*" || tok.eq_ignore_ascii_case("x") {
        return Some(true);
    }
    let (op, rest) = ["<=", ">=", "<", ">", "=", "~", "^"]
        .iter()
        .find_map(|o| tok.strip_prefix(*o).map(|r| (*o, r)))
        .unwrap_or(("", tok));
    let rest = rest.trim();
    if rest.ends_with(".x") || rest.ends_with(".*") || rest.ends_with(".X") {
        let prefix = parse_ver(&rest[..rest.len() - 2])?;
        let mut upper = prefix.clone();
        *upper.last_mut()? += 1;
        return Some(cmp_ver(mc, &prefix) != Ordering::Less && cmp_ver(mc, &upper) == Ordering::Less);
    }
    let v = parse_ver(rest)?;
    Some(match op {
        ">=" => cmp_ver(mc, &v) != Ordering::Less,
        "<=" => cmp_ver(mc, &v) != Ordering::Greater,
        ">" => cmp_ver(mc, &v) == Ordering::Greater,
        "<" => cmp_ver(mc, &v) == Ordering::Less,
        "~" => {
            let upper = if v.len() == 1 { vec![v[0] + 1] } else { vec![v[0], v[1] + 1] };
            cmp_ver(mc, &v) != Ordering::Less && cmp_ver(mc, &upper) == Ordering::Less
        }
        "^" => {
            let upper = vec![v[0] + 1];
            cmp_ver(mc, &v) != Ordering::Less && cmp_ver(mc, &upper) == Ordering::Less
        }
        _ => cmp_ver(mc, &v) == Ordering::Equal,
    })
}

/// Fabric and Quilt ranges: ">=1.21 <1.22", "~1.21", "1.21.x", "1.20.1 || 1.20.2", "*".
fn fabric_range(range: &str, mc: &str) -> Option<bool> {
    let mc = parse_ver(mc)?;
    let mut norm = range.to_string();
    for op in ["<=", ">=", ">", "<", "~", "^", "="] {
        norm = norm.replace(&format!("{op} "), op);
    }
    for alt in norm.split("||") {
        let mut all = true;
        for tok in alt.split_whitespace() {
            match fabric_token(tok, &mc) {
                Some(true) => {}
                Some(false) => all = false,
                None => return None,
            }
        }
        if all {
            return Some(true);
        }
    }
    Some(false)
}

/// Forge and NeoForge (Maven) ranges: "[1.21,1.22)", "[1.21.4]", "[1.20,)".
fn maven_range(range: &str, mc: &str) -> Option<bool> {
    let mc = parse_ver(mc)?;
    let r = range.trim();
    if r.is_empty() || r == "*" {
        return Some(true);
    }
    let first = r.chars().next()?;
    if first != '[' && first != '(' {
        let v = parse_ver(r)?;
        return Some(cmp_ver(&mc, &v) != Ordering::Less);
    }
    if r.len() < 2 || !(r.ends_with(']') || r.ends_with(')')) {
        return None;
    }
    let (lo_inc, hi_inc) = (first == '[', r.ends_with(']'));
    let inner = &r[1..r.len() - 1];
    if inner.contains(|c: char| matches!(c, '[' | ']' | '(' | ')')) {
        return None;
    }
    match inner.split_once(',') {
        None => Some(cmp_ver(&mc, &parse_ver(inner)?) == Ordering::Equal),
        Some((lo, hi)) => {
            if !lo.trim().is_empty() {
                let c = cmp_ver(&mc, &parse_ver(lo)?);
                if c == Ordering::Less || (c == Ordering::Equal && !lo_inc) {
                    return Some(false);
                }
            }
            if !hi.trim().is_empty() {
                let c = cmp_ver(&mc, &parse_ver(hi)?);
                if c == Ordering::Greater || (c == Ordering::Equal && !hi_inc) {
                    return Some(false);
                }
            }
            Some(true)
        }
    }
}

// ───────────────────────── The checks ─────────────────────────

fn accepted(loader: &str) -> &'static [&'static str] {
    match loader {
        "fabric" => &["fabric"],
        "quilt" => &["quilt", "fabric"],
        "neoforge" => &["neoforge", "forge"],
        "forge" => &["forge"],
        _ => &[],
    }
}

fn kind_label(k: &str) -> &'static str {
    match k {
        "fabric" => "Fabric",
        "quilt" => "Quilt",
        "forge" => "Forge",
        "neoforge" => "NeoForge",
        _ => "unknown",
    }
}

fn disable_fix(file: &str, label: &str) -> Option<Fix> {
    Some(Fix { kind: "disable".into(), label: label.into(), target: file.to_string() })
}

fn evaluate(loader: &str, loader_label: &str, mc: &str, mods: &[ModInfo], damaged: &[String]) -> Vec<Finding> {
    let acc = accepted(loader);
    let mut findings: Vec<Finding> = vec![];

    let mut provided: HashSet<String> = HashSet::new();
    for m in mods {
        provided.insert(m.id.to_lowercase());
        for p in &m.provides {
            provided.insert(p.to_lowercase());
        }
    }

    // The same mod installed twice.
    let mut by_id: HashMap<String, Vec<&ModInfo>> = HashMap::new();
    for m in mods {
        by_id.entry(m.id.to_lowercase()).or_default().push(m);
    }
    for list in by_id.values() {
        for later in list.iter().skip(1) {
            findings.push(Finding {
                level: "warn".into(),
                file: later.file.clone(),
                mod_name: later.name.clone(),
                message: format!("{} is installed twice ({} and {}). Keep only one.", later.name, list[0].file, later.file),
                fix: disable_fix(&later.file, "Disable this copy"),
            });
        }
    }

    // Missing mods, grouped so one missing library is reported once.
    let mut missing: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for m in mods {
        let Some(active) = m.kinds.iter().find(|k| acc.contains(*k)).copied() else {
            let labels: Vec<&str> = m.kinds.iter().map(|k| kind_label(k)).collect();
            findings.push(Finding {
                level: "error".into(),
                file: m.file.clone(),
                mod_name: m.name.clone(),
                message: format!("{} is made for {}, but this instance uses {}.", m.name, labels.join(" / "), loader_label),
                fix: disable_fix(&m.file, "Disable this mod"),
            });
            continue;
        };
        if loader == "neoforge" && active == "forge" {
            findings.push(Finding {
                level: "warn".into(),
                file: m.file.clone(),
                mod_name: m.name.clone(),
                message: format!("{} looks like a Forge mod. Most Forge mods won't load on NeoForge.", m.name),
                fix: disable_fix(&m.file, "Disable this mod"),
            });
        }
        let Some(deps) = m.deps.get(active) else { continue };

        // Built for another Minecraft version.
        if !deps.mc.is_empty() {
            let results: Vec<Option<bool>> = deps
                .mc
                .iter()
                .map(|r| if active == "forge" || active == "neoforge" { maven_range(r, mc) } else { fabric_range(r, mc) })
                .collect();
            if results.iter().all(|r| *r == Some(false)) {
                findings.push(Finding {
                    level: "error".into(),
                    file: m.file.clone(),
                    mod_name: m.name.clone(),
                    message: format!("{} is built for Minecraft {}, but this instance is {}.", m.name, deps.mc.join(" or "), mc),
                    fix: disable_fix(&m.file, "Disable this mod"),
                });
                continue;
            }
        }

        for d in &deps.required {
            let key = d.to_lowercase();
            if BUILTIN.contains(&key.as_str()) || provided.contains(&key) {
                continue;
            }
            missing.entry(d.clone()).or_default().push(m.name.clone());
        }

        for b in &deps.breaks {
            if provided.contains(&b.to_lowercase()) {
                findings.push(Finding {
                    level: "error".into(),
                    file: m.file.clone(),
                    mod_name: m.name.clone(),
                    message: format!("{} says it can't be used together with {}.", m.name, b),
                    fix: disable_fix(&m.file, "Disable this mod"),
                });
            }
        }
    }

    for (dep, who) in missing {
        findings.push(Finding {
            level: "error".into(),
            file: String::new(),
            mod_name: dep.clone(),
            message: format!("{} is needed by {}, but it isn't installed.", dep, who.join(", ")),
            fix: Some(Fix { kind: "install".into(), label: format!("Install {dep}"), target: dep.clone() }),
        });
    }

    for f in damaged {
        findings.push(Finding {
            level: "warn".into(),
            file: f.clone(),
            mod_name: f.clone(),
            message: format!("{f} couldn't be read. It may be damaged."),
            fix: disable_fix(f, "Disable this file"),
        });
    }

    findings.sort_by(|a, b| {
        let rank = |l: &str| if l == "error" { 0 } else { 1 };
        rank(&a.level).cmp(&rank(&b.level)).then(a.message.cmp(&b.message))
    });
    findings
}

fn run_check(app: &tauri::AppHandle, id: u64) -> Result<CompatReport, String> {
    let inst = instances::load(app)?
        .into_iter()
        .find(|i| i.id == id)
        .ok_or("Instance not found")?;
    let loader = inst.loader.to_lowercase();
    if accepted(&loader).is_empty() {
        return Ok(CompatReport { checked: 0, findings: vec![] });
    }
    let dir = instances::dir_of(app, id)?.join(".minecraft").join("mods");
    let mut files: Vec<String> = fs::read_dir(&dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().to_string())
                .filter(|n| n.ends_with(".jar"))
                .collect()
        })
        .unwrap_or_default();
    files.sort();

    let mut mods: Vec<ModInfo> = vec![];
    let mut damaged: Vec<String> = vec![];
    for f in &files {
        match parse_jar(&dir.join(f), f) {
            Ok(Some(m)) => mods.push(m),
            Ok(None) => {}
            Err(_) => damaged.push(f.clone()),
        }
    }
    let findings = evaluate(&loader, &inst.loader, &inst.version, &mods, &damaged);
    Ok(CompatReport { checked: files.len(), findings })
}

#[derive(Serialize)]
pub struct ModNode {
    pub file: String,
    pub id: String,
    pub name: String,
    /// Ids of the mods this one needs.
    pub requires: Vec<String>,
    /// Ids this jar provides (its own id, aliases and bundled mods).
    pub provides: Vec<String>,
}

fn graph_blocking(app: &tauri::AppHandle, id: u64) -> Result<Vec<ModNode>, String> {
    let inst = instances::load(app)?
        .into_iter()
        .find(|i| i.id == id)
        .ok_or("Instance not found")?;
    let loader = inst.loader.to_lowercase();
    let acc = accepted(&loader);
    if acc.is_empty() {
        return Ok(vec![]);
    }
    let dir = instances::dir_of(app, id)?.join(".minecraft").join("mods");
    let mut files: Vec<String> = fs::read_dir(&dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().to_string())
                .filter(|n| n.ends_with(".jar"))
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    let mut nodes: Vec<ModNode> = vec![];
    for f in files {
        let Ok(Some(m)) = parse_jar(&dir.join(&f), &f) else { continue };
        let Some(active) = m.kinds.iter().find(|k| acc.contains(*k)).copied() else { continue };
        let requires: Vec<String> = m
            .deps
            .get(active)
            .map(|d| {
                d.required
                    .iter()
                    .filter(|r| !BUILTIN.contains(&r.to_lowercase().as_str()))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        let mut provides = m.provides.clone();
        provides.push(m.id.clone());
        nodes.push(ModNode { file: m.file.clone(), id: m.id.clone(), name: m.name.clone(), requires, provides });
    }
    Ok(nodes)
}

/// Which mod needs which, for Mod Bisect.
#[tauri::command]
pub async fn mod_graph(app: tauri::AppHandle, id: u64) -> Result<Vec<ModNode>, String> {
    tauri::async_runtime::spawn_blocking(move || graph_blocking(&app, id))
        .await
        .map_err(|e| e.to_string())?
}

/// The jar in an instance's mods folder whose own mod id is `mod_id` (used by Crash Doctor).
pub(crate) fn file_for_mod_id(app: &tauri::AppHandle, id: u64, mod_id: &str) -> Option<String> {
    let dir = instances::dir_of(app, id).ok()?.join(".minecraft").join("mods");
    let rd = fs::read_dir(&dir).ok()?;
    let mut files: Vec<String> = rd
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.ends_with(".jar"))
        .collect();
    files.sort();
    let want = mod_id.to_lowercase();
    for f in files {
        if let Ok(Some(m)) = parse_jar(&dir.join(&f), &f) {
            if m.id.to_lowercase() == want {
                return Some(f);
            }
        }
    }
    None
}

#[tauri::command]
pub async fn check_compat(app: tauri::AppHandle, id: u64) -> Result<CompatReport, String> {
    tauri::async_runtime::spawn_blocking(move || run_check(&app, id))
        .await
        .map_err(|e| e.to_string())?
}
