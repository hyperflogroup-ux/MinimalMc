use crate::data_dir;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf, time::{SystemTime, UNIX_EPOCH}};

#[derive(Serialize, Deserialize, Clone)]
pub struct Instance {
    pub id: u64,
    pub name: String,
    pub version: String,
    pub loader: String,
    pub last: String,
    #[serde(default)]
    pub group: String,
    #[serde(default = "default_ram")]
    pub ram: u32,
    #[serde(default)]
    pub mods: u32,
    /// Exact loader version (e.g. a NeoForge build). Empty means "newest stable".
    #[serde(default)]
    pub loader_version: String,
    /// Total time played, in seconds.
    #[serde(default)]
    pub played: u64,
    /// Extra Java arguments added to the launch command.
    #[serde(default)]
    pub jvm_args: String,
    /// Game window size. 0 means the game's default.
    #[serde(default)]
    pub width: u32,
    #[serde(default)]
    pub height: u32,
    /// A server to join straight after launching ("host" or "host:port"). Empty means none.
    #[serde(default)]
    pub server: String,
    /// Back up changed worlds before every launch.
    #[serde(default)]
    pub auto_backup: bool,
}

fn default_ram() -> u32 {
    4
}

fn db(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(data_dir(app)?.join("instances.json"))
}

pub(crate) fn load(app: &tauri::AppHandle) -> Result<Vec<Instance>, String> {
    match fs::read_to_string(db(app)?) {
        Ok(s) => serde_json::from_str(&s).map_err(|e| e.to_string()),
        Err(_) => Ok(vec![]),
    }
}

pub(crate) fn save(app: &tauri::AppHandle, list: &[Instance]) -> Result<(), String> {
    let s = serde_json::to_string_pretty(list).map_err(|e| e.to_string())?;
    fs::write(db(app)?, s).map_err(|e| e.to_string())
}

pub(crate) fn dir_of(app: &tauri::AppHandle, id: u64) -> Result<PathBuf, String> {
    Ok(data_dir(app)?.join("instances").join(id.to_string()))
}

fn now_id() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

#[tauri::command]
pub fn list_instances(app: tauri::AppHandle) -> Result<Vec<Instance>, String> {
    load(&app)
}

#[tauri::command]
pub fn create_instance(app: tauri::AppHandle, name: String, version: String, loader: String, group: String, ram: u32) -> Result<Instance, String> {
    create_with(&app, name, version, loader, String::new(), group, ram)
}

pub(crate) fn create_with(app: &tauri::AppHandle, name: String, version: String, loader: String, loader_version: String, group: String, ram: u32) -> Result<Instance, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("Name is required".into());
    }
    let mut list = load(app)?;
    let inst = Instance { id: now_id(), name, version, loader, last: "Never".into(), group, ram, mods: 0, loader_version, played: 0, jvm_args: String::new(), width: 0, height: 0, server: String::new(), auto_backup: false };
    let dir = dir_of(app, inst.id)?;
    for sub in ["mods", "saves", "resourcepacks", "shaderpacks", "config"] {
        fs::create_dir_all(dir.join(".minecraft").join(sub)).map_err(|e| e.to_string())?;
    }
    list.push(inst.clone());
    save(app, &list)?;
    Ok(inst)
}

#[tauri::command]
pub fn rename_instance(app: tauri::AppHandle, id: u64, name: String) -> Result<(), String> {
    let mut list = load(&app)?;
    let i = list.iter_mut().find(|i| i.id == id).ok_or("Instance not found")?;
    i.name = name.trim().to_string();
    save(&app, &list)
}

