//! World backups: plain folder copies kept inside the instance, so restoring is just copying back.
//! Restoring never throws anything away: the world being replaced is saved as a backup first.

use crate::instances;
use serde::Serialize;
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Serialize)]
pub struct Backup {
    /// The folder name: "<world>__<time>".
    pub name: String,
    pub world: String,
    /// Milliseconds since 1970.
    pub time: u64,
    pub size: u64,
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// A single folder name: no separators, no "..".
fn valid(n: &str) -> bool {
    !n.is_empty() && n != "." && n != ".." && !n.contains(|c: char| matches!(c, '/' | '\\' | ':'))
}

fn saves(app: &tauri::AppHandle, id: u64) -> Result<PathBuf, String> {
    Ok(instances::dir_of(app, id)?.join(".minecraft").join("saves"))
}

fn root(app: &tauri::AppHandle, id: u64) -> Result<PathBuf, String> {
    Ok(instances::dir_of(app, id)?.join("backups"))
}

fn backup_blocking(app: &tauri::AppHandle, id: u64, world: &str) -> Result<Backup, String> {
    if !valid(world) {
        return Err("Invalid world name".into());
    }
    let from = saves(app, id)?.join(world);
    if !from.is_dir() {
        return Err("World not found".into());
    }
    let time = now_ms();
    let name = format!("{world}__{time}");
    let to = root(app, id)?.join(&name);
    instances::copy_dir(&from, &to).map_err(|e| format!("Couldn't copy the world: {e}"))?;
    Ok(Backup { size: instances::dir_size(&to), name, world: world.to_string(), time })
}

#[tauri::command]
pub async fn backup_world(app: tauri::AppHandle, id: u64, world: String) -> Result<Backup, String> {
    tauri::async_runtime::spawn_blocking(move || backup_blocking(&app, id, &world))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn list_backups(app: tauri::AppHandle, id: u64) -> Result<Vec<Backup>, String> {
    let dir = root(&app, id)?;
    let Ok(rd) = fs::read_dir(&dir) else { return Ok(vec![]) };
    let mut out: Vec<Backup> = vec![];
    for e in rd.filter_map(|e| e.ok()) {
        let name = e.file_name().to_string_lossy().to_string();
        let Some((world, stamp)) = name.rsplit_once("__") else { continue };
        let Ok(time) = stamp.parse::<u64>() else { continue };
        if !e.path().is_dir() {
            continue;
        }
        out.push(Backup { world: world.to_string(), time, size: instances::dir_size(&e.path()), name: name.clone() });
    }
    out.sort_by(|a, b| b.time.cmp(&a.time));
    Ok(out)
}

fn restore_blocking(app: &tauri::AppHandle, id: u64, backup: &str) -> Result<(), String> {
    if !valid(backup) {
        return Err("Invalid backup name".into());
    }
    let (world, _) = backup.rsplit_once("__").ok_or("That isn't a backup")?;
    if !valid(world) {
        return Err("Invalid world name".into());
    }
    let backups = root(app, id)?;
    let src = backups.join(backup);
    if !src.is_dir() {
        return Err("Backup not found".into());
    }
    let target = saves(app, id)?.join(world);
    if target.exists() {
        // Keep what is being replaced, as a normal backup, so a restore can always be undone.
        fs::create_dir_all(&backups).map_err(|e| e.to_string())?;
        let keep = backups.join(format!("{world}__{}", now_ms()));
        fs::rename(&target, &keep).map_err(|e| format!("Couldn't set the current world aside: {e}"))?;
    }
    instances::copy_dir(&src, &target).map_err(|e| format!("Couldn't restore the backup: {e}"))
}

#[tauri::command]
pub async fn restore_backup(app: tauri::AppHandle, id: u64, backup: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || restore_blocking(&app, id, &backup))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn delete_backup(app: tauri::AppHandle, id: u64, backup: String) -> Result<(), String> {
    if !valid(&backup) || !backup.contains("__") {
        return Err("Invalid backup name".into());
    }
    let dir = root(&app, id)?.join(&backup);
    if dir.is_dir() {
        fs::remove_dir_all(dir).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn world_changed_ms(world_dir: &std::path::Path) -> u64 {
    fs::metadata(world_dir.join("level.dat"))
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Backs up every world that changed since its newest backup, then keeps only the newest `keep`
/// backups of each world. Returns how many backups were made.
pub(crate) fn auto_backup(app: &tauri::AppHandle, id: u64, keep: usize) -> Result<usize, String> {
    let dir = saves(app, id)?;
    let Ok(rd) = fs::read_dir(&dir) else { return Ok(0) };
    let existing = list_backups(app.clone(), id)?;
    let mut made = 0usize;
    for e in rd.filter_map(|e| e.ok()) {
        let world = e.file_name().to_string_lossy().to_string();
        if !valid(&world) || !e.path().join("level.dat").is_file() {
            continue;
        }
        let newest = existing.iter().filter(|b| b.world == world).map(|b| b.time).max().unwrap_or(0);
        if world_changed_ms(&e.path()) > newest {
            backup_blocking(app, id, &world)?;
            made += 1;
        }
    }
    // Keep only the newest few of each world (the list is newest first).
    let all = list_backups(app.clone(), id)?;
    let mut worlds: Vec<String> = all.iter().map(|b| b.world.clone()).collect();
    worlds.sort();
    worlds.dedup();
    let backups = root(app, id)?;
    for w in worlds {
        for old in all.iter().filter(|b| b.world == w).skip(keep) {
            let _ = fs::remove_dir_all(backups.join(&old.name));
        }
    }
    Ok(made)
}

fn all_blocking(app: &tauri::AppHandle, id: u64) -> Result<usize, String> {
    let dir = saves(app, id)?;
    let Ok(rd) = fs::read_dir(&dir) else { return Ok(0) };
    let mut made = 0usize;
    for e in rd.filter_map(|e| e.ok()) {
        let world = e.file_name().to_string_lossy().to_string();
        if !valid(&world) || !e.path().join("level.dat").is_file() {
            continue;
        }
        backup_blocking(app, id, &world)?;
        made += 1;
    }
    Ok(made)
}

#[tauri::command]
pub async fn backup_all_worlds(app: tauri::AppHandle, id: u64) -> Result<usize, String> {
    tauri::async_runtime::spawn_blocking(move || all_blocking(&app, id))
        .await
        .map_err(|e| e.to_string())?
}
