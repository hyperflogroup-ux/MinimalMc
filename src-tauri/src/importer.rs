//! Importing instances from Prism Launcher / PolyMC / MultiMC.
//!
//! Those launchers store an instance as a folder with `instance.cfg`, `mmc-pack.json` (which lists the
//! Minecraft and loader versions) and the game files in `minecraft/`. The files are copied; the
//! original instance is never touched.

use crate::instances;
use serde::Serialize;
use std::{
    fs,
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Serialize)]
pub struct Importable {
    pub name: String,
    pub path: String,
    pub mc: String,
    pub loader: String,
    pub source: String,
}

/// Where these launchers usually keep their instances.
fn roots() -> Vec<(String, PathBuf)> {
    let mut v: Vec<(String, PathBuf)> = vec![];
    if let Some(a) = std::env::var_os("APPDATA") {
        let a = PathBuf::from(a);
        v.push(("Prism Launcher".into(), a.join("PrismLauncher").join("instances")));
        v.push(("PolyMC".into(), a.join("PolyMC").join("instances")));
    }
    if let Some(h) = std::env::var_os("HOME") {
        let h = PathBuf::from(h);
        v.push(("Prism Launcher".into(), h.join(".local").join("share").join("PrismLauncher").join("instances")));
        v.push((
            "Prism Launcher".into(),
            h.join("Library").join("Application Support").join("PrismLauncher").join("instances"),
        ));
    }
    v
}

/// (Minecraft version, loader, loader version) from mmc-pack.json.
fn read_pack(dir: &Path) -> Option<(String, String, String)> {
    let text = fs::read_to_string(dir.join("mmc-pack.json")).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let comps = v.get("components")?.as_array()?;
    let mut mc = String::new();
    let mut loader = "Vanilla".to_string();
    let mut loader_version = String::new();
    for c in comps {
        let uid = c.get("uid").and_then(|u| u.as_str()).unwrap_or("");
        let ver = c.get("version").and_then(|u| u.as_str()).unwrap_or("").to_string();
        match uid {
            "net.minecraft" => mc = ver,
            "net.fabricmc.fabric-loader" => {
                loader = "Fabric".into();
                loader_version = ver;
            }
            "org.quiltmc.quilt-loader" => {
                loader = "Quilt".into();
                loader_version = ver;
            }
            "net.neoforged" => {
                loader = "NeoForge".into();
                loader_version = ver;
            }
            "net.minecraftforge" => {
                loader = "Forge".into();
                loader_version = ver;
            }
            _ => {}
        }
    }
    if mc.is_empty() {
        None
    } else {
        Some((mc, loader, loader_version))
    }
}

/// A "key=value" line from instance.cfg.
fn cfg_value(dir: &Path, key: &str) -> Option<String> {
    let text = fs::read_to_string(dir.join("instance.cfg")).ok()?;
    let prefix = format!("{key}=");
    text.lines().find_map(|l| l.strip_prefix(prefix.as_str()).map(|v| v.trim().to_string()))
}

#[tauri::command]
pub fn list_importable() -> Vec<Importable> {
    let mut out: Vec<Importable> = vec![];
    for (source, root) in roots() {
        let Ok(rd) = fs::read_dir(&root) else { continue };
        for e in rd.filter_map(|e| e.ok()) {
            let dir = e.path();
            if !dir.is_dir() {
                continue;
            }
            let Some((mc, loader, _)) = read_pack(&dir) else { continue };
            let name = cfg_value(&dir, "name")
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| e.file_name().to_string_lossy().to_string());
            out.push(Importable { name, path: dir.display().to_string(), mc, loader, source: source.clone() });
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

fn copy_filtered(from: &Path, to: &Path, skip: &[&str], top: bool) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for e in fs::read_dir(from)? {
        let e = e?;
        let name = e.file_name().to_string_lossy().to_string();
        if top && skip.contains(&name.as_str()) {
            continue;
        }
        let dest = to.join(&name);
        if e.file_type()?.is_dir() {
            copy_filtered(&e.path(), &dest, skip, false)?;
        } else {
            fs::copy(e.path(), dest)?;
        }
    }
    Ok(())
}

fn import_blocking(app: &tauri::AppHandle, path: &str) -> Result<instances::Instance, String> {
    let dir = PathBuf::from(path.trim().trim_matches('"'));
    let Some((mc, loader, loader_version)) = read_pack(&dir) else {
        return Err("That folder isn't a Prism or MultiMC instance (there's no mmc-pack.json in it).".into());
    };
    let name = cfg_value(&dir, "name").filter(|n| !n.trim().is_empty()).unwrap_or_else(|| {
        dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "Imported".to_string())
    });
    let ram = cfg_value(&dir, "MaxMemAlloc")
        .and_then(|v| v.parse::<u32>().ok())
        .map(|mb| ((mb + 512) / 1024).clamp(1, 16))
        .unwrap_or(4);
    let inst = instances::create_with(app, name, mc, loader, loader_version, "Imported".to_string(), ram)?;

    let src = if dir.join("minecraft").is_dir() { dir.join("minecraft") } else { dir.join(".minecraft") };
    if src.is_dir() {
        let dest = instances::dir_of(app, inst.id)?.join(".minecraft");
        copy_filtered(&src, &dest, &["logs", "crash-reports"], true)
            .map_err(|e| format!("Couldn't copy the instance's files: {e}"))?;
    }
    if cfg_value(&dir, "OverrideJavaArgs").as_deref() == Some("true") {
        if let Some(args) = cfg_value(&dir, "JvmArgs").filter(|a| !a.trim().is_empty()) {
            instances::update_instance(app.clone(), inst.id, None, None, Some(args), None, None, None)?;
        }
    }
    let _ = instances::recount_mods(app, inst.id);
    instances::load(app)?
        .into_iter()
        .find(|i| i.id == inst.id)
        .ok_or_else(|| "The import finished, but the new instance wasn't found.".to_string())
}