pub(crate) fn copy_dir(from: &PathBuf, to: &PathBuf) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for e in fs::read_dir(from)? {
        let e = e?;
        let dest = to.join(e.file_name());
        if e.file_type()?.is_dir() {
            copy_dir(&e.path(), &dest)?;
        } else {
            fs::copy(e.path(), dest)?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn duplicate_instance(app: tauri::AppHandle, id: u64) -> Result<Instance, String> {
    let mut list = load(&app)?;
    let src = list.iter().find(|i| i.id == id).ok_or("Instance not found")?.clone();
    let copy = Instance { id: now_id(), name: format!("{} (copy)", src.name), last: "Never".into(), ..src };
    let from = dir_of(&app, id)?;
    if from.exists() {
        copy_dir(&from, &dir_of(&app, copy.id)?).map_err(|e| e.to_string())?;
    }
    list.push(copy.clone());
    save(&app, &list)?;
    Ok(copy)
}

#[tauri::command]
pub fn delete_instance(app: tauri::AppHandle, id: u64) -> Result<(), String> {
    let mut list = load(&app)?;
    list.retain(|i| i.id != id);
    save(&app, &list)?;
    let dir = dir_of(&app, id)?;
    if dir.exists() {
        fs::remove_dir_all(dir).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ───────────── files inside an instance ─────────────

#[derive(Serialize)]
pub struct InstanceFile {
    pub name: String,
    pub size: u64,
    pub is_dir: bool,
}

/// Only these folders can be listed or edited from the UI.
fn kind_dir(kind: &str) -> Result<&'static str, String> {
    match kind {
        "mods" => Ok("mods"),
        "resourcepacks" => Ok("resourcepacks"),
        "shaderpacks" => Ok("shaderpacks"),
        "saves" => Ok("saves"),
        "screenshots" => Ok("screenshots"),
        _ => Err(format!("Unknown folder: {kind}")),
    }
}

pub(crate) fn dir_size(p: &std::path::Path) -> u64 {
    let Ok(rd) = fs::read_dir(p) else { return 0 };
    rd.filter_map(|e| e.ok())
        .map(|e| match e.metadata() {
            Ok(m) if m.is_dir() => dir_size(&e.path()),
            Ok(m) => m.len(),
            Err(_) => 0,
        })
        .sum()
}

pub(crate) fn recount_mods(app: &tauri::AppHandle, id: u64) -> Result<(), String> {
    let dir = dir_of(app, id)?.join(".minecraft").join("mods");
    let count = fs::read_dir(&dir)
        .map(|r| {
            r.filter_map(|e| e.ok())
                .filter(|e| e.path().extension().map(|x| x == "jar").unwrap_or(false))
                .count()
        })
        .unwrap_or(0) as u32;
    let mut list = load(app)?;
    if let Some(i) = list.iter_mut().find(|i| i.id == id) {
        i.mods = count;
    }
    save(app, &list)
}

#[tauri::command]
pub fn list_instance_files(app: tauri::AppHandle, id: u64, kind: String) -> Result<Vec<InstanceFile>, String> {
    let dir = dir_of(&app, id)?.join(".minecraft").join(kind_dir(&kind)?);
    let Ok(rd) = fs::read_dir(&dir) else { return Ok(vec![]) };
    let mut out: Vec<InstanceFile> = vec![];
    for e in rd.filter_map(|e| e.ok()) {
        let name = e.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name.ends_with(".part") {
            continue;
        }
        let Ok(m) = e.metadata() else { continue };
        let size = if m.is_dir() { dir_size(&e.path()) } else { m.len() };
        out.push(InstanceFile { name, size, is_dir: m.is_dir() });
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(out)
}

#[tauri::command]
pub fn remove_instance_file(app: tauri::AppHandle, id: u64, kind: String, name: String) -> Result<(), String> {
    if kind == "saves" {
        return Err("Worlds can't be deleted from here.".into());
    }
    // Only a bare entry name inside the chosen folder: no separators, no "..".
    if name.is_empty() || name == "." || name == ".." || name.contains(|c: char| matches!(c, '/' | '\\' | ':')) {
        return Err("Invalid file name".into());
    }
    let dir = dir_of(&app, id)?.join(".minecraft").join(kind_dir(&kind)?);
    let target = dir.join(&name);
    let meta = fs::symlink_metadata(&target).map_err(|_| "File not found".to_string())?;
    let res = if meta.is_dir() { fs::remove_dir_all(&target) } else { fs::remove_file(&target) };
    res.map_err(|e| e.to_string())?;
    if kind == "mods" {
        recount_mods(&app, id)?;
    }
    Ok(())
}

#[tauri::command]
pub fn open_instance_folder(app: tauri::AppHandle, id: u64) -> Result<(), String> {
    let dir = dir_of(&app, id)?.join(".minecraft");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let opener = if cfg!(target_os = "windows") {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    std::process::Command::new(opener).arg(&dir).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn update_instance(
    app: tauri::AppHandle,
    id: u64,
    group: Option<String>,
    ram: Option<u32>,
    jvm_args: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    server: Option<String>,
) -> Result<(), String> {
    let mut list = load(&app)?;
    let i = list.iter_mut().find(|i| i.id == id).ok_or("Instance not found")?;
    if let Some(g) = group {
        i.group = g.trim().to_string();
    }
    if let Some(r) = ram {
        i.ram = r.clamp(1, 64);
    }
    if let Some(a) = jvm_args {
        i.jvm_args = a.trim().to_string();
    }
    if let Some(w) = width {
        i.width = w.min(10_000);
    }
    if let Some(h) = height {
        i.height = h.min(10_000);
    }
    if let Some(s) = server {
        i.server = s.trim().to_string();
    }
    save(&app, &list)
}

#[tauri::command]
pub fn set_auto_backup(app: tauri::AppHandle, id: u64, enabled: bool) -> Result<(), String> {
    let mut list = load(&app)?;
    let i = list.iter_mut().find(|i| i.id == id).ok_or("Instance not found")?;
    i.auto_backup = enabled;
    save(&app, &list)
}

/// Turns a mod on or off by renaming "x.jar" <-> "x.jar.disabled". The loader ignores disabled files.
#[tauri::command]
pub fn set_mod_enabled(app: tauri::AppHandle, id: u64, name: String, enabled: bool) -> Result<(), String> {
    if name.is_empty() || name.starts_with('.') || name.contains(|c: char| matches!(c, '/' | '\\' | ':')) {
        return Err("Invalid file name".into());
    }
    let dir = dir_of(&app, id)?.join(".minecraft").join("mods");
    let (from, to) = if enabled {
        let Some(base) = name.strip_suffix(".disabled") else { return Ok(()) };
        (dir.join(&name), dir.join(base))
    } else {
        if name.ends_with(".disabled") {
            return Ok(());
        }
        (dir.join(&name), dir.join(format!("{name}.disabled")))
    };
    if !from.is_file() {
        return Err("File not found".into());
    }
    if to.exists() {
        return Err("A file with that name already exists.".into());
    }
    fs::rename(&from, &to).map_err(|e| e.to_string())?;
    recount_mods(&app, id)
}

/// The end of the last launch's output (the game and the launcher write it here).
#[tauri::command]
pub fn read_launch_log(app: tauri::AppHandle, id: u64) -> Result<String, String> {
    let p = dir_of(&app, id)?.join(".minecraft").join("logs").join("minimalmc-launch.log");
    let bytes = match fs::read(&p) {
        Ok(b) => b,
        Err(_) => return Ok(String::new()),
    };
    let start = bytes.len().saturating_sub(300_000);
    Ok(String::from_utf8_lossy(&bytes[start..]).to_string())
}
