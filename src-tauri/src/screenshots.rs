//! Screenshots taken in the game (F2). Thumbnails are made on demand so a big folder stays quick.

use crate::instances;
use serde::Serialize;
use std::{fs, io::Cursor, path::PathBuf, time::UNIX_EPOCH};

#[derive(Serialize)]
pub struct Shot {
    pub name: String,
    pub size: u64,
    /// Milliseconds since 1970.
    pub time: u64,
}

fn folder(app: &tauri::AppHandle, id: u64) -> Result<PathBuf, String> {
    Ok(instances::dir_of(app, id)?.join(".minecraft").join("screenshots"))
}

/// A plain image file name: no folders, and only picture types.
fn valid(n: &str) -> bool {
    let l = n.to_lowercase();
    !n.is_empty()
        && !n.starts_with('.')
        && !n.contains(|c: char| matches!(c, '/' | '\\' | ':'))
        && (l.ends_with(".png") || l.ends_with(".jpg") || l.ends_with(".jpeg"))
}

fn b64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(bytes.len() * 4 / 3 + 4);
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        s.push(T[(n >> 18) as usize & 63] as char);
        s.push(T[(n >> 12) as usize & 63] as char);
        s.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        s.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    s
}

#[tauri::command]
pub fn list_screenshots(app: tauri::AppHandle, id: u64) -> Result<Vec<Shot>, String> {
    let dir = folder(&app, id)?;
    let Ok(rd) = fs::read_dir(&dir) else { return Ok(vec![]) };
    let mut out: Vec<Shot> = vec![];
    for e in rd.filter_map(|e| e.ok()) {
        let name = e.file_name().to_string_lossy().to_string();
        if !valid(&name) {
            continue;
        }
        let Ok(meta) = e.metadata() else { continue };
        let time = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        out.push(Shot { name, size: meta.len(), time });
    }
    out.sort_by(|a, b| b.time.cmp(&a.time));
    out.truncate(300);
    Ok(out)
}

fn thumb_blocking(path: PathBuf) -> Result<String, String> {
    let img = image::open(&path).map_err(|e| format!("Couldn't read the screenshot: {e}"))?;
    let small = img.thumbnail(360, 220);
    let mut buf = Cursor::new(Vec::new());
    small.write_to(&mut buf, image::ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(b64(&buf.into_inner()))
}

/// A small PNG of the screenshot, as base64.
#[tauri::command]
pub async fn screenshot_thumb(app: tauri::AppHandle, id: u64, name: String) -> Result<String, String> {
    if !valid(&name) {
        return Err("Invalid file name".into());
    }
    let path = folder(&app, id)?.join(&name);
    tauri::async_runtime::spawn_blocking(move || thumb_blocking(path))
        .await
        .map_err(|e| e.to_string())?
}

/// The full-size file, as base64.
#[tauri::command]
pub async fn screenshot_full(app: tauri::AppHandle, id: u64, name: String) -> Result<String, String> {
    if !valid(&name) {
        return Err("Invalid file name".into());
    }
    let path = folder(&app, id)?.join(&name);
    tauri::async_runtime::spawn_blocking(move || {
        let bytes = fs::read(&path).map_err(|e| e.to_string())?;
        Ok::<String, String>(b64(&bytes))
    })
    .await
    .map_err(|e| e.to_string())?
}
