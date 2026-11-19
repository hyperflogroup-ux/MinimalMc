//! Automatic Java: downloads the same Java runtimes Mojang's own launcher uses.
//!
//! Each runtime ("component", e.g. java-runtime-delta = Java 21) is stored once under
//! <app data>/runtimes/<component> and shared by every instance that needs it.

use crate::launch::{download_all, get_text, java_candidates, java_exe, java_info, set, Job};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Component, Path, PathBuf},
};

const ALL_URLS: [&str; 2] = [
    "https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json",
    "https://piston-meta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json",
];
/// Written only after every file of a runtime has been downloaded. Holds the path of the java executable.
const MARKER: &str = ".minimalmc-complete";

#[derive(Deserialize)]
struct RtEntry {
    manifest: RtRef,
}

#[derive(Deserialize)]
struct RtRef {
    url: String,
}

#[derive(Deserialize)]
struct RtManifest {
    files: HashMap<String, RtFile>,
}

#[derive(Deserialize)]
struct RtFile {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    downloads: Option<RtDownloads>,
    #[serde(default)]
    executable: bool,
    #[serde(default)]
    target: Option<String>,
}

#[derive(Deserialize)]
struct RtDownloads {
    #[serde(default)]
    raw: Option<RtRaw>,
}

#[derive(Deserialize)]
struct RtRaw {
    url: String,
    #[serde(default)]
    sha1: Option<String>,
    #[serde(default)]
    size: u64,
}

fn platform_key() -> Option<&'static str> {
    if cfg!(all(windows, target_arch = "x86_64")) {
        Some("windows-x64")
    } else if cfg!(all(windows, target_arch = "aarch64")) {
        Some("windows-arm64")
    } else if cfg!(all(windows, target_arch = "x86")) {
        Some("windows-x86")
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        Some("mac-os-arm64")
    } else if cfg!(target_os = "macos") {
        Some("mac-os")
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Some("linux")
    } else if cfg!(all(target_os = "linux", target_arch = "x86")) {
        Some("linux-i386")
    } else {
        None
    }
}

fn runtime_dir(root: &Path, component: &str) -> PathBuf {
    root.join("runtimes").join(component)
}

/// A runtime that is already fully downloaded. Needs no internet.
pub(crate) fn cached(root: &Path, component: &str) -> Option<PathBuf> {
    let dir = runtime_dir(root, component);
    let rel = fs::read_to_string(dir.join(MARKER)).ok()?;
    let java = dir.join(rel.trim());
    java.is_file().then_some(java)
}

/// Downloads a runtime (skipping files that are already there) and returns its java executable.
pub(crate) async fn install(client: &reqwest::Client, root: &Path, id: u64, component: &str) -> Result<PathBuf, String> {
    let key = platform_key().ok_or("Automatic Java isn't available on this operating system")?;
    set(id, |s| s.stage = "Finding Java".into());

    let mut all_text: Option<String> = None;
    let mut last_err = String::new();
    for u in ALL_URLS {
        match get_text(client, u).await {
            Ok(t) => {
                all_text = Some(t);
                break;
            }
            Err(e) => last_err = e,
        }
    }
    let all_text = all_text.ok_or(last_err)?;
    let all: HashMap<String, HashMap<String, Vec<RtEntry>>> =
        serde_json::from_str(&all_text).map_err(|e| format!("Unreadable Java list: {e}"))?;
    let entry = all
        .get(key)
        .and_then(|p| p.get(component))
        .and_then(|v| v.first())
        .ok_or_else(|| format!("Mojang has no Java runtime \"{component}\" for this computer"))?;

    let manifest_text = get_text(client, &entry.manifest.url).await?;
    let manifest: RtManifest =
        serde_json::from_str(&manifest_text).map_err(|e| format!("Unreadable Java file list: {e}"))?;

    let exe_suffix = format!("bin/{}", java_exe());
    let java_rel = manifest
        .files
        .keys()
        .filter(|k| k.ends_with(&exe_suffix))
        .min_by_key(|k| k.len())
        .cloned()
        .ok_or("This Java runtime has no java executable")?;

    let dest = runtime_dir(root, component);
    let _ = fs::remove_file(dest.join(MARKER));
    let mut jobs: Vec<Job> = vec![];
    let mut exec: Vec<PathBuf> = vec![];
    let mut links: Vec<(PathBuf, String)> = vec![];
    for (rel, f) in &manifest.files {
        if !Path::new(rel).components().all(|c| matches!(c, Component::Normal(_))) {
            return Err("Unsafe path in the Java download".into());
        }
        let path = dest.join(rel);
        match f.kind.as_str() {
            "directory" => {
                fs::create_dir_all(&path).map_err(|e| e.to_string())?;
            }
            "file" => {
                let Some(raw) = f.downloads.as_ref().and_then(|d| d.raw.as_ref()) else { continue };
                jobs.push(Job { url: raw.url.clone(), path: path.clone(), sha1: raw.sha1.clone(), size: raw.size });
                if f.executable {
                    exec.push(path);
                }
            }
            "link" => {
                if let Some(t) = &f.target {
                    links.push((path, t.clone()));
                }
            }
            _ => {}
        }
    }
    download_all(client, id, "Downloading Java", jobs).await?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for p in &exec {
            let _ = fs::set_permissions(p, fs::Permissions::from_mode(0o755));
        }
        for (p, target) in &links {
            if let Some(parent) = p.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::remove_file(p);
            let _ = std::os::unix::fs::symlink(target, p);
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (&exec, &links);
    }

    fs::write(dest.join(MARKER), &java_rel).map_err(|e| e.to_string())?;
    Ok(dest.join(java_rel))
}

// ───────────────────────── For the Settings page ─────────────────────────

#[derive(Serialize)]
pub struct JavaEntry {
    pub path: String,
    pub version: String,
    pub major: u32,
    /// Downloaded and kept up to date by MinimalMc (as opposed to installed by you).
    pub managed: bool,
}

#[tauri::command]
pub async fn list_java(app: tauri::AppHandle) -> Result<Vec<JavaEntry>, String> {
    let root = crate::data_dir(&app)?;
    let mut out: Vec<JavaEntry> = vec![];
    if let Ok(rd) = fs::read_dir(root.join("runtimes")) {
        for e in rd.filter_map(|e| e.ok()) {
            let comp = e.file_name().to_string_lossy().to_string();
            if let Some(p) = cached(&root, &comp) {
                if let Some((major, version)) = java_info(&p).await {
                    out.push(JavaEntry { path: p.display().to_string(), version, major, managed: true });
                }
            }
        }
    }
    for p in java_candidates() {
        if let Some((major, version)) = java_info(&p).await {
            out.push(JavaEntry { path: p.display().to_string(), version, major, managed: false });
        }
    }
    Ok(out)
}
