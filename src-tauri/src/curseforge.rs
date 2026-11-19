//! CurseForge support. It needs the player's own API key (saved in Settings), which stays on this
//! computer and is only ever sent to CurseForge.
//!
//! Search results and project pages are turned into the same shapes the Modrinth side uses, so the
//! same screens show both. Mods whose authors don't allow downloads through launchers are never
//! downloaded: the player is told, and pointed to the CurseForge page instead.

use crate::instances::{self, Instance};
use crate::launch::{self, download_all, Job};
use crate::modrinth::{Gallery, Hit, InstallResult, Member, MemberUser, Project, ProjectDetail, SearchResult, VersionInfo};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha1::Sha1;
use sha2::Digest;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

const API: &str = "https://api.curseforge.com/v1";
const GAME: u32 = 432;
const USER_AGENT: &str = "MinimalMc/1.0.0";
const LOADER_NAMES: [&str; 6] = ["Forge", "Fabric", "Quilt", "NeoForge", "LiteLoader", "Rift"];

// ───────────────────────── The API key ─────────────────────────

fn key_file(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(crate::data_dir(app)?.join("curseforge.key"))
}

fn read_key(app: &tauri::AppHandle) -> Option<String> {
    let k = fs::read_to_string(key_file(app).ok()?).ok()?;
    let k = k.trim().to_string();
    if k.is_empty() {
        None
    } else {
        Some(k)
    }
}

#[derive(Serialize)]
pub struct KeyStatus {
    pub saved: bool,
    /// The last few characters, so the player can tell which key is saved.
    pub hint: String,
}

#[tauri::command]
pub fn curseforge_status(app: tauri::AppHandle) -> KeyStatus {
    match read_key(&app) {
        Some(k) => {
            let tail: String = k.chars().rev().take(4).collect::<Vec<char>>().into_iter().rev().collect();
            KeyStatus { saved: true, hint: tail }
        }
        None => KeyStatus { saved: false, hint: String::new() },
    }
}

/// Saves the key. An empty key removes it.
#[tauri::command]
pub fn curseforge_set_key(app: tauri::AppHandle, key: String) -> Result<(), String> {
    let k = key.trim().to_string();
    let path = key_file(&app)?;
    if k.is_empty() {
        let _ = fs::remove_file(&path);
        return Ok(());
    }
    if k.len() > 300 || k.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err("That doesn't look like an API key.".into());
    }
    fs::write(&path, &k).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())
}

async fn get_json<T: DeserializeOwned>(app: &tauri::AppHandle, path: &str, query: &[(&str, String)]) -> Result<T, String> {
    let key = read_key(app).ok_or("Add your CurseForge API key in Settings first.")?;
    let r = client()?
        .get(format!("{API}{path}"))
        .query(query)
        .header("x-api-key", key)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("Couldn't reach CurseForge: {e}"))?;
    let status = r.status();
    if status.as_u16() == 401 || status.as_u16() == 403 {
        return Err("CurseForge didn't accept the API key. Check it in Settings, under CurseForge.".into());
    }
    if !status.is_success() {
        return Err(format!("CurseForge returned an error ({status})."));
    }
    r.json::<T>().await.map_err(|e| format!("Unexpected response from CurseForge: {e}"))
}

#[tauri::command]
pub async fn curseforge_test(app: tauri::AppHandle) -> Result<String, String> {
    get_json::<serde_json::Value>(&app, &format!("/games/{GAME}"), &[]).await?;
    Ok("Connected to CurseForge. Your key works.".to_string())
}

// ───────────────────────── What CurseForge sends back ─────────────────────────

