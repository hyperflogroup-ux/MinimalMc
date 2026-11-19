//! Exporting an instance for Modrinth (.mrpack), plus a few small helpers for opening folders and links.

use crate::instances;
use serde::{Deserialize, Serialize};
use sha1::Sha1;
use sha2::{Digest, Sha512};
use std::{
    collections::HashMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

fn home() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from)
}

fn exports_dir() -> Result<PathBuf, String> {
    let dir = home().ok_or("Couldn't find your user folder")?.join("Documents").join("MinimalMc exports");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn opts(large: bool) -> SimpleFileOptions {
    SimpleFileOptions::default().compression_method(CompressionMethod::Deflated).large_file(large)
}

fn add_dir(z: &mut ZipWriter<fs::File>, dir: &Path, prefix: &str, files: &mut usize) -> Result<(), String> {
    for e in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let e = e.map_err(|e| e.to_string())?;
        let name = e.file_name().to_string_lossy().to_string();
        let rel = format!("{prefix}/{name}");
        // The launcher's own offline-skin pack is rebuilt on every launch, so it isn't exported.
        if rel == "files/resourcepacks/minimalmc-skin" {
            continue;
        }
        let ft = e.file_type().map_err(|e| e.to_string())?;
        if ft.is_dir() {
            add_dir(z, &e.path(), &rel, files)?;
        } else if ft.is_file() {
            let len = e.metadata().map(|m| m.len()).unwrap_or(0);
            z.start_file(rel.as_str(), opts(len > 3_000_000_000)).map_err(|e| e.to_string())?;
            let mut f = fs::File::open(e.path()).map_err(|e| e.to_string())?;
            std::io::copy(&mut f, z).map_err(|e| e.to_string())?;
            *files += 1;
        }
    }
    Ok(())
}

