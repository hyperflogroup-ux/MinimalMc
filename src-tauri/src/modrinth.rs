use crate::instances;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha512};
use std::{
    collections::HashSet,
    fs,
    path::Path,
    time::Duration,
};

const API: &str = "https://api.modrinth.com/v2";
// Modrinth requires a uniquely identifying User-Agent. Add contact info here if you like,
// e.g. "yourname/MinimalMc/1.0.0 (you@example.com)".
const USER_AGENT: &str = "MinimalMc/1.0.0";

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())
}

/// Maps the app's page names to Modrinth project types.
fn norm_type(t: &str) -> Result<&'static str, String> {
    match t {
        "mods" | "mod" => Ok("mod"),
        "modpacks" | "modpack" => Ok("modpack"),
        "shaders" | "shader" => Ok("shader"),
        "packs" | "resourcepack" => Ok("resourcepack"),
        _ => Err(format!("Unknown project type: {t}")),
    }
}

// ───────────────────────── Search ─────────────────────────

#[derive(Deserialize)]
struct SearchResponse {
    hits: Vec<Hit>,
    #[serde(default)]
    total_hits: u64,
}

#[derive(Deserialize, Serialize)]
pub struct Hit {
    pub project_id: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub downloads: u64,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub project_type: String,
    #[serde(default)]
    pub latest_version: Option<String>,
}

#[derive(Serialize)]
pub struct SearchResult {
    pub hits: Vec<Hit>,
    pub total: u64,
}

#[tauri::command]
pub async fn modrinth_search(
    query: String,
    project_type: String,
    loader: Option<String>,
    game_version: Option<String>,
    sort: String,
    offset: u32,
) -> Result<SearchResult, String> {
    let ptype = norm_type(&project_type)?;

    let mut facets: Vec<Vec<String>> = vec![vec![format!("project_type:{ptype}")]];
    if ptype == "mod" || ptype == "modpack" {
        if let Some(l) = loader.map(|l| l.to_lowercase()) {
            if l == "quilt" {
                // Quilt runs most Fabric mods too.
                facets.push(vec!["categories:quilt".to_string(), "categories:fabric".to_string()]);
            } else if !l.is_empty() && l != "vanilla" && l != "any" {
                facets.push(vec![format!("categories:{l}")]);
            }
        }
    }
    if let Some(v) = game_version {
        if !v.is_empty() {
            facets.push(vec![format!("versions:{v}")]);
        }
    }
    let facets = serde_json::to_string(&facets).map_err(|e| e.to_string())?;

    let index = match sort.as_str() {
        "downloads" => "downloads",
        "follows" => "follows",
        "newest" => "newest",
        "updated" => "updated",
        _ => "relevance",
    };
    let offset = offset.to_string();

    let resp: SearchResponse = client()?
        .get(format!("{API}/search"))
        .query(&[
            ("query", query.as_str()),
            ("facets", facets.as_str()),
            ("index", index),
            ("limit", "20"),
            ("offset", offset.as_str()),
        ])
        .send()
        .await
        .map_err(|e| format!("Couldn't reach Modrinth: {e}"))?
        .error_for_status()
        .map_err(|e| format!("Modrinth returned an error: {e}"))?
        .json()
        .await
        .map_err(|e| format!("Unexpected response from Modrinth: {e}"))?;

    Ok(SearchResult { hits: resp.hits, total: resp.total_hits })
}

// ───────────────────────── Install ─────────────────────────

#[derive(Deserialize)]
struct MrVersion {
    #[serde(default)]
    version_type: String,
    #[serde(default)]
    files: Vec<MrFile>,
    #[serde(default)]
    dependencies: Vec<MrDep>,
}

#[derive(Deserialize)]
struct MrFile {
    url: String,
    filename: String,
    #[serde(default)]
    primary: bool,
    hashes: MrHashes,
}

#[derive(Deserialize)]
struct MrHashes {
    #[serde(default)]
    sha512: Option<String>,
}

#[derive(Deserialize)]
struct MrDep {
    #[serde(default)]
    project_id: Option<String>,
    #[serde(default)]
    dependency_type: String,
}

