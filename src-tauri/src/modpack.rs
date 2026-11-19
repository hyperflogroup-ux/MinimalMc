//! Installing Modrinth modpacks (.mrpack).
//!
//! A .mrpack is a zip with `modrinth.index.json` (the list of files to download, plus the Minecraft
//! and loader versions) and `overrides/` folders with configs and other files shipped inside the pack.
//! Installing one creates a NEW instance. The files download in the background, and the progress shows
//! on that instance's Play button.

use crate::launch::{self, download_all, Job, LaunchStatus};
use crate::{data_dir, instances};
use serde::Deserialize;
use sha2::{Digest, Sha512};
use std::{
    collections::HashMap,
    fs,
    path::{Component, Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const API: &str = "https://api.modrinth.com/v2";
const USER_AGENT: &str = "MinimalMc/1.0.0";
/// The only hosts the Modrinth modpack format allows files to be downloaded from.
const ALLOWED_HOSTS: [&str; 4] = ["cdn.modrinth.com", "github.com", "raw.githubusercontent.com", "gitlab.com"];

// ───────────────────────── Modrinth version listing ─────────────────────────

#[derive(Deserialize)]
struct PVersion {
    #[serde(default)]
    id: String,
    #[serde(default)]
    version_type: String,
    #[serde(default)]
    files: Vec<PFile>,
}

#[derive(Deserialize)]
struct PFile {
    url: String,
    filename: String,
    #[serde(default)]
    primary: bool,
    hashes: PHashes,
}

#[derive(Deserialize)]
struct PHashes {
    #[serde(default)]
    sha512: Option<String>,
}

// ───────────────────────── modrinth.index.json ─────────────────────────

#[derive(Deserialize)]
struct Index {
    #[serde(default)]
    name: String,
    #[serde(default)]
    files: Vec<IFile>,
    #[serde(default)]
    dependencies: HashMap<String, String>,
}

#[derive(Deserialize)]
struct IFile {
    path: String,
    #[serde(default)]
    hashes: IHashes,
    #[serde(default)]
    env: Option<IEnv>,
    #[serde(default)]
    downloads: Vec<String>,
}

#[derive(Deserialize, Default)]
struct IHashes {
    #[serde(default)]
    sha1: Option<String>,
}

#[derive(Deserialize)]
struct IEnv {
    #[serde(default)]
    client: Option<String>,
}

// ───────────────────────── Helpers ─────────────────────────

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())
}

fn host_ok(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else { return false };
    let host = rest.split(|c: char| c == '/' || c == '?' || c == '#').next().unwrap_or("");
    ALLOWED_HOSTS.contains(&host)
}

/// A path that stays inside the instance folder: relative, no "..", no drive letters.
fn safe_rel(p: &str) -> Option<PathBuf> {
    if p.is_empty() || p.contains(':') {
        return None;
    }
    let path = Path::new(p);
    if path.is_absolute() {
        return None;
    }
    let ok = path.components().all(|c| matches!(c, Component::Normal(_)));
    ok.then(|| path.to_path_buf())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

async fn download_pack(c: &reqwest::Client, f: &PFile, dir: &Path) -> Result<PathBuf, String> {
    if !f.filename.to_lowercase().ends_with(".mrpack") {
        return Err("This project doesn't have a modpack file (.mrpack).".into());
    }
    if !host_ok(&f.url) {
        return Err("The modpack file is hosted somewhere that isn't allowed.".into());
    }
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let bytes = c
        .get(&f.url)
        .send()
        .await
        .map_err(|e| format!("Couldn't download the modpack: {e}"))?
        .error_for_status()
        .map_err(|e| format!("Couldn't download the modpack: {e}"))?
        .bytes()
        .await
        .map_err(|e| format!("Couldn't download the modpack: {e}"))?;
    if let Some(expected) = &f.hashes.sha512 {
        if !hex(&Sha512::digest(&bytes[..])).eq_ignore_ascii_case(expected) {
            return Err("The downloaded modpack failed its checksum and was rejected.".into());
        }
    }
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let path = dir.join(format!("pack-{stamp}.mrpack"));
    fs::write(&path, &bytes).map_err(|e| e.to_string())?;
    Ok(path)
}

fn read_index(pack: &Path) -> Result<Index, String> {
    let f = fs::File::open(pack).map_err(|e| e.to_string())?;
    let mut z = zip::ZipArchive::new(f).map_err(|e| format!("The modpack file is damaged: {e}"))?;
    let mut e = z
        .by_name("modrinth.index.json")
        .map_err(|_| "The modpack file has no modrinth.index.json.".to_string())?;
    let mut text = String::new();
    std::io::Read::read_to_string(&mut e, &mut text).map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| format!("Unreadable modpack index: {e}"))
}