/// Opens a web address in the normal browser. Only https addresses are allowed.
#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    let ok = url.starts_with("https://")
        && url.len() < 2000
        && !url.chars().any(|c| c.is_whitespace() || matches!(c, '"' | '<' | '>' | '^' | '|' | '`'));
    if !ok {
        return Err("That isn't a link that can be opened.".into());
    }
    let opener = if cfg!(target_os = "windows") {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    Command::new(opener).arg(&url).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

/// Opens the folder the exports are saved in.
#[tauri::command]
pub fn open_exports_folder() -> Result<(), String> {
    let dir = exports_dir()?;
    let opener = if cfg!(target_os = "windows") {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    Command::new(opener).arg(&dir).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

// ───────────────────────── Export for Modrinth (.mrpack) ─────────────────────────
//
// A Modrinth modpack lists each mod with the address Modrinth hosts it at, so the file stays small.
// Every mod, resource pack and shader is looked up by its checksum. Ones Modrinth knows are listed;
// the rest are put inside the file itself, so nothing is lost.

#[derive(Serialize)]
pub struct MrpackResult {
    pub path: String,
    pub size: u64,
    /// Files listed with their Modrinth address.
    pub linked: usize,
    /// Files that had to be included inside the pack.
    pub bundled: usize,
}

struct Entry {
    rel: String,
    abs: PathBuf,
    sha1: String,
    sha512: String,
    size: u64,
}

#[derive(Deserialize)]
struct VHashes {
    #[serde(default)]
    sha1: Option<String>,
    #[serde(default)]
    sha512: Option<String>,
}

#[derive(Deserialize)]
struct VFile {
    url: String,
    #[serde(default)]
    size: u64,
    hashes: VHashes,
}

#[derive(Deserialize)]
struct VVersion {
    #[serde(default)]
    files: Vec<VFile>,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn collect_entries(game: &Path) -> Vec<Entry> {
    let mut out: Vec<Entry> = vec![];
    for sub in ["mods", "resourcepacks", "shaderpacks"] {
        let Ok(rd) = fs::read_dir(game.join(sub)) else { continue };
        for e in rd.filter_map(|e| e.ok()) {
            let name = e.file_name().to_string_lossy().to_string();
            let lower = name.to_lowercase();
            if !(lower.ends_with(".jar") || lower.ends_with(".zip")) {
                continue;
            }
            let Ok(meta) = e.metadata() else { continue };
            if !meta.is_file() {
                continue;
            }
            let Ok(bytes) = fs::read(e.path()) else { continue };
            out.push(Entry {
                rel: format!("{sub}/{name}"),
                abs: e.path(),
                sha1: hex(&Sha1::digest(&bytes[..])),
                sha512: hex(&Sha512::digest(&bytes[..])),
                size: bytes.len() as u64,
            });
        }
    }
    out
}

/// The loader's name in the Modrinth format and its exact version.
fn loader_dep(root: &Path, inst: &instances::Instance) -> Result<Option<(&'static str, String)>, String> {
    let key: &'static str = match inst.loader.to_lowercase().as_str() {
        "fabric" => "fabric-loader",
        "quilt" => "quilt-loader",
        "forge" => "forge",
        "neoforge" => "neoforge",
        _ => return Ok(None),
    };
    if !inst.loader_version.is_empty() {
        return Ok(Some((key, inst.loader_version.clone())));
    }
    // Fabric and Quilt record which loader they picked in the name of the saved profile.
    let kind = if key == "fabric-loader" { "fabric" } else if key == "quilt-loader" { "quilt" } else { "" };
    if !kind.is_empty() {
        let file = root.join("versions").join(format!("{kind}-{}.json", inst.version));
        if let Ok(text) = fs::read_to_string(&file) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(id) = v.get("id").and_then(|x| x.as_str()) {
                    let prefix = format!("{key}-");
                    let suffix = format!("-{}", inst.version);
                    if let Some(lv) = id.strip_prefix(prefix.as_str()).and_then(|r| r.strip_suffix(suffix.as_str())) {
                        return Ok(Some((key, lv.to_string())));
                    }
                }
            }
        }
    }
    Err("Launch this instance once first, so MinimalMc knows which loader version it uses.".into())
}

fn write_mrpack(
    inst: &instances::Instance,
    dep: Option<(&'static str, String)>,
    game: &Path,
    entries: Vec<Entry>,
    known: HashMap<String, VFile>,
) -> Result<MrpackResult, String> {
    let out_dir = exports_dir()?;
    let safe: String = inst
        .name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let base = if safe.trim().is_empty() { "instance".to_string() } else { safe.trim().to_string() };
    let mut path = out_dir.join(format!("{base}.mrpack"));
    let mut n = 2;
    while path.exists() {
        path = out_dir.join(format!("{base} ({n}).mrpack"));
        n += 1;
    }

    let mut listed: Vec<serde_json::Value> = vec![];
    let mut bundled: Vec<&Entry> = vec![];
    for e in &entries {
        match known.get(&e.sha1) {
            Some(f) => {
                let size = if f.size > 0 { f.size } else { e.size };
                let sha512 = f.hashes.sha512.clone().unwrap_or_else(|| e.sha512.clone());
                listed.push(serde_json::json!({
                    "path": e.rel,
                    "hashes": { "sha1": e.sha1, "sha512": sha512 },
                    "env": { "client": "required", "server": "required" },
                    "downloads": [f.url],
                    "fileSize": size,
                }));
            }
            None => bundled.push(e),
        }
    }

    let mut deps = serde_json::Map::new();
    deps.insert("minecraft".to_string(), serde_json::Value::from(inst.version.clone()));
    if let Some((k, v)) = &dep {
        deps.insert((*k).to_string(), serde_json::Value::from(v.clone()));
    }
    let index = serde_json::json!({
        "formatVersion": 1,
        "game": "minecraft",
        "versionId": "1.0.0",
        "name": inst.name,
        "summary": "Exported from MinimalMc",
        "files": listed,
        "dependencies": deps,
    });

    let file = fs::File::create(&path).map_err(|e| format!("Couldn't create the file: {e}"))?;
    let mut z = ZipWriter::new(file);
    z.start_file("modrinth.index.json", opts(false)).map_err(|e| e.to_string())?;
    z.write_all(serde_json::to_string_pretty(&index).map_err(|e| e.to_string())?.as_bytes())
        .map_err(|e| e.to_string())?;
    for e in &bundled {
        z.start_file(format!("overrides/{}", e.rel), opts(e.size > 3_000_000_000)).map_err(|e| e.to_string())?;
        let mut f = fs::File::open(&e.abs).map_err(|e| e.to_string())?;
        std::io::copy(&mut f, &mut z).map_err(|e| e.to_string())?;
    }
    let config = game.join("config");
    if config.is_dir() {
        let mut count = 0usize;
        add_dir(&mut z, &config, "overrides/config", &mut count)?;
    }
    z.finish().map_err(|e| e.to_string())?;
    let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    Ok(MrpackResult { path: path.display().to_string(), size, linked: entries.len() - bundled.len(), bundled: bundled.len() })
}

/// Saves an instance as a Modrinth modpack (.mrpack). Needs internet, to look the mods up.
#[tauri::command]
pub async fn export_mrpack(app: tauri::AppHandle, id: u64) -> Result<MrpackResult, String> {
    let inst = instances::load(&app)?
        .into_iter()
        .find(|i| i.id == id)
        .ok_or("Instance not found")?;
    let root = crate::data_dir(&app)?;
    let dep = loader_dep(&root, &inst)?;
    let game = instances::dir_of(&app, id)?.join(".minecraft");

    let g = game.clone();
    let entries = tauri::async_runtime::spawn_blocking(move || collect_entries(&g))
        .await
        .map_err(|e| e.to_string())?;

    // Which of these files does Modrinth know?
    let mut known: HashMap<String, VFile> = HashMap::new();
    if !entries.is_empty() {
        let hashes: Vec<&String> = entries.iter().map(|e| &e.sha1).collect();
        let body = serde_json::json!({ "hashes": hashes, "algorithm": "sha1" });
        let client = reqwest::Client::builder()
            .user_agent("MinimalMc/1.0.0")
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| e.to_string())?;
        let found: HashMap<String, VVersion> = client
            .post("https://api.modrinth.com/v2/version_files")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Couldn't reach Modrinth to look up your mods: {e}"))?
            .error_for_status()
            .map_err(|e| format!("Modrinth returned an error: {e}"))?
            .json()
            .await
            .map_err(|e| format!("Unexpected response from Modrinth: {e}"))?;
        for (hash, ver) in found {
            let pick = ver
                .files
                .into_iter()
                .find(|f| f.hashes.sha1.as_deref().map(|s| s.eq_ignore_ascii_case(&hash)).unwrap_or(false));
            if let Some(f) = pick {
                known.insert(hash, f);
            }
        }
    }

    tauri::async_runtime::spawn_blocking(move || write_mrpack(&inst, dep, &game, entries, known))
        .await
        .map_err(|e| e.to_string())?
}
