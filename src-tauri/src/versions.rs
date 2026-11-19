use serde::{Deserialize, Serialize};

const MANIFEST: &str = "https://launchermeta.mojang.com/mc/game/version_manifest_v2.json";

#[derive(Deserialize)]
struct Manifest {
    versions: Vec<Entry>,
}

#[derive(Deserialize)]
struct Entry {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    #[serde(rename = "releaseTime")]
    release_time: String,
}

#[derive(Serialize)]
pub struct Version {
    pub id: String,
    pub kind: String,
    pub released: String,
}

#[tauri::command]
pub async fn fetch_versions(include_snapshots: bool) -> Result<Vec<Version>, String> {
    let m: Manifest = reqwest::get(MANIFEST)
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    Ok(m.versions
        .into_iter()
        .filter(|v| include_snapshots || v.kind == "release")
        .map(|v| Version { id: v.id, kind: v.kind, released: v.release_time })
        .collect())
}