#[derive(Serialize)]
pub struct InstallResult {
    pub installed: Vec<String>,
    pub already_present: Vec<String>,
    pub warnings: Vec<String>,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Only ever a bare file name: no folders, no hidden files, no drive letters.
fn safe_filename(name: &str) -> Result<String, String> {
    let n = Path::new(name)
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("Bad file name from Modrinth")?;
    if n.is_empty() || n.starts_with('.') || n.contains(|c: char| matches!(c, '/' | '\\' | ':')) {
        return Err("Unsafe file name from Modrinth".into());
    }
    Ok(n.to_string())
}

/// Newest compatible version, preferring full releases over betas/alphas.
async fn pick_version(
    c: &reqwest::Client,
    project: &str,
    ptype: &str,
    loader: &str,
    mc: &str,
) -> Result<MrVersion, String> {
    let mut q: Vec<(&str, String)> = vec![("game_versions", serde_json::to_string(&[mc]).map_err(|e| e.to_string())?)];
    if ptype == "mod" {
        let l = loader.to_lowercase();
        let loaders: Vec<String> = if l == "quilt" { vec!["quilt".into(), "fabric".into()] } else { vec![l] };
        q.push(("loaders", serde_json::to_string(&loaders).map_err(|e| e.to_string())?));
    }
    let list: Vec<MrVersion> = c
        .get(format!("{API}/project/{project}/version"))
        .query(&q)
        .send()
        .await
        .map_err(|e| format!("Couldn't reach Modrinth: {e}"))?
        .error_for_status()
        .map_err(|e| format!("Modrinth returned an error: {e}"))?
        .json()
        .await
        .map_err(|e| format!("Unexpected response from Modrinth: {e}"))?;

    let idx = list.iter().position(|v| v.version_type == "release").unwrap_or(0);
    list.into_iter().nth(idx).ok_or_else(|| {
        if ptype == "mod" {
            format!("No version is compatible with Minecraft {mc} on {loader}")
        } else {
            format!("No version is compatible with Minecraft {mc}")
        }
    })
}

async fn get_version(c: &reqwest::Client, version_id: &str) -> Result<MrVersion, String> {
    c.get(format!("{API}/version/{version_id}"))
        .send()
        .await
        .map_err(|e| format!("Couldn't reach Modrinth: {e}"))?
        .error_for_status()
        .map_err(|e| format!("Modrinth returned an error: {e}"))?
        .json()
        .await
        .map_err(|e| format!("Unexpected response from Modrinth: {e}"))
}

// ───────────────────────── Project pages ─────────────────────────

#[derive(Deserialize, Serialize, Default)]
pub struct Gallery {
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub featured: bool,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Deserialize, Serialize, Default)]
pub struct License {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Deserialize, Serialize, Default)]
pub struct Project {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub loaders: Vec<String>,
    #[serde(default)]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub downloads: u64,
    #[serde(default)]
    pub followers: u64,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub issues_url: Option<String>,
    #[serde(default)]
    pub source_url: Option<String>,
    #[serde(default)]
    pub wiki_url: Option<String>,
    #[serde(default)]
    pub discord_url: Option<String>,
    #[serde(default)]
    pub license: Option<License>,
    #[serde(default)]
    pub gallery: Vec<Gallery>,
    #[serde(default)]
    pub published: String,
    #[serde(default)]
    pub updated: String,
    #[serde(default)]
    pub client_side: String,
    #[serde(default)]
    pub server_side: String,
    #[serde(default)]
    pub project_type: String,
}

#[derive(Deserialize, Serialize, Default)]
pub struct MemberUser {
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub avatar_url: Option<String>,
}

#[derive(Deserialize, Serialize)]
pub struct Member {
    pub user: MemberUser,
    #[serde(default)]
    pub role: String,
}

#[derive(Deserialize, Serialize)]
pub struct VersionInfo {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version_number: String,
    #[serde(default)]
    pub version_type: String,
    #[serde(default)]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub loaders: Vec<String>,
    #[serde(default)]
    pub date_published: String,
    #[serde(default)]
    pub downloads: u64,
    #[serde(default)]
    pub changelog: Option<String>,
}

#[derive(Serialize)]
pub struct ProjectDetail {
    pub project: Project,
    pub members: Vec<Member>,
    pub versions: Vec<VersionInfo>,
}

async fn get_json<T: serde::de::DeserializeOwned>(c: &reqwest::Client, url: &str) -> Result<T, String> {
    c.get(url)
        .send()
        .await
        .map_err(|e| format!("Couldn't reach Modrinth: {e}"))?
        .error_for_status()
        .map_err(|e| format!("Modrinth returned an error: {e}"))?
        .json::<T>()
        .await
        .map_err(|e| format!("Unexpected response from Modrinth: {e}"))
}

/// Everything shown on a project's page: its description and pictures, who made it, and its versions.
#[tauri::command]
pub async fn modrinth_project(project_id: String) -> Result<ProjectDetail, String> {
    if project_id.is_empty() || !project_id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_')) {
        return Err("Invalid project".into());
    }
    let c = client()?;
    let project: Project = get_json(&c, &format!("{API}/project/{project_id}")).await?;
    let members: Vec<Member> = get_json(&c, &format!("{API}/project/{project_id}/members")).await.unwrap_or_default();
    let mut versions: Vec<VersionInfo> =
        get_json(&c, &format!("{API}/project/{project_id}/version")).await.unwrap_or_default();
    versions.truncate(200);
    for v in versions.iter_mut() {
        v.changelog = v.changelog.take().map(|t| t.chars().take(4000).collect());
    }
    Ok(ProjectDetail { project, members, versions })
}