#[tauri::command]
pub async fn import_instance(app: tauri::AppHandle, path: String) -> Result<instances::Instance, String> {
    tauri::async_runtime::spawn_blocking(move || import_blocking(&app, &path))
        .await
        .map_err(|e| e.to_string())?
}

// ───────────────────────── .zip and .mrpack files ─────────────────────────

/// A path that stays inside the folder it's unpacked to: relative, no "..", no drive letters.
fn safe_rel(p: &str) -> Option<PathBuf> {
    if p.is_empty() || p.contains(':') {
        return None;
    }
    let path = Path::new(p);
    if path.is_absolute() || !path.components().all(|c| matches!(c, Component::Normal(_))) {
        return None;
    }
    Some(path.to_path_buf())
}

enum Kind {
    Mrpack,
    Curse,
    /// A Prism or MultiMC export; holds the folder prefix inside the zip.
    Prism(String),
    Unknown,
}

/// Works out what a zip is from the files inside it, whatever its extension is.
fn sniff(p: &Path) -> Result<Kind, String> {
    let f = fs::File::open(p).map_err(|e| format!("Couldn't open that file: {e}"))?;
    let z = zip::ZipArchive::new(f).map_err(|e| format!("That isn't a valid .zip or .mrpack file: {e}"))?;
    let mut curse = false;
    let mut prism: Option<String> = None;
    for name in z.file_names() {
        if name == "modrinth.index.json" {
            return Ok(Kind::Mrpack);
        }
        if name == "manifest.json" {
            curse = true;
        }
        if let Some(prefix) = name.strip_suffix("mmc-pack.json") {
            if prefix.matches('/').count() <= 1 {
                prism = Some(prefix.to_string());
            }
        }
    }
    Ok(match (prism, curse) {
        (Some(prefix), _) => Kind::Prism(prefix),
        (None, true) => Kind::Curse,
        _ => Kind::Unknown,
    })
}

fn import_prism_zip(app: &tauri::AppHandle, zip_path: &Path, prefix: &str) -> Result<instances::Instance, String> {
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let tmp = crate::data_dir(app)?.join("imports").join(stamp.to_string());
    fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let f = fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut z = zip::ZipArchive::new(f).map_err(|e| e.to_string())?;
    for i in 0..z.len() {
        let mut e = z.by_index(i).map_err(|e| e.to_string())?;
        if e.is_dir() {
            continue;
        }
        let name = e.name().to_string();
        let Some(rel) = name.strip_prefix(prefix) else { continue };
        let Some(rel) = safe_rel(rel) else { continue };
        let dest = tmp.join(rel);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut out = fs::File::create(&dest).map_err(|e| e.to_string())?;
        std::io::copy(&mut e, &mut out).map_err(|e| e.to_string())?;
    }
    let result = import_blocking(app, &tmp.display().to_string());
    let _ = fs::remove_dir_all(&tmp);
    result
}

/// Imports a modpack or instance from a .mrpack or .zip file: a Modrinth pack, a CurseForge pack,
/// or a Prism / MultiMC export. The file itself is never changed or deleted.
#[tauri::command]
pub async fn import_file(app: tauri::AppHandle, path: String) -> Result<instances::Instance, String> {
    let p = PathBuf::from(path.trim().trim_matches('"'));
    let probe = p.clone();
    let kind = tauri::async_runtime::spawn_blocking(move || sniff(&probe))
        .await
        .map_err(|e| e.to_string())??;
    match kind {
        Kind::Mrpack => crate::modpack::import_mrpack(app, path).await,
        Kind::Curse => crate::curseforge::import_cf_zip(app, path).await,
        Kind::Prism(prefix) => tauri::async_runtime::spawn_blocking(move || import_prism_zip(&app, &p, &prefix))
            .await
            .map_err(|e| e.to_string())?,
        Kind::Unknown => Err("I don't recognise this file. It should be a Modrinth modpack (.mrpack), a CurseForge modpack (.zip), or a Prism or MultiMC export (.zip).".into()),
    }
}
