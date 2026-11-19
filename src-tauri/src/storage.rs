//! Shared storage: when several instances hold the exact same mod (or resource pack, or shader), the
//! copies are replaced by hard links to one file, so the data is only stored once. Each instance still
//! sees its own normal file, and deleting it in one instance never affects the others.

use crate::{data_dir, instances};
use serde::{Deserialize, Serialize};
use sha1::Sha1;
use sha2::Digest;
use std::{
    collections::HashMap,
    fs,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::UNIX_EPOCH,
};

static RUNNING: AtomicBool = AtomicBool::new(false);

/// Releases the "already running" flag however the function ends.
struct Guard;
impl Drop for Guard {
    fn drop(&mut self) {
        RUNNING.store(false, Ordering::SeqCst);
    }
}

#[derive(Serialize)]
pub struct DedupeResult {
    pub merged: usize,
    pub saved: u64,
}

#[derive(Serialize, Deserialize, Default)]
struct Index {
    /// Files already handled: path -> (size, modified seconds). Skipped next time unless they changed.
    files: HashMap<String, (u64, u64)>,
    /// A known copy of each content hash: hash -> path.
    canon: HashMap<String, String>,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn hash_file(p: &Path) -> Option<String> {
    let bytes = fs::read(p).ok()?;
    Some(hex(&Sha1::digest(&bytes[..])))
}

fn modified_secs(m: &fs::Metadata) -> u64 {
    m.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0)
}

pub(crate) fn dedupe(app: &tauri::AppHandle) -> Result<DedupeResult, String> {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return Ok(DedupeResult { merged: 0, saved: 0 });
    }
    let _guard = Guard;

    let root = data_dir(app)?;
    let index_file = root.join("shared-index.json");
    let mut index: Index = fs::read_to_string(&index_file)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    let mut merged = 0usize;
    let mut saved = 0u64;

    for inst in instances::load(app)? {
        for sub in ["mods", "resourcepacks", "shaderpacks"] {
            let dir = instances::dir_of(app, inst.id)?.join(".minecraft").join(sub);
            let Ok(rd) = fs::read_dir(&dir) else { continue };
            for e in rd.filter_map(|e| e.ok()) {
                let name = e.file_name().to_string_lossy().to_string();
                let lower = name.to_lowercase();
                if !(lower.ends_with(".jar") || lower.ends_with(".zip") || lower.ends_with(".jar.disabled")) {
                    continue;
                }
                let path = e.path();
                let Ok(meta) = e.metadata() else { continue };
                if !meta.is_file() {
                    continue;
                }
                let key = path.display().to_string();
                let state = (meta.len(), modified_secs(&meta));
                if index.files.get(&key) == Some(&state) {
                    continue;
                }
                let Some(hash) = hash_file(&path) else { continue };

                // Is there already another copy with exactly this content?
                let existing = index.canon.get(&hash).cloned();
                let usable = existing.as_ref().filter(|c| {
                    c.as_str() != key
                        && fs::metadata(c.as_str()).map(|m| m.len() == meta.len()).unwrap_or(false)
                        && hash_file(Path::new(c.as_str())).as_deref() == Some(hash.as_str())
                });
                if let Some(canon_path) = usable {
                    let tmp = dir.join(format!("{name}.linktmp"));
                    let _ = fs::remove_file(&tmp);
                    if fs::hard_link(canon_path, &tmp).is_err() {
                        continue;
                    }
                    if fs::rename(&tmp, &path).is_err() {
                        let _ = fs::remove_file(&tmp);
                        continue;
                    }
                    merged += 1;
                    saved += meta.len();
                } else {
                    index.canon.insert(hash, key.clone());
                }
                if let Ok(m2) = fs::metadata(&path) {
                    index.files.insert(key, (m2.len(), modified_secs(&m2)));
                }
            }
        }
    }
    if let Ok(text) = serde_json::to_string(&index) {
        let _ = fs::write(&index_file, text);
    }
    Ok(DedupeResult { merged, saved })
}

/// Runs in the background after installs, without making anyone wait.
pub(crate) fn dedupe_in_background(app: tauri::AppHandle) {
    tauri::async_runtime::spawn_blocking(move || {
        let _ = dedupe(&app);
    });
}

#[tauri::command]
pub async fn dedupe_storage(app: tauri::AppHandle) -> Result<DedupeResult, String> {
    tauri::async_runtime::spawn_blocking(move || dedupe(&app))
        .await
        .map_err(|e| e.to_string())?
}
