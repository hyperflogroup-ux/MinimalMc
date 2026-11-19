use serde::Serialize;
use std::{env, path::PathBuf};
use tokio::process::Command;

#[derive(Serialize)]
pub struct JavaInstall {
    pub path: String,
    pub version: String,
}

fn candidates() -> Vec<PathBuf> {
    let exe = if cfg!(windows) { "java.exe" } else { "java" };
    let mut out = vec![];
    if let Ok(h) = env::var("JAVA_HOME") {
        out.push(PathBuf::from(h).join("bin").join(exe));
    }
    if let Some(p) = env::var_os("PATH") {
        out.extend(env::split_paths(&p).map(|d| d.join(exe)));
    }
    out.retain(|p| p.is_file());
    out.dedup();
    out
}

#[tauri::command]
pub async fn detect_java() -> Result<Vec<JavaInstall>, String> {
    let mut found = vec![];
    for path in candidates() {
        let Ok(o) = Command::new(&path).arg("-version").output().await else { continue };
        let text = String::from_utf8_lossy(&o.stderr);
        let version = text
            .split('"')
            .nth(1)
            .unwrap_or("unknown")
            .to_string();
        found.push(JavaInstall { path: path.display().to_string(), version });
    }
    Ok(found)
}