#[derive(Deserialize)]
struct Data<T> {
    data: T,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct Pagination {
    #[serde(default)]
    total_count: u64,
}

#[derive(Deserialize)]
struct SearchResp {
    data: Vec<CfMod>,
    #[serde(default)]
    pagination: Option<Pagination>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct CfImage {
    title: Option<String>,
    description: Option<String>,
    thumbnail_url: Option<String>,
    url: Option<String>,
}

#[derive(Deserialize)]
struct CfAuthor {
    #[serde(default)]
    name: String,
}

#[derive(Deserialize)]
struct CfCategory {
    #[serde(default)]
    name: String,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct CfLinks {
    website_url: Option<String>,
    wiki_url: Option<String>,
    issues_url: Option<String>,
    source_url: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfIndex {
    #[serde(default)]
    game_version: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfMod {
    id: u64,
    #[serde(default)]
    name: String,
    #[serde(default)]
    slug: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    download_count: f64,
    logo: Option<CfImage>,
    #[serde(default)]
    authors: Vec<CfAuthor>,
    #[serde(default)]
    categories: Vec<CfCategory>,
    #[serde(default)]
    screenshots: Vec<CfImage>,
    links: Option<CfLinks>,
    #[serde(default)]
    latest_files_indexes: Vec<CfIndex>,
    #[serde(default)]
    date_created: String,
    #[serde(default)]
    date_modified: String,
    class_id: Option<u64>,
}

#[derive(Deserialize, Clone, Default)]
struct CfHash {
    #[serde(default)]
    value: String,
    #[serde(default)]
    algo: u32,
}

#[derive(Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
struct CfDep {
    #[serde(default)]
    mod_id: u64,
    #[serde(default)]
    relation_type: u32,
}

#[derive(Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
struct CfFile {
    #[serde(default)]
    id: u64,
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    file_name: String,
    #[serde(default)]
    release_type: u32,
    #[serde(default)]
    file_date: String,
    #[serde(default)]
    download_count: f64,
    download_url: Option<String>,
    #[serde(default)]
    game_versions: Vec<String>,
    #[serde(default)]
    hashes: Vec<CfHash>,
    #[serde(default)]
    dependencies: Vec<CfDep>,
}

// ───────────────────────── Helpers ─────────────────────────

fn class_id(project_type: &str) -> Result<u32, String> {
    match project_type {
        "mods" | "mod" => Ok(6),
        "modpacks" | "modpack" => Ok(4471),
        "shaders" | "shader" => Ok(6552),
        "packs" | "resourcepack" => Ok(12),
        _ => Err(format!("Unknown project type: {project_type}")),
    }
}

/// CurseForge's number for a mod loader.
fn loader_type(l: &str) -> Option<u32> {
    match l {
        "forge" => Some(1),
        "fabric" => Some(4),
        "quilt" => Some(5),
        "neoforge" => Some(6),
        _ => None,
    }
}

/// A file's game version list mixes Minecraft versions and loader names. Splits them:
/// (Minecraft versions, loaders).
fn split_versions(list: &[String]) -> (Vec<String>, Vec<String>) {
    let mut versions: Vec<String> = vec![];
    let mut loaders: Vec<String> = vec![];
    for v in list {
        if LOADER_NAMES.contains(&v.as_str()) {
            loaders.push(v.to_lowercase());
        } else if v.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            versions.push(v.clone());
        }
    }
    (versions, loaders)
}

fn version_key(s: &str) -> Vec<u64> {
    s.split(|c: char| !c.is_ascii_digit()).filter_map(|p| p.parse::<u64>().ok()).collect()
}

fn release_name(t: u32) -> &'static str {
    match t {
        1 => "release",
        2 => "beta",
        _ => "alpha",
    }
}

fn safe_name(n: &str) -> bool {
    !n.is_empty() && n != "." && n != ".." && !n.contains(|c: char| matches!(c, '/' | '\\' | ':'))
}

/// Files may only come from CurseForge's own file servers.
fn cdn_ok(url: &str) -> bool {
    url.strip_prefix("https://")
        .map(|r| r.split('/').next().unwrap_or(""))
        .map(|h| h == "forgecdn.net" || h.ends_with(".forgecdn.net"))
        .unwrap_or(false)
}

// ───────────────────────── Search ─────────────────────────

#[tauri::command]
pub async fn curseforge_search(
    app: tauri::AppHandle,
    query: String,
    project_type: String,
    loader: Option<String>,
    game_version: Option<String>,
    sort: String,
    offset: u32,
) -> Result<SearchResult, String> {
    let class = class_id(&project_type)?;
    let sort_field = match sort.as_str() {
        "downloads" => "6",
        "newest" => "11",
        "updated" => "3",
        _ => "2",
    };
    let mut q: Vec<(&str, String)> = vec![
        ("gameId", GAME.to_string()),
        ("classId", class.to_string()),
        ("sortField", sort_field.to_string()),
        ("sortOrder", "desc".to_string()),
        ("index", offset.to_string()),
        ("pageSize", "20".to_string()),
    ];
    if !query.trim().is_empty() {
        q.push(("searchFilter", query.trim().to_string()));
    }
    if let Some(v) = game_version.filter(|v| !v.is_empty()) {
        q.push(("gameVersion", v));
    }
    if class == 6 || class == 4471 {
        if let Some(t) = loader.map(|l| l.to_lowercase()).and_then(|l| loader_type(&l)) {
            q.push(("modLoaderType", t.to_string()));
        }
    }
    let resp: SearchResp = get_json(&app, "/mods/search", &q).await?;
    let total = resp.pagination.map(|p| p.total_count).unwrap_or(0);
    let hits = resp
        .data
        .into_iter()
        .map(|m| Hit {
            project_id: m.id.to_string(),
            slug: m.slug,
            title: m.name,
            description: m.summary,
            author: m.authors.first().map(|a| a.name.clone()).unwrap_or_default(),
            downloads: m.download_count as u64,
            icon_url: m.logo.and_then(|l| l.thumbnail_url.or(l.url)).filter(|u| !u.is_empty()),
            categories: m.categories.iter().map(|c| c.name.to_lowercase()).collect(),
            project_type: project_type.clone(),
            latest_version: m.latest_files_indexes.first().map(|i| i.game_version.clone()),
        })
        .collect();
    Ok(SearchResult { hits, total })
}

// ───────────────────────── Project pages ─────────────────────────

#[tauri::command]
pub async fn curseforge_project(app: tauri::AppHandle, project_id: String) -> Result<ProjectDetail, String> {
    let id: u64 = project_id.trim().parse().map_err(|_| "Invalid project".to_string())?;
    let m: Data<CfMod> = get_json(&app, &format!("/mods/{id}"), &[]).await?;
    let m = m.data;
    let body: String = get_json::<Data<String>>(&app, &format!("/mods/{id}/description"), &[])
        .await
        .map(|d| d.data)
        .unwrap_or_default();
    let mut files: Vec<CfFile> = get_json::<Data<Vec<CfFile>>>(&app, &format!("/mods/{id}/files"), &[("pageSize", "50".to_string())])
        .await
        .map(|d| d.data)
        .unwrap_or_default();
    files.sort_by(|a, b| b.file_date.cmp(&a.file_date));

    let mut all_versions: Vec<String> = vec![];
    let mut all_loaders: Vec<String> = vec![];
    let mut versions: Vec<VersionInfo> = vec![];
    for f in &files {
        let (gv, lo) = split_versions(&f.game_versions);
        all_versions.extend(gv.iter().cloned());
        all_loaders.extend(lo.iter().cloned());
        versions.push(VersionInfo {
            id: f.id.to_string(),
            name: f.display_name.clone(),
            version_number: f.file_name.clone(),
            version_type: release_name(f.release_type).to_string(),
            game_versions: gv,
            loaders: lo,
            date_published: f.file_date.clone(),
            downloads: f.download_count as u64,
            changelog: None,
        });
    }
    all_versions.sort_by(|a, b| version_key(a).cmp(&version_key(b)));
    all_versions.dedup();
    all_loaders.sort();
    all_loaders.dedup();

    let links = m.links.unwrap_or_default();
    let project = Project {
        id: m.id.to_string(),
        slug: m.slug.clone(),
        title: m.name.clone(),
        description: m.summary.clone(),
        body,
        categories: m.categories.iter().map(|c| c.name.to_lowercase()).collect(),
        loaders: all_loaders,
        game_versions: all_versions,
        downloads: m.download_count as u64,
        icon_url: m.logo.as_ref().and_then(|l| l.url.clone()).filter(|u| !u.is_empty()),
        issues_url: links.issues_url,
        source_url: links.source_url,
        wiki_url: links.wiki_url,
        gallery: m
            .screenshots
            .iter()
            .filter_map(|s| {
                let url = s.url.clone().unwrap_or_default();
                if url.is_empty() {
                    None
                } else {
                    Some(Gallery { url, featured: false, title: s.title.clone(), description: s.description.clone() })
                }
            })
            .collect(),
        published: m.date_created.clone(),
        updated: m.date_modified.clone(),
        ..Default::default()
    };
    let members: Vec<Member> = m
        .authors
        .iter()
        .map(|a| Member { user: MemberUser { username: a.name.clone(), avatar_url: None }, role: "Author".to_string() })
        .collect();
    Ok(ProjectDetail { project, members, versions })
}

// ───────────────────────── Installing ─────────────────────────

async fn pick_file(app: &tauri::AppHandle, mod_id: u64, inst: &Instance, is_mod: bool) -> Result<CfFile, String> {
    // Quilt can run Fabric mods too, so Quilt looks for Quilt files first and Fabric files second.
    let mut tries: Vec<Option<u32>> = vec![];
    if is_mod {
        match inst.loader.to_lowercase().as_str() {
            "quilt" => {
                tries.push(Some(5));
                tries.push(Some(4));
            }
            other => tries.push(loader_type(other)),
        }
    } else {
        tries.push(None);
    }
    for lt in tries {
        let mut q: Vec<(&str, String)> = vec![("gameVersion", inst.version.clone()), ("pageSize", "50".to_string())];
        if let Some(t) = lt {
            q.push(("modLoaderType", t.to_string()));
        }
        let resp: Data<Vec<CfFile>> = get_json(app, &format!("/mods/{mod_id}/files"), &q).await?;
        let mut files = resp.data;
        files.sort_by(|a, b| b.file_date.cmp(&a.file_date));
        let best = files.iter().find(|f| f.release_type == 1).or_else(|| files.first()).cloned();
        if let Some(f) = best {
            return Ok(f);
        }
    }
    Err(format!("No version of this is compatible with Minecraft {} on {}", inst.version, inst.loader))
}

async fn get_file(app: &tauri::AppHandle, mod_id: u64, file_id: u64) -> Result<CfFile, String> {
    let r: Data<CfFile> = get_json(app, &format!("/mods/{mod_id}/files/{file_id}"), &[]).await?;
    Ok(r.data)
}

async fn website(app: &tauri::AppHandle, mod_id: u64) -> String {
    get_json::<Data<CfMod>>(app, &format!("/mods/{mod_id}"), &[])
        .await
        .ok()
        .and_then(|m| m.data.links)
        .and_then(|l| l.website_url)
        .unwrap_or_default()
}

/// Downloads a file, checks its SHA-1 when CurseForge gives one, and puts it in place.
/// Ok(true) if it was written, Ok(false) if it was already there.
async fn download_to(c: &reqwest::Client, url: &str, file: &CfFile, dir: &Path) -> Result<bool, String> {
    if !cdn_ok(url) || !safe_name(&file.file_name) {
        return Err("CurseForge gave an unexpected download address.".into());
    }
    let dest = dir.join(&file.file_name);
    if dest.is_file() {
        return Ok(false);
    }
    let bytes = c
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Download failed: {e}"))?
        .error_for_status()
        .map_err(|e| format!("Download failed: {e}"))?
        .bytes()
        .await
        .map_err(|e| format!("Download failed: {e}"))?;
    if let Some(h) = file.hashes.iter().find(|h| h.algo == 1) {
        let got: String = Sha1::digest(&bytes[..]).iter().map(|b| format!("{b:02x}")).collect();
        if !got.eq_ignore_ascii_case(&h.value) {
            return Err(format!("{} failed its checksum and was rejected.", file.file_name));
        }
    }
    let tmp = dir.join(format!("{}.part", file.file_name));
    fs::write(&tmp, &bytes).map_err(|e| e.to_string())?;
    fs::rename(&tmp, &dest).map_err(|e| e.to_string())?;
    Ok(true)
}

/// Installs a mod, resource pack or shader from CurseForge, plus the mods it needs.
#[tauri::command]
pub async fn curseforge_install(
    app: tauri::AppHandle,
    instance_id: u64,
    project_id: String,
    project_type: String,
    version_id: Option<String>,
) -> Result<InstallResult, String> {
    let root_id: u64 = project_id.trim().parse().map_err(|_| "Invalid project".to_string())?;
    let root_file: Option<u64> = match &version_id {
        Some(v) => Some(v.trim().parse().map_err(|_| "Invalid version".to_string())?),
        None => None,
    };
    let (sub, is_mod) = match project_type.as_str() {
        "mods" | "mod" => ("mods", true),
        "shaders" | "shader" => ("shaderpacks", false),
        "packs" | "resourcepack" => ("resourcepacks", false),
        "modpacks" | "modpack" => return Err("Installing CurseForge modpacks isn't supported yet.".into()),
        _ => return Err("Unknown project type".into()),
    };
    let inst = instances::load(&app)?
        .into_iter()
        .find(|i| i.id == instance_id)
        .ok_or("Instance not found")?;
    if is_mod && inst.loader.eq_ignore_ascii_case("vanilla") {
        return Err("Vanilla instances can't use mods. Choose a Fabric, Forge, NeoForge or Quilt instance.".into());
    }
    let dir = instances::dir_of(&app, instance_id)?.join(".minecraft").join(sub);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let c = client()?;
    let mut result = InstallResult { installed: vec![], already_present: vec![], warnings: vec![] };
    let mut seen: HashSet<u64> = HashSet::new();
    let mut queue: Vec<(u64, Option<u64>)> = vec![(root_id, root_file)];

    while let Some((mod_id, file_id)) = queue.pop() {
        if !seen.insert(mod_id) {
            continue;
        }
        let found = match file_id {
            Some(f) => get_file(&app, mod_id, f).await,
            None => pick_file(&app, mod_id, &inst, is_mod).await,
        };
        let file = match found {
            Ok(f) => f,
            Err(e) if mod_id != root_id => {
                result.warnings.push(format!("Skipped a required mod ({mod_id}): {e}"));
                continue;
            }
            Err(e) => return Err(e),
        };

        // Authors can switch off downloads through launchers. That choice is respected.
        let url = match file.download_url.clone() {
            Some(u) => u,
            None => {
                let page = website(&app, mod_id).await;
                let what = if mod_id == root_id { "file" } else { "required mod" };
                let more = if page.is_empty() { String::new() } else { format!(" {page}") };
                let msg = format!(
                    "The author of this {what} doesn't allow downloads through launchers. Download it from its CurseForge page and put it in the instance's {sub} folder.{more}"
                );
                if mod_id == root_id {
                    return Err(msg);
                }
                result.warnings.push(msg);
                continue;
            }
        };
        match download_to(&c, &url, &file, &dir).await {
            Ok(true) => result.installed.push(file.file_name.clone()),
            Ok(false) => result.already_present.push(file.file_name.clone()),
            Err(e) => {
                if mod_id == root_id {
                    return Err(e);
                }
                result.warnings.push(format!("Skipped a required mod ({mod_id}): {e}"));
                continue;
            }
        }
        if is_mod {
            for d in &file.dependencies {
                if d.relation_type == 3 {
                    queue.push((d.mod_id, None));
                }
            }
        }
    }

    if is_mod {
        let _ = instances::recount_mods(&app, instance_id);
    }
    crate::storage::dedupe_in_background(app.clone());
    Ok(result)
}

// ───────────────────────── Modpacks ─────────────────────────
//
// A CurseForge modpack is a .zip with manifest.json (the Minecraft version, the loader, and a list of
// project and file numbers) plus an overrides folder. The numbers are looked up through the API.

#[derive(Deserialize)]
struct CfModLoader {
    #[serde(default)]
    id: String,
    #[serde(default)]
    primary: bool,
}

#[derive(Deserialize)]
struct CfMc {
    #[serde(default)]
    version: String,
    #[serde(default, rename = "modLoaders")]
    mod_loaders: Vec<CfModLoader>,
}

#[derive(Deserialize)]
struct CfPackFile {
    #[serde(rename = "projectID")]
    project_id: u64,
    #[serde(rename = "fileID")]
    file_id: u64,
    #[serde(default)]
    required: Option<bool>,
}

#[derive(Deserialize)]
struct CfManifest {
    #[serde(default)]
    name: String,
    minecraft: CfMc,
    #[serde(default)]
    files: Vec<CfPackFile>,
    #[serde(default)]
    overrides: Option<String>,
}

fn read_manifest(pack: &Path) -> Result<CfManifest, String> {
    let f = fs::File::open(pack).map_err(|e| format!("Couldn't open that file: {e}"))?;
    let mut z = zip::ZipArchive::new(f).map_err(|e| format!("That isn't a valid modpack file: {e}"))?;
    let mut e = z
        .by_name("manifest.json")
        .map_err(|_| "This modpack has no manifest.json.".to_string())?;
    let mut text = String::new();
    std::io::Read::read_to_string(&mut e, &mut text).map_err(|e| e.to_string())?;
    let m: CfManifest = serde_json::from_str(text.trim_start_matches('\u{feff}'))
        .map_err(|e| format!("Unreadable modpack manifest: {e}"))?;
    if m.minecraft.version.is_empty() {
        return Err("This modpack doesn't say which Minecraft version it is for.".into());
    }
    Ok(m)
}

/// ("Forge", "47.2.0") from a loader id like "forge-47.2.0".
fn parse_loader(m: &CfMc) -> (String, String) {
    let pick = m.mod_loaders.iter().find(|l| l.primary).or_else(|| m.mod_loaders.first());
    match pick {
        Some(l) => {
            let (name, ver) = l.id.split_once('-').unwrap_or((l.id.as_str(), ""));
            let loader = match name {
                "forge" => "Forge",
                "fabric" => "Fabric",
                "quilt" => "Quilt",
                "neoforge" => "NeoForge",
                _ => "Vanilla",
            };
            (loader.to_string(), ver.to_string())
        }
        None => ("Vanilla".to_string(), String::new()),
    }
}

async fn post_json<T: DeserializeOwned>(app: &tauri::AppHandle, path: &str, body: &serde_json::Value) -> Result<T, String> {
    let key = read_key(app).ok_or("Add your CurseForge API key in Settings first.")?;
    let r = client()?
        .post(format!("{API}{path}"))
        .header("x-api-key", key)
        .header("Accept", "application/json")
        .json(body)
        .send()
        .await
        .map_err(|e| format!("Couldn't reach CurseForge: {e}"))?;
    let status = r.status();
    if status.as_u16() == 401 || status.as_u16() == 403 {
        return Err("CurseForge didn't accept the API key. Check it in Settings, under CurseForge.".into());
    }
    if !status.is_success() {
        return Err(format!("CurseForge returned an error ({status})."));
    }
    r.json::<T>().await.map_err(|e| format!("Unexpected response from CurseForge: {e}"))
}

/// Looks up every file and project in a pack: (files by file number, projects by project number).
async fn resolve(app: &tauri::AppHandle, wanted: &[(u64, u64)]) -> Result<(HashMap<u64, CfFile>, HashMap<u64, CfMod>), String> {
    let mut files: HashMap<u64, CfFile> = HashMap::new();
    let ids: Vec<u64> = wanted.iter().map(|w| w.1).collect();
    for chunk in ids.chunks(100) {
        let r: Data<Vec<CfFile>> = post_json(app, "/mods/files", &serde_json::json!({ "fileIds": chunk })).await?;
        for f in r.data {
            files.insert(f.id, f);
        }
    }
    let mut mod_ids: Vec<u64> = wanted.iter().map(|w| w.0).collect();
    mod_ids.sort();
    mod_ids.dedup();
    let mut mods: HashMap<u64, CfMod> = HashMap::new();
    for chunk in mod_ids.chunks(100) {
        let r: Data<Vec<CfMod>> = post_json(app, "/mods", &serde_json::json!({ "modIds": chunk })).await?;
        for m in r.data {
            mods.insert(m.id, m);
        }
    }
    Ok((files, mods))
}

async fn finish_cf_pack(app: tauri::AppHandle, id: u64, pack: PathBuf, manifest: CfManifest) -> Result<(), String> {
    let game = instances::dir_of(&app, id)?.join(".minecraft");
    launch::set(id, |s| s.stage = "Looking up mods".into());
    let wanted: Vec<(u64, u64)> = manifest
        .files
        .iter()
        .filter(|f| f.required != Some(false))
        .map(|f| (f.project_id, f.file_id))
        .collect();
    let (files, mods) = resolve(&app, &wanted).await?;

    let mut jobs: Vec<Job> = vec![];
    let mut manual: Vec<String> = vec![];
    for (pid, fid) in &wanted {
        let info = mods.get(pid);
        let name = info.map(|m| m.name.clone()).unwrap_or_else(|| format!("Mod {pid}"));
        let site = info.and_then(|m| m.links.as_ref()).and_then(|l| l.website_url.clone()).unwrap_or_default();
        let page = if site.is_empty() { String::new() } else { format!("{site}/files/{fid}") };
        let Some(file) = files.get(fid) else {
            manual.push(format!("{name}: {page}"));
            continue;
        };
        match file.download_url.clone().filter(|u| cdn_ok(u)) {
            Some(url) if safe_name(&file.file_name) => {
                let folder = match info.and_then(|m| m.class_id) {
                    Some(12) => "resourcepacks",
                    Some(6552) => "shaderpacks",
                    _ => "mods",
                };
                jobs.push(Job {
                    url,
                    path: game.join(folder).join(&file.file_name),
                    sha1: file.hashes.iter().find(|h| h.algo == 1).map(|h| h.value.clone()),
                    size: 0,
                });
            }
            // The author doesn't allow launchers to download this one.
            _ => manual.push(format!("{name}: {page}")),
        }
    }
    download_all(&client()?, id, "Installing modpack", jobs).await?;

    launch::set(id, |s| {
        s.stage = "Copying pack files".into();
        s.done = 0;
        s.total = 0;
    });
    let ov = manifest.overrides.clone().unwrap_or_else(|| "overrides".to_string());
    let ov = if ov.is_empty() || ov.contains("..") || ov.starts_with('/') || ov.contains(':') { "overrides".to_string() } else { ov };
    let prefix = format!("{}/", ov.trim_end_matches('/'));
    crate::modpack::extract_overrides(&pack, &game, &[prefix.as_str()])?;

    let notice = if manual.is_empty() {
        None
    } else {
        let text = format!(
            "These files can't be downloaded automatically, because their authors don't allow it. Download each one from its page and put it in the matching folder (usually mods):\n\n{}\n",
            manual.join("\n")
        );
        let _ = fs::write(game.join("MANUAL-DOWNLOADS.txt"), text);
        Some(format!(
            "{} {} must be downloaded by hand. See MANUAL-DOWNLOADS.txt in the instance folder.",
            manual.len(),
            if manual.len() == 1 { "mod" } else { "mods" }
        ))
    };
    let _ = instances::recount_mods(&app, id);
    crate::storage::dedupe_in_background(app.clone());
    launch::set(id, |s| {
        s.busy = false;
        s.stage = "Installed".into();
        s.notice = notice;
    });
    Ok(())
}

/// Creates the instance for a pack and finishes the job in the background.
async fn start_cf_pack(app: tauri::AppHandle, pack: PathBuf, manifest: CfManifest, delete_after: bool) -> Result<Instance, String> {
    let (loader, loader_version) = parse_loader(&manifest.minecraft);
    let name = if manifest.name.trim().is_empty() { "Modpack".to_string() } else { manifest.name.trim().to_string() };
    let inst = instances::create_with(&app, name, manifest.minecraft.version.clone(), loader, loader_version, "Modded".to_string(), 6)?;
    let id = inst.id;
    launch::set(id, |s| {
        *s = launch::LaunchStatus::default();
        s.stage = "Installing modpack".into();
        s.busy = true;
    });
    let a = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = finish_cf_pack(a, id, pack.clone(), manifest).await {
            launch::set(id, |s| {
                s.busy = false;
                s.error = Some(e);
            });
        }
        if delete_after {
            let _ = fs::remove_file(&pack);
        }
    });
    Ok(inst)
}

/// Imports a CurseForge modpack (.zip) from a file on this computer.
pub(crate) async fn import_cf_zip(app: tauri::AppHandle, path: String) -> Result<Instance, String> {
    if read_key(&app).is_none() {
        return Err("Importing a CurseForge modpack needs your CurseForge API key. Add it in Settings, under CurseForge.".into());
    }
    let p = PathBuf::from(path.trim().trim_matches('"'));
    let manifest = read_manifest(&p)?;
    start_cf_pack(app, p, manifest, false).await
}

/// Installs a modpack from CurseForge as a new instance.
#[tauri::command]
pub async fn curseforge_install_modpack(app: tauri::AppHandle, project_id: String, version_id: Option<String>) -> Result<Instance, String> {
    let mod_id: u64 = project_id.trim().parse().map_err(|_| "Invalid project".to_string())?;
    let file = match &version_id {
        Some(v) => {
            let fid: u64 = v.trim().parse().map_err(|_| "Invalid version".to_string())?;
            get_file(&app, mod_id, fid).await?
        }
        None => {
            let r: Data<Vec<CfFile>> = get_json(&app, &format!("/mods/{mod_id}/files"), &[("pageSize", "50".to_string())]).await?;
            let mut files = r.data;
            files.sort_by(|a, b| b.file_date.cmp(&a.file_date));
            files
                .iter()
                .find(|f| f.release_type == 1)
                .or_else(|| files.first())
                .cloned()
                .ok_or("This modpack has no files.")?
        }
    };
    let url = match file.download_url.clone() {
        Some(u) => u,
        None => {
            let page = website(&app, mod_id).await;
            return Err(format!(
                "The author of this modpack doesn't allow downloads through launchers. Download the .zip from its CurseForge page and import that file instead. {page}"
            ));
        }
    };
    let dir = crate::data_dir(&app)?.join("modpacks");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let c = client()?;
    download_to(&c, &url, &file, &dir).await?;
    let pack = dir.join(&file.file_name);
    match read_manifest(&pack) {
        Ok(manifest) => start_cf_pack(app, pack, manifest, true).await,
        Err(e) => {
            let _ = fs::remove_file(&pack);
            Err(e)
        }
    }
}
