//! Instance snapshots: a saved copy of an instance's mods, configs, packs and options, so a bad
//! change (an update, a new mod, a messed-up config) can be undone in one click.
//!
//! Worlds are not part of a snapshot (they have their own backups). Mods, resource packs and shaders
//! never change once downloaded, so snapshots share those files with the instance instead of copying
//! them, which keeps a snapshot small and fast. Configs are real copies.

use crate::instances;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const KEEP: usize = 20;
const FOLDERS: [&str; 4] = ["mods", "config", "resourcepacks", "shaderpacks"];
const FILES: [&str; 2] = ["options.txt", "servers.dat"];

#[derive(Serialize, Deserialize, Clone)]
pub struct Snapshot {
    /// The folder name, which is the time it was made.
    pub name: String,
    pub label: String,
    /// Milliseconds since 1970.
    pub time: u64,
    pub minecraft: String,
    pub loader: String,
    pub mods: usize,
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn root(app: &tauri::AppHandle, id: u64) -> Result<PathBuf, String> {
    Ok(instances::dir_of(app, id)?.join("snapshots"))
}

fn game(app: &tauri::AppHandle, id: u64) -> Result<PathBuf, String> {
    Ok(instances::dir_of(app, id)?.join(".minecraft"))
}

/// Only digits: a snapshot's folder name is its time.
fn valid(n: &str) -> bool {
    !n.is_empty() && n.chars().all(|c| c.is_ascii_digit())
}

/// Files that are never edited after they are downloaded can be shared instead of copied.
fn shareable(name: &str) -> bool {
    let l = name.to_lowercase();
    l.ends_with(".jar") || l.ends_with(".zip") || l.ends_with(".jar.disabled")
}

fn copy_tree(from: &Path, to: &Path, share: bool) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for e in fs::read_dir(from)? {
        let e = e?;
        let name = e.file_name().to_string_lossy().to_string();
        let dest = to.join(&name);
        if e.file_type()?.is_dir() {
            copy_tree(&e.path(), &dest, share)?;
        } else if share && shareable(&name) {
            if fs::hard_link(e.path(), &dest).is_err() {
                fs::copy(e.path(), &dest)?;
            }
        } else {
            fs::copy(e.path(), &dest)?;
        }
    }
    Ok(())
}

fn count_mods(dir: &Path) -> usize {
    fs::read_dir(dir)
        .map(|rd| rd.filter_map(|e| e.ok()).filter(|e| e.file_name().to_string_lossy().ends_with(".jar")).count())
        .unwrap_or(0)
}

fn create_blocking(app: &tauri::AppHandle, id: u64, label: &str) -> Result<Snapshot, String> {
    let inst = instances::load(app)?
        .into_iter()
        .find(|i| i.id == id)
        .ok_or("Instance not found")?;
    let src = game(app, id)?;
    let time = now_ms();
    let name = time.to_string();
    let dir = root(app, id)?.join(&name);
    let files = dir.join("files");
    fs::create_dir_all(&files).map_err(|e| e.to_string())?;

    for sub in FOLDERS {
        let from = src.join(sub);
        if from.is_dir() {
            copy_tree(&from, &files.join(sub), true).map_err(|e| format!("Couldn't copy {sub}: {e}"))?;
        }
    }
    // The launcher's own offline-skin pack is rebuilt on every launch.
    let _ = fs::remove_dir_all(files.join("resourcepacks").join("minimalmc-skin"));
    for f in FILES {
        let from = src.join(f);
        if from.is_file() {
            fs::copy(&from, files.join(f)).map_err(|e| e.to_string())?;
        }
    }

    let label = if label.trim().is_empty() { "Snapshot".to_string() } else { label.trim().to_string() };
    let snap = Snapshot {
        name: name.clone(),
        label,
        time,
        minecraft: inst.version.clone(),
        loader: inst.loader.clone(),
        mods: count_mods(&files.join("mods")),
    };
    let text = serde_json::to_string_pretty(&snap).map_err(|e| e.to_string())?;
    fs::write(dir.join("meta.json"), text).map_err(|e| e.to_string())?;

    // Keep the newest few, so snapshots don't pile up forever.
    let mut all = list_blocking(app, id)?;
    all.sort_by(|a, b| b.time.cmp(&a.time));
    for old in all.iter().skip(KEEP) {
        let _ = fs::remove_dir_all(root(app, id)?.join(&old.name));
    }
    Ok(snap)
}

fn list_blocking(app: &tauri::AppHandle, id: u64) -> Result<Vec<Snapshot>, String> {
    let dir = root(app, id)?;
    let Ok(rd) = fs::read_dir(&dir) else { return Ok(vec![]) };
    let mut out: Vec<Snapshot> = vec![];
    for e in rd.filter_map(|e| e.ok()) {
        let name = e.file_name().to_string_lossy().to_string();
        if !valid(&name) {
            continue;
        }
        let Ok(text) = fs::read_to_string(e.path().join("meta.json")) else { continue };
        let Ok(snap) = serde_json::from_str::<Snapshot>(&text) else { continue };
        out.push(snap);
    }
    out.sort_by(|a, b| b.time.cmp(&a.time));
    Ok(out)
}

fn restore_blocking(app: &tauri::AppHandle, id: u64, name: &str) -> Result<(), String> {
    if !valid(name) {
        return Err("Invalid snapshot".into());
    }
    let src = root(app, id)?.join(name).join("files");
    if !src.is_dir() {
        return Err("Snapshot not found".into());
    }
    // Save how things are right now, so a restore can itself be undone.
    create_blocking(app, id, "Before restoring a snapshot")?;

    let dest = game(app, id)?;
    for sub in FOLDERS {
        let cur = dest.join(sub);
        if cur.exists() {
            fs::remove_dir_all(&cur).map_err(|e| format!("Couldn't clear {sub}: {e}"))?;
        }
        let from = src.join(sub);
        if from.is_dir() {
            copy_tree(&from, &cur, true).map_err(|e| format!("Couldn't restore {sub}: {e}"))?;
        }
    }
    for f in FILES {
        let from = src.join(f);
        let to = dest.join(f);
        if from.is_file() {
            fs::copy(&from, &to).map_err(|e| e.to_string())?;
        } else {
            let _ = fs::remove_file(&to);
        }
    }
    let _ = instances::recount_mods(app, id);
    Ok(())
}

#[tauri::command]
pub async fn create_snapshot(app: tauri::AppHandle, id: u64, label: String) -> Result<Snapshot, String> {
    tauri::async_runtime::spawn_blocking(move || create_blocking(&app, id, &label))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn list_snapshots(app: tauri::AppHandle, id: u64) -> Result<Vec<Snapshot>, String> {
    list_blocking(&app, id)
}

#[tauri::command]
pub async fn restore_snapshot(app: tauri::AppHandle, id: u64, name: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || restore_blocking(&app, id, &name))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn delete_snapshot(app: tauri::AppHandle, id: u64, name: String) -> Result<(), String> {
    if !valid(&name) {
        return Err("Invalid snapshot".into());
    }
    let dir = root(&app, id)?.join(&name);
    if dir.is_dir() {
        fs::remove_dir_all(dir).map_err(|e| e.to_string())?;
    }
    Ok(())
}
