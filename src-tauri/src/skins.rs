use crate::data_dir;
use serde::Serialize;
use std::{fs, path::PathBuf};

#[derive(Serialize)]
pub struct SkinFile {
    pub name: String,
    pub png_base64: String,
    pub active: bool,
}

fn dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let d = data_dir(app)?.join("skins");
    fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    Ok(d)
}

fn safe(name: &str) -> String {
    let s: String = name.chars().filter(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | ' ')).collect();
    let s = s.trim().to_string();
    if s.is_empty() { "skin".into() } else { s }
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
pub fn list_skins(app: tauri::AppHandle) -> Result<Vec<SkinFile>, String> {
    let d = dir(&app)?;
    let active = fs::read_to_string(d.join("active.txt")).unwrap_or_default();
    let mut out = vec![];
    for e in fs::read_dir(&d).map_err(|e| e.to_string())? {
        let p = e.map_err(|e| e.to_string())?.path();
        if p.extension().and_then(|x| x.to_str()) != Some("png") {
            continue;
        }
        let name = p.file_stem().and_then(|s| s.to_str()).unwrap_or("skin").to_string();
        let bytes = fs::read(&p).map_err(|e| e.to_string())?;
        out.push(SkinFile { active: name == active.trim(), name, png_base64: b64(&bytes) });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// Accepts bytes of any supported image format and stores them as PNG.
#[tauri::command]
pub fn save_skin(app: tauri::AppHandle, name: String, data: Vec<u8>) -> Result<String, String> {
    let img = image::load_from_memory(&data).map_err(|e| format!("Not a readable image: {e}"))?;
    let name = safe(&name);
    img.save_with_format(dir(&app)?.join(format!("{name}.png")), image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(name)
}

#[tauri::command]
pub fn delete_skin(app: tauri::AppHandle, name: String) -> Result<(), String> {
    let d = dir(&app)?;
    let name = safe(&name);
    let _ = fs::remove_file(d.join(format!("{name}.png")));
    if fs::read_to_string(d.join("active.txt")).map(|s| s.trim() == name).unwrap_or(false) {
        let _ = fs::remove_file(d.join("active.txt"));
    }
    Ok(())
}

#[tauri::command]
pub fn set_active_skin(app: tauri::AppHandle, name: String) -> Result<(), String> {
    fs::write(dir(&app)?.join("active.txt"), safe(&name)).map_err(|e| e.to_string())
}