/// Downloads one file, verifies its SHA-512, and moves it into place.
/// Returns (file name, true if newly written / false if it was already there).
async fn fetch_file(c: &reqwest::Client, f: &MrFile, dir: &Path) -> Result<(String, bool), String> {
    let name = safe_filename(&f.filename)?;
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let dest = dir.join(&name);
    if dest.is_file() {
        return Ok((name, false));
    }

    let bytes = c
        .get(&f.url)
        .send()
        .await
        .map_err(|e| format!("Download failed: {e}"))?
        .error_for_status()
        .map_err(|e| format!("Download failed: {e}"))?
        .bytes()
        .await
        .map_err(|e| format!("Download failed: {e}"))?;

    if let Some(expected) = &f.hashes.sha512 {
        let got = hex(&Sha512::digest(&bytes[..]));
        if !got.eq_ignore_ascii_case(expected) {
            return Err(format!("Checksum mismatch for {name}. The download was rejected."));
        }
    }

    let tmp = dir.join(format!("{name}.part"));
    fs::write(&tmp, &bytes).map_err(|e| e.to_string())?;
    fs::rename(&tmp, &dest).map_err(|e| e.to_string())?;
    Ok((name, true))
}

#[tauri::command]
pub async fn modrinth_install(
    app: tauri::AppHandle,
    instance_id: u64,
    project_id: String,
    project_type: String,
    version_id: Option<String>,
) -> Result<InstallResult, String> {
    let ptype = norm_type(&project_type)?;
    if let Some(v) = &version_id {
        if v.is_empty() || !v.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err("Invalid version".into());
        }
    }
    if ptype == "modpack" {
        return Err("Installing modpacks isn't supported yet.".into());
    }

    let inst = instances::load(&app)?
        .into_iter()
        .find(|i| i.id == instance_id)
        .ok_or("Instance not found")?;
    if ptype == "mod" && inst.loader.eq_ignore_ascii_case("vanilla") {
        return Err("Vanilla instances can't use mods. Choose a Fabric, Forge, NeoForge or Quilt instance.".into());
    }

    let sub = match ptype {
        "mod" => "mods",
        "resourcepack" => "resourcepacks",
        _ => "shaderpacks",
    };
    let dir = instances::dir_of(&app, instance_id)?.join(".minecraft").join(sub);

    let c = client()?;
    let mut result = InstallResult { installed: vec![], already_present: vec![], warnings: vec![] };
    let mut seen: HashSet<String> = HashSet::new();
    let root = project_id.clone();
    let mut queue = vec![project_id];

    while let Some(pid) = queue.pop() {
        if !seen.insert(pid.clone()) {
            continue;
        }
        // The version the player picked is used for the mod itself; its needed mods are chosen automatically.
        let picked = match (&version_id, pid == root) {
            (Some(vid), true) => get_version(&c, vid).await,
            _ => pick_version(&c, &pid, ptype, &inst.loader, &inst.version).await,
        };
        let v = match picked {
            Ok(v) => v,
            Err(e) if pid != root => {
                result.warnings.push(format!("Skipped required dependency {pid}: {e}"));
                continue;
            }
            Err(e) => return Err(e),
        };
        let file = v
            .files
            .iter()
            .find(|f| f.primary)
            .or_else(|| v.files.first())
            .ok_or("This version has no downloadable file")?;
        let (name, fresh) = fetch_file(&c, file, &dir).await?;
        if fresh {
            result.installed.push(name);
        } else {
            result.already_present.push(name);
        }
        if ptype == "mod" {
            for d in &v.dependencies {
                if d.dependency_type == "required" {
                    if let Some(p) = &d.project_id {
                        queue.push(p.clone());
                    }
                }
            }
        }
    }

    if ptype == "mod" {
        let count = fs::read_dir(&dir)
            .map(|r| {
                r.filter_map(|e| e.ok())
                    .filter(|e| e.path().extension().map(|x| x == "jar").unwrap_or(false))
                    .count()
            })
            .unwrap_or(0) as u32;
        let mut list = instances::load(&app)?;
        if let Some(i) = list.iter_mut().find(|i| i.id == instance_id) {
            i.mods = count;
        }
        instances::save(&app, &list)?;
    }

    crate::storage::dedupe_in_background(app.clone());
    Ok(result)
}