/// Copies the files shipped inside the pack. "client-overrides" is applied after "overrides".
pub(crate) fn extract_overrides(pack: &Path, game_dir: &Path, prefixes: &[&str]) -> Result<(), String> {
    let f = fs::File::open(pack).map_err(|e| e.to_string())?;
    let mut z = zip::ZipArchive::new(f).map_err(|e| e.to_string())?;
    for prefix in prefixes.iter().copied() {
        for i in 0..z.len() {
            let mut e = z.by_index(i).map_err(|e| e.to_string())?;
            if e.is_dir() {
                continue;
            }
            let name = e.name().to_string();
            let Some(rel) = name.strip_prefix(prefix) else { continue };
            let Some(rel) = safe_rel(rel) else { continue };
            let dest = game_dir.join(rel);
            if let Some(p) = dest.parent() {
                fs::create_dir_all(p).map_err(|e| e.to_string())?;
            }
            let mut out = fs::File::create(&dest).map_err(|e| e.to_string())?;
            std::io::copy(&mut e, &mut out).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

// ───────────────────────── The command ─────────────────────────

/// Creates a new instance from a Modrinth modpack and starts downloading its files in the background.
#[tauri::command]
pub async fn install_modpack(app: tauri::AppHandle, project_id: String, version_id: Option<String>) -> Result<instances::Instance, String> {
    let c = client()?;
    let root = data_dir(&app)?;

    // 1. The newest release of the pack.
    let versions: Vec<PVersion> = c
        .get(format!("{API}/project/{project_id}/version"))
        .send()
        .await
        .map_err(|e| format!("Couldn't reach Modrinth: {e}"))?
        .error_for_status()
        .map_err(|e| format!("Modrinth returned an error: {e}"))?
        .json()
        .await
        .map_err(|e| format!("Unexpected response from Modrinth: {e}"))?;
    // The version the player picked, or else the newest release.
    let idx = match &version_id {
        Some(wanted) => versions.iter().position(|v| &v.id == wanted).ok_or("That version of the modpack wasn't found.")?,
        None => versions.iter().position(|v| v.version_type == "release").unwrap_or(0),
    };
    let version = versions.into_iter().nth(idx).ok_or("This modpack has no versions.")?;
    let file = version
        .files
        .iter()
        .find(|f| f.primary)
        .or_else(|| version.files.first())
        .ok_or("This modpack version has no file.")?;

    // 2. Download it and read what it needs.
    let pack_path = download_pack(&c, file, &root.join("modpacks")).await?;
    let index = match read_index(&pack_path) {
        Ok(i) => i,
        Err(e) => {
            let _ = fs::remove_file(&pack_path);
            return Err(e);
        }
    };
    start_pack(app, c, pack_path, index, true).await
}

/// Creates the instance for a Modrinth pack and downloads its files in the background.
/// `delete_after` is for packs this launcher downloaded itself, never for a file the player picked.
async fn start_pack(
    app: tauri::AppHandle,
    c: reqwest::Client,
    pack_path: PathBuf,
    index: Index,
    delete_after: bool,
) -> Result<instances::Instance, String> {
    let Some(mc) = index.dependencies.get("minecraft").cloned() else {
        if delete_after {
            let _ = fs::remove_file(&pack_path);
        }
        return Err("This modpack doesn't say which Minecraft version it is for.".into());
    };
    let dep = |k: &str| index.dependencies.get(k).cloned();
    let (loader, loader_version): (&str, String) = if let Some(v) = dep("fabric-loader") {
        ("Fabric", v)
    } else if let Some(v) = dep("quilt-loader") {
        ("Quilt", v)
    } else if let Some(v) = dep("neoforge") {
        ("NeoForge", v)
    } else if let Some(v) = dep("forge") {
        ("Forge", v)
    } else {
        ("Vanilla", String::new())
    };
    let name = if index.name.trim().is_empty() { "Modpack".to_string() } else { index.name.trim().to_string() };

    let inst = instances::create_with(&app, name, mc, loader.to_string(), loader_version, "Modded".to_string(), 6)?;
    let id = inst.id;
    launch::set(id, |s| {
        *s = LaunchStatus::default();
        s.stage = "Installing modpack".into();
        s.busy = true;
    });
    let a = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = finish(a, c, id, pack_path.clone(), index).await {
            launch::set(id, |s| {
                s.busy = false;
                s.error = Some(e);
            });
        }
        if delete_after {
            let _ = fs::remove_file(&pack_path);
        }
    });
    Ok(inst)
}

/// Imports a Modrinth modpack (.mrpack) from a file on this computer.
pub(crate) async fn import_mrpack(app: tauri::AppHandle, path: String) -> Result<instances::Instance, String> {
    let p = PathBuf::from(path.trim().trim_matches('"'));
    let index = read_index(&p)?;
    start_pack(app, client()?, p, index, false).await
}

async fn finish(app: tauri::AppHandle, c: reqwest::Client, id: u64, pack: PathBuf, index: Index) -> Result<(), String> {
    let game_dir = instances::dir_of(&app, id)?.join(".minecraft");
    let mut jobs: Vec<Job> = vec![];
    for f in &index.files {
        // Files that only the server needs are skipped.
        if f.env.as_ref().and_then(|e| e.client.as_deref()) == Some("unsupported") {
            continue;
        }
        let rel = safe_rel(&f.path).ok_or_else(|| format!("The modpack has an unsafe file path: {}", f.path))?;
        let url = f
            .downloads
            .iter()
            .find(|u| host_ok(u))
            .ok_or_else(|| format!("{} can only be downloaded from a host that isn't allowed.", f.path))?
            .clone();
        jobs.push(Job { url, path: game_dir.join(rel), sha1: f.hashes.sha1.clone(), size: 0 });
    }
    download_all(&c, id, "Installing modpack", jobs).await?;

    launch::set(id, |s| {
        s.stage = "Copying pack files".into();
        s.done = 0;
        s.total = 0;
    });
    extract_overrides(&pack, &game_dir, &["overrides/", "client-overrides/"])?;
    let _ = instances::recount_mods(&app, id);
    crate::storage::dedupe_in_background(app.clone());
    launch::set(id, |s| {
        s.busy = false;
        s.stage = "Idle".into();
    });
    Ok(())
}
