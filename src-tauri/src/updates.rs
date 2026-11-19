//! Updating installed mods through Modrinth.
//!
//! Every enabled jar is identified by its checksum, so no guessing from file names. Before a mod is
//! replaced, the old file is moved into a backup folder, which makes the last update undoable.

use crate::instances::{self, Instance};
use serde::{Deserialize, Serialize};
use sha1::Sha1;
use sha2::{Digest, Sha512};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const API: &str = "https://api.modrinth.com/v2";
const USER_AGENT: &str = "MinimalMc/1.0.0";
/// Files are only downloaded from Modrinth's own file host.
const CDN: &str = "https://cdn.modrinth.com/";

#[derive(Deserialize)]
struct RVersion {
    #[serde(default)]
    version_number: String,
    #[serde(default)]
    files: Vec<RFile>,
}

#[derive(Deserialize, Clone)]
struct RFile {
    url: String,
    filename: String,
    #[serde(default)]
    primary: bool,
    hashes: RHashes,
}

#[derive(Deserialize, Clone)]
struct RHashes {
    #[serde(default)]
    sha1: Option<String>,
    #[serde(default)]
    sha512: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct ModUpdate {
    /// The installed file.
    pub file: String,
    pub new_file: String,
    pub new_version: String,
}

#[derive(Serialize)]
pub struct UpdateReport {
    pub checked: usize,
    pub updates: Vec<ModUpdate>,
    /// How many mods the last update changed (0 means there is nothing to undo).
    pub can_undo: usize,
}

#[derive(Serialize)]
pub struct UpdateResult {
    pub updated: Vec<String>,
    pub failed: Vec<String>,
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn safe(n: &str) -> bool {
    !n.is_empty() && n != "." && n != ".." && !n.contains(|c: char| matches!(c, '/' | '\\' | ':'))
}

fn mods_dir(app: &tauri::AppHandle, id: u64) -> Result<PathBuf, String> {
    Ok(instances::dir_of(app, id)?.join(".minecraft").join("mods"))
}

fn backups_root(app: &tauri::AppHandle, id: u64) -> Result<PathBuf, String> {
    Ok(instances::dir_of(app, id)?.join("mods-backup"))
}

fn newest_backup(root: &Path) -> Option<PathBuf> {
    let mut best: Option<(u64, PathBuf)> = None;
    for e in fs::read_dir(root).ok()?.filter_map(|e| e.ok()) {
        let Ok(stamp) = e.file_name().to_string_lossy().parse::<u64>() else { continue };
        if best.as_ref().map(|b| stamp > b.0).unwrap_or(true) {
            best = Some((stamp, e.path()));
        }
    }
    best.map(|b| b.1)
}

/// (old file, new file) pairs written when an update was applied.
fn read_manifest(dir: &Path) -> Vec<(String, String)> {
    let Ok(text) = fs::read_to_string(dir.join("manifest.json")) else { return vec![] };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { return vec![] };
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| Some((x.get("old")?.as_str()?.to_string(), x.get("new")?.as_str()?.to_string())))
                .collect()
        })
        .unwrap_or_default()
}

/// sha1 of every enabled jar: hash -> file name.
fn hash_jars(dir: &Path) -> HashMap<String, String> {
    let mut out: HashMap<String, String> = HashMap::new();
    let Ok(rd) = fs::read_dir(dir) else { return out };
    for e in rd.filter_map(|e| e.ok()) {
        let name = e.file_name().to_string_lossy().to_string();
        if !name.ends_with(".jar") {
            continue;
        }
        let Ok(bytes) = fs::read(e.path()) else { continue };
        out.insert(hex(&Sha1::digest(&bytes[..])), name);
    }
    out
}

fn load_instance(app: &tauri::AppHandle, id: u64) -> Result<Instance, String> {
    instances::load(app)?
        .into_iter()
        .find(|i| i.id == id)
        .ok_or_else(|| "Instance not found".to_string())
}

async fn find_updates(
    c: &reqwest::Client,
    inst: &Instance,
    hashes: &HashMap<String, String>,
    dir: &Path,
) -> Result<Vec<(ModUpdate, RFile)>, String> {
    if hashes.is_empty() {
        return Ok(vec![]);
    }
    let l = inst.loader.to_lowercase();
    let loaders: Vec<String> = if l == "quilt" { vec!["quilt".into(), "fabric".into()] } else { vec![l] };
    let body = serde_json::json!({
        "hashes": hashes.keys().collect::<Vec<_>>(),
        "algorithm": "sha1",
        "loaders": loaders,
        "game_versions": [inst.version.clone()],
    });
    let found: HashMap<String, RVersion> = c
        .post(format!("{API}/version_files/update"))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Couldn't reach Modrinth: {e}"))?
        .error_for_status()
        .map_err(|e| format!("Modrinth returned an error: {e}"))?
        .json()
        .await
        .map_err(|e| format!("Unexpected response from Modrinth: {e}"))?;

    let mut out: Vec<(ModUpdate, RFile)> = vec![];
    for (hash, ver) in found {
        let Some(old) = hashes.get(&hash) else { continue };
        let Some(file) = ver.files.iter().find(|f| f.primary).or_else(|| ver.files.first()) else { continue };
        // Already the newest version.
        if file.hashes.sha1.as_deref().map(|s| s.eq_ignore_ascii_case(&hash)).unwrap_or(false) {
            continue;
        }
        if !safe(&file.filename) || dir.join(&file.filename).exists() || !file.url.starts_with(CDN) {
            continue;
        }
        out.push((
            ModUpdate { file: old.clone(), new_file: file.filename.clone(), new_version: ver.version_number.clone() },
            file.clone(),
        ));
    }
    out.sort_by(|a, b| a.0.file.cmp(&b.0.file));
    Ok(out)
}

async fn download(c: &reqwest::Client, f: &RFile, dir: &Path) -> Result<(), String> {
    let bytes = c
        .get(&f.url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .bytes()
        .await
        .map_err(|e| e.to_string())?;
    if let Some(expected) = &f.hashes.sha512 {
        if !hex(&Sha512::digest(&bytes[..])).eq_ignore_ascii_case(expected) {
            return Err("checksum mismatch".into());
        }
    }
    let tmp = dir.join(format!("{}.part", f.filename));
    fs::write(&tmp, &bytes).map_err(|e| e.to_string())?;
    fs::rename(&tmp, dir.join(&f.filename)).map_err(|e| e.to_string())
}

fn undo_count(app: &tauri::AppHandle, id: u64) -> usize {
    backups_root(app, id)
        .ok()
        .and_then(|r| newest_backup(&r))
        .map(|b| read_manifest(&b).len())
        .unwrap_or(0)
}

#[tauri::command]
pub async fn check_mod_updates(app: tauri::AppHandle, id: u64) -> Result<UpdateReport, String> {
    let inst = load_instance(&app, id)?;
    if inst.loader.eq_ignore_ascii_case("vanilla") {
        return Ok(UpdateReport { checked: 0, updates: vec![], can_undo: 0 });
    }
    let dir = mods_dir(&app, id)?;
    let d = dir.clone();
    let hashes = tauri::async_runtime::spawn_blocking(move || hash_jars(&d))
        .await
        .map_err(|e| e.to_string())?;
    let c = client()?;
    let ups = find_updates(&c, &inst, &hashes, &dir).await?;
    Ok(UpdateReport {
        checked: hashes.len(),
        updates: ups.into_iter().map(|(u, _)| u).collect(),
        can_undo: undo_count(&app, id),
    })
}

/// Updates the given files (all of them when the list is empty). The addresses to download from are
/// looked up again here, never taken from the interface.
#[tauri::command]
pub async fn update_mods(app: tauri::AppHandle, id: u64, files: Vec<String>) -> Result<UpdateResult, String> {
    let inst = load_instance(&app, id)?;
    let dir = mods_dir(&app, id)?;
    let d = dir.clone();
    let hashes = tauri::async_runtime::spawn_blocking(move || hash_jars(&d))
        .await
        .map_err(|e| e.to_string())?;
    let c = client()?;
    let ups = find_updates(&c, &inst, &hashes, &dir).await?;
    let chosen: Vec<(ModUpdate, RFile)> = ups
        .into_iter()
        .filter(|(u, _)| files.is_empty() || files.contains(&u.file))
        .collect();
    if chosen.is_empty() {
        return Ok(UpdateResult { updated: vec![], failed: vec![] });
    }

    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
    let backup = backups_root(&app, id)?.join(stamp.to_string());
    fs::create_dir_all(&backup).map_err(|e| e.to_string())?;

    let mut updated: Vec<String> = vec![];
    let mut failed: Vec<String> = vec![];
    let mut pairs: Vec<serde_json::Value> = vec![];
    for (u, f) in chosen {
        if download(&c, &f, &dir).await.is_err() {
            failed.push(u.file.clone());
            continue;
        }
        if fs::rename(dir.join(&u.file), backup.join(&u.file)).is_ok() {
            pairs.push(serde_json::json!({ "old": u.file, "new": u.new_file }));
            updated.push(u.new_file.clone());
        } else {
            // Couldn't set the old file aside, so don't leave two versions behind.
            let _ = fs::remove_file(dir.join(&u.new_file));
            failed.push(u.file.clone());
        }
    }
    if pairs.is_empty() {
        let _ = fs::remove_dir_all(&backup);
    } else {
        let text = serde_json::to_string_pretty(&pairs).map_err(|e| e.to_string())?;
        fs::write(backup.join("manifest.json"), text).map_err(|e| e.to_string())?;
    }
    let _ = instances::recount_mods(&app, id);
    Ok(UpdateResult { updated, failed })
}

/// Puts the previous versions back and removes the ones the last update added.
#[tauri::command]
pub fn rollback_mod_updates(app: tauri::AppHandle, id: u64) -> Result<usize, String> {
    let root = backups_root(&app, id)?;
    let Some(b) = newest_backup(&root) else { return Err("There's no update to undo.".into()) };
    let dir = mods_dir(&app, id)?;
    let mut restored = 0;
    for (old, new) in read_manifest(&b) {
        if !safe(&old) || !safe(&new) {
            continue;
        }
        let _ = fs::remove_file(dir.join(&new));
        if fs::rename(b.join(&old), dir.join(&old)).is_ok() {
            restored += 1;
        }
    }
    let _ = fs::remove_dir_all(&b);
    let _ = instances::recount_mods(&app, id);
    Ok(restored)
}
