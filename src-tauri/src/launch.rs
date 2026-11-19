//! Launching Minecraft with an offline account.
//!
//! Files are cached under the app data folder (libraries, assets, versions), so once a
//! version has been launched once, it starts again without any internet connection.

use crate::{data_dir, instances, runtime};
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex, OnceLock,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{process::Command, sync::Semaphore, task::JoinSet};

const MANIFEST: &str = "https://launchermeta.mojang.com/mc/game/version_manifest_v2.json";
const ASSET_BASE: &str = "https://resources.download.minecraft.net";
const LIB_BASE: &str = "https://libraries.minecraft.net/";
const FABRIC_META: &str = "https://meta.fabricmc.net/v2/versions/loader";
const QUILT_META: &str = "https://meta.quiltmc.org/v3/versions/loader";
const NEOFORGE_API: &str = "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge";
const NEOFORGE_MAVEN: &str = "https://maven.neoforged.net/releases/net/neoforged/neoforge";
const FORGE_PROMOS: &str = "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json";
const FORGE_MAVEN: &str = "https://maven.minecraftforge.net/net/minecraftforge/forge";
const USER_AGENT: &str = "MinimalMc/1.0.0";

// ───────────────────────── Status (polled by the UI) ─────────────────────────

#[derive(Serialize, Clone, Default)]
pub struct LaunchStatus {
    pub stage: String,
    pub done: u64,
    pub total: u64,
    /// Preparing or downloading.
    pub busy: bool,
    /// The game process is alive.
    pub running: bool,
    pub error: Option<String>,
    /// Something worth telling the player that didn't stop the launch (e.g. the skin couldn't be applied).
    pub notice: Option<String>,
}

fn table() -> &'static Mutex<HashMap<u64, LaunchStatus>> {
    static T: OnceLock<Mutex<HashMap<u64, LaunchStatus>>> = OnceLock::new();
    T.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(crate) fn set(id: u64, f: impl FnOnce(&mut LaunchStatus)) {
    if let Ok(mut t) = table().lock() {
        f(t.entry(id).or_default());
    }
}

#[tauri::command]
pub fn launch_status(id: u64) -> LaunchStatus {
    table().lock().ok().and_then(|t| t.get(&id).cloned()).unwrap_or_default()
}

// ───────────────────────── Mojang / Fabric metadata ─────────────────────────

#[derive(Deserialize)]
struct Manifest {
    versions: Vec<ManifestEntry>,
}

#[derive(Deserialize)]
struct ManifestEntry {
    id: String,
    url: String,
}

#[derive(Deserialize, Clone)]
struct Artifact {
    #[serde(default)]
    path: Option<String>,
    url: String,
    #[serde(default)]
    sha1: Option<String>,
    #[serde(default)]
    size: u64,
}

#[derive(Deserialize, Clone)]
struct LibDownloads {
    #[serde(default)]
    artifact: Option<Artifact>,
    #[serde(default)]
    classifiers: Option<HashMap<String, Artifact>>,
}

#[derive(Deserialize, Clone)]
struct RuleOs {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arch: Option<String>,
}

#[derive(Deserialize, Clone)]
struct Rule {
    action: String,
    #[serde(default)]
    os: Option<RuleOs>,
    #[serde(default)]
    features: Option<HashMap<String, bool>>,
}

#[derive(Deserialize, Clone)]
struct Extract {
    #[serde(default)]
    exclude: Vec<String>,
}

#[derive(Deserialize, Clone)]
struct Lib {
    name: String,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    downloads: Option<LibDownloads>,
    #[serde(default)]
    rules: Vec<Rule>,
    #[serde(default)]
    natives: Option<HashMap<String, String>>,
    #[serde(default)]
    extract: Option<Extract>,
}

#[derive(Deserialize, Clone)]
struct Arguments {
    #[serde(default)]
    game: Vec<serde_json::Value>,
    #[serde(default)]
    jvm: Vec<serde_json::Value>,
}

#[derive(Deserialize, Clone)]
struct AssetIndexRef {
    id: String,
    url: String,
    #[serde(default)]
    sha1: Option<String>,
    #[serde(default)]
    size: u64,
}

#[derive(Deserialize, Clone)]
struct VDownloads {
    #[serde(default)]
    client: Option<Artifact>,
}

#[derive(Deserialize, Clone)]
struct JavaVer {
    #[serde(rename = "majorVersion")]
    major_version: u32,
    #[serde(default)]
    component: Option<String>,
}

#[derive(Deserialize, Clone)]
struct LogFile {
    id: String,
    url: String,
    #[serde(default)]
    sha1: Option<String>,
    #[serde(default)]
    size: u64,
}

#[derive(Deserialize, Clone)]
struct LogClient {
    argument: String,
    file: LogFile,
}

#[derive(Deserialize, Clone)]
struct Logging {
    #[serde(default)]
    client: Option<LogClient>,
}

#[derive(Deserialize, Clone)]
struct VJson {
    id: String,
    #[serde(rename = "mainClass", default)]
    main_class: Option<String>,
    #[serde(rename = "assetIndex", default)]
    asset_index: Option<AssetIndexRef>,
    #[serde(default)]
    downloads: Option<VDownloads>,
    #[serde(default)]
    libraries: Vec<Lib>,
    #[serde(default)]
    arguments: Option<Arguments>,
    #[serde(rename = "minecraftArguments", default)]
    minecraft_arguments: Option<String>,
    #[serde(rename = "javaVersion", default)]
    java_version: Option<JavaVer>,
    #[serde(rename = "type", default)]
    kind: Option<String>,
    #[serde(default)]
    logging: Option<Logging>,
}

#[derive(Deserialize)]
struct LoaderEntry {
    loader: LoaderInfo,
}

#[derive(Deserialize)]
struct LoaderInfo {
    version: String,
    #[serde(default)]
    stable: bool,
}

#[derive(Deserialize)]
struct ForgePromos {
    promos: HashMap<String, String>,
}

#[derive(Deserialize)]
struct NeoVersions {
    versions: Vec<String>,
}

#[derive(Deserialize)]
struct AssetIdx {
    objects: HashMap<String, AssetObj>,
    #[serde(rename = "virtual", default)]
    is_virtual: bool,
    #[serde(default)]
    map_to_resources: bool,
}

#[derive(Deserialize)]
struct AssetObj {
    hash: String,
    size: u64,
}

pub(crate) async fn get_text(c: &reqwest::Client, url: &str) -> Result<String, String> {
    c.get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .text()
        .await
        .map_err(|e| e.to_string())
}

fn save_text(path: &Path, text: &str) {
    if let Some(p) = path.parent() {
        let _ = fs::create_dir_all(p);
    }
    let _ = fs::write(path, text);
}

/// Uses the cached copy when there is one, so launching works without internet.
async fn load_vanilla(c: &reqwest::Client, root: &Path, ver: &str) -> Result<VJson, String> {
    let file = root.join("versions").join(ver).join(format!("{ver}.json"));
    if let Ok(s) = fs::read_to_string(&file) {
        if let Ok(v) = serde_json::from_str::<VJson>(&s) {
            return Ok(v);
        }
    }
    let list = get_text(c, MANIFEST)
        .await
        .map_err(|e| format!("Couldn't download Minecraft's version list. Connect to the internet for the first launch of a version. ({e})"))?;
    let m: Manifest = serde_json::from_str(&list).map_err(|e| e.to_string())?;
    let entry = m
        .versions
        .into_iter()
        .find(|v| v.id == ver)
        .ok_or_else(|| format!("Minecraft {ver} isn't in Mojang's version list"))?;
    let text = get_text(c, &entry.url).await?;
    let v: VJson = serde_json::from_str(&text).map_err(|e| format!("Unreadable version file: {e}"))?;
    save_text(&file, &text);
    Ok(v)
}

/// Fabric and Quilt share the same metadata format. `wanted` is an exact loader version, or empty for the newest stable one.
/// A copy is cached on disk, so launching again works without internet.
async fn load_profile(c: &reqwest::Client, root: &Path, kind: &str, mc: &str, wanted: &str) -> Result<VJson, String> {
    let (base, label) = if kind == "quilt" { (QUILT_META, "Quilt") } else { (FABRIC_META, "Fabric") };
    let cache = if wanted.is_empty() { format!("{kind}-{mc}.json") } else { format!("{kind}-{mc}-{wanted}.json") };
    let file = root.join("versions").join(cache);
    if let Ok(s) = fs::read_to_string(&file) {
        if let Ok(v) = serde_json::from_str::<VJson>(&s) {
            return Ok(v);
        }
    }
    let version = if wanted.is_empty() {
        let list_text = get_text(c, &format!("{base}/{mc}"))
            .await
            .map_err(|e| format!("Couldn't reach the {label} servers. Connect to the internet for the first launch. ({e})"))?;
        let list: Vec<LoaderEntry> = serde_json::from_str(&list_text).map_err(|e| e.to_string())?;
        let pick = list
            .iter()
            .find(|l| l.loader.stable)
            .or_else(|| list.iter().find(|l| !l.loader.version.contains('-')))
            .or_else(|| list.first())
            .ok_or_else(|| format!("{label} doesn't support Minecraft {mc}"))?;
        pick.loader.version.clone()
    } else {
        wanted.to_string()
    };
    let text = get_text(c, &format!("{base}/{mc}/{version}/profile/json"))
        .await
        .map_err(|e| format!("Couldn't download the {label} profile ({e})"))?;
    let v: VJson = serde_json::from_str(&text).map_err(|e| format!("Unreadable {label} profile: {e}"))?;
    save_text(&file, &text);
    Ok(v)
}

// ───────────────────────── NeoForge ─────────────────────────
//
// NeoForge ships an installer that patches Minecraft. It is run once per NeoForge version
// (needs internet), and the result is cached so later launches work offline.

/// "1.21.4" -> "21.4.", "1.21" -> "21.0.", "26.1" -> "26.1." (the start of matching NeoForge versions)
fn neoforge_prefix(mc: &str) -> Option<String> {
    let mut it = mc.split('.');
    let first = it.next()?;
    if first == "1" {
        let minor = it.next()?;
        let patch = it.next().unwrap_or("0");
        Some(format!("{minor}.{patch}."))
    } else {
        Some(format!("{mc}."))
    }
}

async fn neoforge_pick(c: &reqwest::Client, mc: &str) -> Result<String, String> {
    let prefix = neoforge_prefix(mc).ok_or("Unrecognised Minecraft version")?;
    let text = get_text(c, NEOFORGE_API)
        .await
        .map_err(|e| format!("Couldn't reach the NeoForge servers. Connect to the internet for the first launch. ({e})"))?;
    let list: NeoVersions = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    // (build numbers, is a stable release, version text). Stable beats beta, then the highest build wins.
    let mut cands: Vec<(Vec<u64>, bool, String)> = list
        .versions
        .into_iter()
        .filter(|v| v.starts_with(&prefix))
        .map(|v| {
            let stable = !v.contains('-');
            let nums: Vec<u64> = v.split(|c: char| c == '.' || c == '-').filter_map(|p| p.parse::<u64>().ok()).collect();
            (nums, stable, v)
        })
        .collect();
    cands.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));
    cands
        .pop()
        .map(|c| c.2)
        .ok_or_else(|| format!("There's no NeoForge release for Minecraft {mc} that MinimalMc can use"))
}

/// Returns the NeoForge profile and the exact version used. `wanted` empty means "newest".
async fn ensure_neoforge(
    c: &reqwest::Client,
    root: &Path,
    java: &Path,
    id: u64,
    mc: &str,
    wanted: &str,
) -> Result<(VJson, String), String> {
    let ver = if wanted.is_empty() { neoforge_pick(c, mc).await? } else { wanted.to_string() };
    let vfile = root.join("versions").join(format!("neoforge-{ver}")).join(format!("neoforge-{ver}.json"));
    if let Ok(s) = fs::read_to_string(&vfile) {
        if let Ok(v) = serde_json::from_str::<VJson>(&s) {
            return Ok((v, ver));
        }
    }

    // Download the installer (its checksum is published next to it).
    let installers = root.join("installers");
    let installer = installers.join(format!("neoforge-{ver}-installer.jar"));
    let url = format!("{NEOFORGE_MAVEN}/{ver}/neoforge-{ver}-installer.jar");
    let sha1 = get_text(c, &format!("{url}.sha1"))
        .await
        .ok()
        .map(|t| t.trim().to_string())
        .filter(|t| t.len() == 40);
    download_all(c, id, "Downloading NeoForge installer", vec![Job { url, path: installer.clone(), sha1, size: 0 }]).await?;

    // The installer expects a launcher_profiles.json next to the versions folder.
    let profiles = root.join("launcher_profiles.json");
    if !profiles.exists() {
        fs::write(&profiles, r#"{"profiles":{},"version":3}"#).map_err(|e| e.to_string())?;
    }

    set(id, |s| {
        s.stage = "Installing NeoForge".into();
        s.done = 0;
        s.total = 0;
    });
    let log_path = installers.join(format!("neoforge-{ver}-install.log"));
    let mut failure = String::new();
    for flag in ["--installClient", "--install-client"] {
        let log = fs::File::create(&log_path).map_err(|e| e.to_string())?;
        let log2 = log.try_clone().map_err(|e| e.to_string())?;
        let mut cmd = Command::new(java);
        cmd.arg("-jar")
            .arg(&installer)
            .arg(flag)
            .arg(root)
            .current_dir(root)
            .stdin(Stdio::null())
            .stdout(Stdio::from(log))
            .stderr(Stdio::from(log2));
        #[cfg(windows)]
        cmd.creation_flags(0x0800_0000);
        let status = cmd
            .status()
            .await
            .map_err(|e| format!("Couldn't run the NeoForge installer: {e}"))?;
        if status.success() && vfile.is_file() {
            failure.clear();
            break;
        }
        failure = format!("exit code {}", status.code().unwrap_or(-1));
    }
    if !failure.is_empty() || !vfile.is_file() {
        return Err(format!(
            "The NeoForge installer failed ({failure}). Details are in {}",
            log_path.display()
        ));
    }
    let text = fs::read_to_string(&vfile).map_err(|e| e.to_string())?;
    let v: VJson = serde_json::from_str(&text).map_err(|e| format!("Unreadable NeoForge profile: {e}"))?;
    Ok((v, ver))
}

// ───────────────────────── Forge ─────────────────────────
//
// Forge works like NeoForge: its official installer is run once per Forge version (needs internet)
// and the result is cached, so later launches work offline.

/// The version JSON a Forge installer wrote: a folder in `versions` that has "forge" and the Forge
/// version in its name (for example "1.20.1-forge-47.3.0").
fn find_forge_json(root: &Path, ver: &str) -> Option<PathBuf> {
    let rd = fs::read_dir(root.join("versions")).ok()?;
    for e in rd.filter_map(|e| e.ok()) {
        let name = e.file_name().to_string_lossy().to_string();
        let lower = name.to_lowercase();
        if lower.contains("forge") && !lower.starts_with("neoforge") && name.contains(ver) {
            let json = e.path().join(format!("{name}.json"));
            if json.is_file() {
                return Some(json);
            }
        }
    }
    None
}

/// The recommended Forge version for a Minecraft version, or the latest if there's no recommended one.
async fn forge_pick(c: &reqwest::Client, mc: &str) -> Result<String, String> {
    let text = get_text(c, FORGE_PROMOS)
        .await
        .map_err(|e| format!("Couldn't reach the Forge servers. Connect to the internet for the first launch. ({e})"))?;
    let p: ForgePromos = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    p.promos
        .get(&format!("{mc}-recommended"))
        .or_else(|| p.promos.get(&format!("{mc}-latest")))
        .cloned()
        .ok_or_else(|| format!("There's no Forge release for Minecraft {mc}"))
}

/// Returns the Forge profile and the exact version used. `wanted` empty means "recommended".
async fn ensure_forge(
    c: &reqwest::Client,
    root: &Path,
    java: &Path,
    id: u64,
    mc: &str,
    wanted: &str,
) -> Result<(VJson, String), String> {
    let ver = if wanted.is_empty() { forge_pick(c, mc).await? } else { wanted.to_string() };
    if let Some(json) = find_forge_json(root, &ver) {
        if let Ok(s) = fs::read_to_string(&json) {
            if let Ok(v) = serde_json::from_str::<VJson>(&s) {
                return Ok((v, ver));
            }
        }
    }

    // A few old Minecraft versions put the Minecraft version at the end of the file name as well.
    let mut found: Option<(String, String)> = None;
    for url in [
        format!("{FORGE_MAVEN}/{mc}-{ver}/forge-{mc}-{ver}-installer.jar"),
        format!("{FORGE_MAVEN}/{mc}-{ver}-{mc}/forge-{mc}-{ver}-{mc}-installer.jar"),
    ] {
        if let Ok(t) = get_text(c, &format!("{url}.sha1")).await {
            found = Some((url, t.trim().to_string()));
            break;
        }
    }
    let (url, sum) = found.ok_or_else(|| format!("Couldn't find the Forge {ver} installer for Minecraft {mc}"))?;
    let sha1 = if sum.len() == 40 { Some(sum) } else { None };
    let installers = root.join("installers");
    let installer = installers.join(format!("forge-{mc}-{ver}-installer.jar"));
    download_all(c, id, "Downloading Forge installer", vec![Job { url, path: installer.clone(), sha1, size: 0 }]).await?;

    let profiles = root.join("launcher_profiles.json");
    if !profiles.exists() {
        fs::write(&profiles, r#"{"profiles":{},"version":3}"#).map_err(|e| e.to_string())?;
    }

    set(id, |s| {
        s.stage = "Installing Forge".into();
        s.done = 0;
        s.total = 0;
    });
    let log_path = installers.join(format!("forge-{mc}-{ver}-install.log"));
    let mut failure = String::new();
    for flag in ["--installClient", "--install-client"] {
        let log = fs::File::create(&log_path).map_err(|e| e.to_string())?;
        let log2 = log.try_clone().map_err(|e| e.to_string())?;
        let mut cmd = Command::new(java);
        cmd.arg("-jar")
            .arg(&installer)
            .arg(flag)
            .arg(root)
            .current_dir(root)
            .stdin(Stdio::null())
            .stdout(Stdio::from(log))
            .stderr(Stdio::from(log2));
        #[cfg(windows)]
        cmd.creation_flags(0x0800_0000);
        let status = cmd
            .status()
            .await
            .map_err(|e| format!("Couldn't run the Forge installer: {e}"))?;
        if status.success() && find_forge_json(root, &ver).is_some() {
            failure.clear();
            break;
        }
        failure = format!("exit code {}", status.code().unwrap_or(-1));
    }
    let Some(json) = find_forge_json(root, &ver) else {
        return Err(format!("The Forge installer failed ({failure}). Details are in {}", log_path.display()));
    };
    if !failure.is_empty() {
        return Err(format!("The Forge installer failed ({failure}). Details are in {}", log_path.display()));
    }
    let text = fs::read_to_string(&json).map_err(|e| e.to_string())?;
    let v: VJson = serde_json::from_str(&text).map_err(|e| format!("Unreadable Forge profile: {e}"))?;
    Ok((v, ver))
}

/// Remembers the exact loader version an instance was launched with, so it stays the same afterwards.
fn pin_loader_version(app: &tauri::AppHandle, id: u64, version: &str) {
    if let Ok(mut list) = instances::load(app) {
        if let Some(i) = list.iter_mut().find(|i| i.id == id) {
            i.loader_version = version.to_string();
        }
        let _ = instances::save(app, &list);
    }
}

fn lib_key(name: &str) -> String {
    let p: Vec<&str> = name.split(':').collect();
    format!("{}:{}:{}", p.first().copied().unwrap_or(""), p.get(1).copied().unwrap_or(""), p.get(3).copied().unwrap_or(""))
}

/// Layers a loader profile (child) on top of the vanilla version (parent).
fn merge(parent: VJson, child: VJson) -> VJson {
    let mut libs: Vec<Lib> = vec![];
    let mut seen: HashSet<String> = HashSet::new();
    for l in child.libraries.iter().chain(parent.libraries.iter()) {
        if seen.insert(lib_key(&l.name)) {
            libs.push(l.clone());
        }
    }
    let mut jvm = parent.arguments.as_ref().map(|a| a.jvm.clone()).unwrap_or_default();
    let mut game = parent.arguments.as_ref().map(|a| a.game.clone()).unwrap_or_default();
    if let Some(a) = &child.arguments {
        jvm.extend(a.jvm.iter().cloned());
        game.extend(a.game.iter().cloned());
    }
    let has_args = parent.arguments.is_some() || child.arguments.is_some();
    VJson {
        id: child.id,
        main_class: child.main_class.or(parent.main_class),
        asset_index: child.asset_index.or(parent.asset_index),
        downloads: child.downloads.or(parent.downloads),
        libraries: libs,
        arguments: if has_args { Some(Arguments { game, jvm }) } else { None },
        minecraft_arguments: child.minecraft_arguments.or(parent.minecraft_arguments),
        java_version: child.java_version.or(parent.java_version),
        kind: child.kind.or(parent.kind),
        logging: child.logging.or(parent.logging),
    }
}

// ───────────────────────── Rules ─────────────────────────

fn os_name() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    }
}

fn rule_matches(r: &Rule) -> bool {
    if let Some(os) = &r.os {
        if let Some(n) = &os.name {
            if n != os_name() {
                return false;
            }
        }
        if let Some(a) = &os.arch {
            if a == "x86" && !cfg!(target_pointer_width = "32") {
                return false;
            }
            if a == "arm64" && !cfg!(target_arch = "aarch64") {
                return false;
            }
        }
    }
    // None of the optional launcher features (demo mode, custom resolution, ...) are enabled.
    if let Some(f) = &r.features {
        if f.values().any(|v| *v) {
            return false;
        }
    }
    true
}

fn rules_allow(rules: &[Rule]) -> bool {
    if rules.is_empty() {
        return true;
    }
    let mut ok = false;
    for r in rules {
        if rule_matches(r) {
            ok = r.action == "allow";
        }
    }
    ok
}

// ───────────────────────── Downloads ─────────────────────────

#[derive(Clone)]
pub(crate) struct Job {
    pub(crate) url: String,
    pub(crate) path: PathBuf,
    pub(crate) sha1: Option<String>,
    pub(crate) size: u64,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// While a repair runs, files are checked against their checksums, not just their size.
static STRICT: AtomicBool = AtomicBool::new(false);

fn already_ok(j: &Job) -> bool {
    let present = match fs::metadata(&j.path) {
        Ok(m) => m.is_file() && (j.size == 0 || m.len() == j.size),
        Err(_) => false,
    };
    if !present {
        return false;
    }
    if STRICT.load(Ordering::Relaxed) {
        if let Some(h) = &j.sha1 {
            return fs::read(&j.path)
                .map(|b| hex(&Sha1::digest(&b[..])).eq_ignore_ascii_case(h))
                .unwrap_or(false);
        }
    }
    true
}

async fn fetch(c: &reqwest::Client, j: &Job) -> Result<(), String> {
    let mut last = String::new();
    for _ in 0..3 {
        match c.get(&j.url).send().await.and_then(|r| r.error_for_status()) {
            Ok(r) => match r.bytes().await {
                Ok(b) => {
                    if let Some(h) = &j.sha1 {
                        let got = hex(&Sha1::digest(&b[..]));
                        if !got.eq_ignore_ascii_case(h) {
                            last = format!("checksum mismatch for {}", j.url);
                            continue;
                        }
                    }
                    if let Some(p) = j.path.parent() {
                        fs::create_dir_all(p).map_err(|e| e.to_string())?;
                    }
                    let mut t = j.path.clone().into_os_string();
                    t.push(".part");
                    let tmp = PathBuf::from(t);
                    fs::write(&tmp, &b).map_err(|e| e.to_string())?;
                    fs::rename(&tmp, &j.path).map_err(|e| e.to_string())?;
                    return Ok(());
                }
                Err(e) => last = e.to_string(),
            },
            Err(e) => last = e.to_string(),
        }
        tokio::time::sleep(Duration::from_millis(400)).await;
    }
    Err(format!("Download failed ({}): {last}", j.url))
}

pub(crate) async fn download_all(c: &reqwest::Client, id: u64, stage: &str, jobs: Vec<Job>) -> Result<(), String> {
    let mut seen: HashSet<PathBuf> = HashSet::new();
    let todo: Vec<Job> = jobs.into_iter().filter(|j| seen.insert(j.path.clone()) && !already_ok(j)).collect();
    set(id, |s| {
        s.stage = stage.to_string();
        s.done = 0;
        s.total = todo.len() as u64;
    });
    if todo.is_empty() {
        return Ok(());
    }
    let done = Arc::new(AtomicU64::new(0));
    let sem = Arc::new(Semaphore::new(16));
    let mut tasks = JoinSet::new();
    for j in todo {
        let c = c.clone();
        let sem = sem.clone();
        let done = done.clone();
        tasks.spawn(async move {
            let _permit = sem.acquire_owned().await.map_err(|e| e.to_string())?;
            fetch(&c, &j).await?;
            let n = done.fetch_add(1, Ordering::Relaxed) + 1;
            set(id, |s| s.done = n);
            Ok::<(), String>(())
        });
    }
    while let Some(r) = tasks.join_next().await {
        r.map_err(|e| e.to_string())??;
    }
    Ok(())
}

// ───────────────────────── Libraries & natives ─────────────────────────

fn maven_path(name: &str) -> Option<String> {
    let (coords, ext) = match name.split_once('@') {
        Some((c, e)) => (c, e),
        None => (name, "jar"),
    };
    let p: Vec<&str> = coords.split(':').collect();
    if p.len() < 3 {
        return None;
    }
    let group = p[0].replace('.', "/");
    let (a, v) = (p[1], p[2]);
    let file = if p.len() > 3 { format!("{a}-{v}-{}.{ext}", p[3]) } else { format!("{a}-{v}.{ext}") };
    Some(format!("{group}/{a}/{v}/{file}"))
}

struct Plan {
    jobs: Vec<Job>,
    classpath: Vec<PathBuf>,
    natives: Vec<(PathBuf, Vec<String>)>,
}

fn plan_libraries(libs: &[Lib], root: &Path) -> Plan {
    let mut plan = Plan { jobs: vec![], classpath: vec![], natives: vec![] };
    for l in libs {
        if !rules_allow(&l.rules) {
            continue;
        }
        // Older versions ship native code as separate "classifier" jars that get unpacked.
        if let Some(n) = &l.natives {
            if let Some(cls) = n.get(os_name()) {
                let cls = cls.replace("${arch}", "64");
                let a = l.downloads.as_ref().and_then(|d| d.classifiers.as_ref()).and_then(|c| c.get(&cls));
                if let Some(a) = a {
                    let rel = a
                        .path
                        .clone()
                        .unwrap_or_else(|| format!("natives/{}", a.url.rsplit('/').next().unwrap_or("native.jar")));
                    let path = root.join("libraries").join(&rel);
                    plan.jobs.push(Job { url: a.url.clone(), path: path.clone(), sha1: a.sha1.clone(), size: a.size });
                    plan.natives.push((path, l.extract.as_ref().map(|e| e.exclude.clone()).unwrap_or_default()));
                }
            }
            if l.downloads.as_ref().and_then(|d| d.artifact.as_ref()).is_none() {
                continue;
            }
        }
        let art = l.downloads.as_ref().and_then(|d| d.artifact.clone());
        let (url, rel, sha1, size) = match art {
            Some(a) => {
                let Some(rel) = a.path.clone().or_else(|| maven_path(&l.name)) else { continue };
                (a.url, rel, a.sha1, a.size)
            }
            None => {
                let Some(rel) = maven_path(&l.name) else { continue };
                let base = l.url.clone().unwrap_or_else(|| LIB_BASE.to_string());
                let base = if base.ends_with('/') { base } else { format!("{base}/") };
                (format!("{base}{rel}"), rel, None, 0)
            }
        };
        let path = root.join("libraries").join(&rel);
        if !url.is_empty() {
            plan.jobs.push(Job { url, path: path.clone(), sha1, size });
        }
        plan.classpath.push(path.clone());
        // Newer versions list the native jars as normal libraries (name ends in ":natives-windows").
        if l.name.split(':').nth(3).map(|c| c.starts_with("natives-")).unwrap_or(false) {
            plan.natives.push((path, vec![]));
        }
    }
    plan
}

fn extract_natives(natives: &[(PathBuf, Vec<String>)], dest: &Path) -> Result<(), String> {
    let _ = fs::remove_dir_all(dest);
    fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    for (jar, exclude) in natives {
        let f = fs::File::open(jar).map_err(|e| format!("{}: {e}", jar.display()))?;
        let mut zip = zip::ZipArchive::new(f).map_err(|e| e.to_string())?;
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
            if entry.is_dir() {
                continue;
            }
            let name = entry.name().to_string();
            if name.starts_with("META-INF/") || exclude.iter().any(|x| name.starts_with(x.as_str())) {
                continue;
            }
            let lower = name.to_lowercase();
            if !(lower.ends_with(".dll") || lower.ends_with(".so") || lower.ends_with(".dylib") || lower.ends_with(".jnilib")) {
                continue;
            }
            let Some(file_name) = Path::new(&name).file_name() else { continue };
            let mut out = fs::File::create(dest.join(file_name)).map_err(|e| e.to_string())?;
            std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

// ───────────────────────── Java ─────────────────────────

pub(crate) fn java_exe() -> &'static str {
    if cfg!(windows) {
        "java.exe"
    } else {
        "java"
    }
}

pub(crate) fn java_candidates() -> Vec<PathBuf> {
    let exe = java_exe();
    let mut out: Vec<PathBuf> = vec![];
    if let Ok(h) = std::env::var("JAVA_HOME") {
        out.push(PathBuf::from(h).join("bin").join(exe));
    }
    if let Some(p) = std::env::var_os("PATH") {
        out.extend(std::env::split_paths(&p).map(|d| d.join(exe)));
    }
    if cfg!(windows) {
        let mut bases: Vec<PathBuf> = [
            "C:\\Program Files\\Java",
            "C:\\Program Files\\Eclipse Adoptium",
            "C:\\Program Files\\Microsoft",
            "C:\\Program Files\\Zulu",
            "C:\\Program Files\\Amazon Corretto",
            "C:\\Program Files\\BellSoft",
            "C:\\Program Files\\Semeru",
            "C:\\Program Files (x86)\\Java",
        ]
        .iter()
        .map(|s| PathBuf::from(*s))
        .collect();
        // Runtimes downloaded earlier by the official Minecraft launcher.
        if let Some(a) = std::env::var_os("APPDATA") {
            let rt = PathBuf::from(a).join(".minecraft").join("runtime");
            if let Ok(rd) = fs::read_dir(&rt) {
                for comp in rd.filter_map(|e| e.ok()) {
                    bases.push(comp.path().join("windows-x64").join(comp.file_name()));
                }
            }
        }
        for b in bases {
            out.push(b.join("bin").join(exe));
            if let Ok(rd) = fs::read_dir(&b) {
                for child in rd.filter_map(|e| e.ok()) {
                    out.push(child.path().join("bin").join(exe));
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out.retain(|p| p.is_file());
    out
}

/// (major version, full version text) of a Java executable, e.g. (21, "21.0.5").
pub(crate) async fn java_info(path: &Path) -> Option<(u32, String)> {
    let mut cmd = Command::new(path);
    cmd.arg("-version");
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000);
    let o = cmd.output().await.ok()?;
    let text = String::from_utf8_lossy(&o.stderr).to_string();
    let v = text.split('"').nth(1)?.to_string();
    let mut parts = v.split(|c: char| c == '.' || c == '_' || c == '-' || c == '+');
    let first: u32 = parts.next()?.parse().ok()?;
    let major = if first == 1 { parts.next()?.parse().ok()? } else { first };
    Some((major, v.clone()))
}

async fn pick_java(required: u32) -> Result<PathBuf, String> {
    let mut found: Vec<(u32, PathBuf)> = vec![];
    for p in java_candidates() {
        if let Some((m, _)) = java_info(&p).await {
            found.push((m, p));
        }
    }
    // Old Minecraft versions only run on Java 8; newer ones need at least the version they were built for.
    let fit = if required <= 8 {
        found.iter().find(|(m, _)| *m == 8)
    } else {
        found.iter().filter(|(m, _)| *m >= required).min_by_key(|(m, _)| *m)
    };
    fit.map(|(_, p)| p.clone()).ok_or_else(|| {
        format!("This version of Minecraft needs Java {required}, but it wasn't found on this computer. Install it (for example from adoptium.net) and try again.")
    })
}

/// Picks the Java for a launch:
/// 1. a runtime MinimalMc already downloaded,
/// 2. a compatible Java that's installed on this computer,
/// 3. otherwise downloads the one Mojang ships for this Minecraft version.
async fn resolve_java(c: &reqwest::Client, root: &Path, id: u64, component: &str, required: u32) -> Result<PathBuf, String> {
    if let Some(p) = runtime::cached(root, component) {
        return Ok(p);
    }
    if let Ok(p) = pick_java(required).await {
        return Ok(p);
    }
    runtime::install(c, root, id, component).await.map_err(|e| {
        format!("This version of Minecraft needs Java {required}. It isn't installed, and downloading it failed: {e}")
    })
}

// ───────────────────────── Arguments ─────────────────────────

fn expand(vals: &[serde_json::Value]) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for v in vals {
        match v {
            serde_json::Value::String(s) => out.push(s.clone()),
            serde_json::Value::Object(o) => {
                let rules: Vec<Rule> = o
                    .get("rules")
                    .and_then(|r| serde_json::from_value(r.clone()).ok())
                    .unwrap_or_default();
                if !rules_allow(&rules) {
                    continue;
                }
                match o.get("value") {
                    Some(serde_json::Value::String(s)) => out.push(s.clone()),
                    Some(serde_json::Value::Array(a)) => out.extend(a.iter().filter_map(|x| x.as_str().map(String::from))),
                    _ => {}
                }
            }
            _ => {}
        }
    }
    out
}

fn subst(s: &str, vars: &HashMap<&str, String>) -> String {
    let mut out = s.to_string();
    for (k, v) in vars {
        out = out.replace(&format!("${{{k}}}"), v);
    }
    out
}

/// Same algorithm Java uses for `UUID.nameUUIDFromBytes("OfflinePlayer:<name>")`, so offline
/// worlds keep the same player data as other launchers' offline accounts.
fn offline_uuid(name: &str) -> String {
    let mut b = md5::Md5::digest(format!("OfflinePlayer:{name}").as_bytes()).to_vec();
    b[6] = (b[6] & 0x0f) | 0x30;
    b[8] = (b[8] & 0x3f) | 0x80;
    hex(&b)
}

fn add_playtime(app: &tauri::AppHandle, id: u64, secs: u64) {
    if let Ok(mut list) = instances::load(app) {
        if let Some(i) = list.iter_mut().find(|i| i.id == id) {
            i.played += secs;
        }
        let _ = instances::save(app, &list);
    }
}

fn mark_played(app: &tauri::AppHandle, id: u64) {
    if let Ok(mut list) = instances::load(app) {
        if let Some(i) = list.iter_mut().find(|i| i.id == id) {
            i.last = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0).to_string();
        }
        let _ = instances::save(app, &list);
    }
}

// ───────────────────────── Offline skin ─────────────────────────
//
// Offline accounts can't download skins from Mojang. Instead the chosen skin is written into a
// small resource pack that replaces the game's built-in default player skins, and that pack is
// switched on for the instance. It is rebuilt on every launch.

const SKIN_PACK: &str = "minimalmc-skin";

/// Before 1.13 the pack list in options.txt used plain folder names instead of "file/<name>".
fn legacy_pack_ids(ver: &str) -> bool {
    let mut it = ver.split('.');
    let major = it.next().and_then(|a| a.parse::<u32>().ok());
    let minor = it.next().and_then(|b| b.parse::<u32>().ok());
    matches!((major, minor), (Some(1), Some(m)) if m < 13)
}

fn fallback_pack_format(ver: &str) -> u64 {
    if ver.split('.').next() != Some("1") {
        return 15;
    }
    let minor = ver.split('.').nth(1).and_then(|m| m.parse::<u64>().ok()).unwrap_or(99);
    match minor {
        0..=8 => 1,
        9..=10 => 2,
        11..=12 => 3,
        13..=14 => 4,
        _ => 15,
    }
}

/// The resource pack format this Minecraft version expects, read from its own jar.
fn jar_pack_format(jar: &Path, ver: &str) -> u64 {
    let read = || -> Option<u64> {
        let f = fs::File::open(jar).ok()?;
        let mut z = zip::ZipArchive::new(f).ok()?;
        let mut e = z.by_name("version.json").ok()?;
        let mut text = String::new();
        std::io::Read::read_to_string(&mut e, &mut text).ok()?;
        let v: serde_json::Value = serde_json::from_str(&text).ok()?;
        let pv = v.get("pack_version")?;
        pv.get("resource")
            .and_then(|x| x.as_u64())
            .or_else(|| pv.get("resource_major").and_then(|x| x.as_u64()))
    };
    read().unwrap_or_else(|| fallback_pack_format(ver))
}

/// Old skins are 64x32. The game expects 64x64, so the missing left arm and leg are mirrored in.
fn upgrade_legacy(old: &image::RgbaImage) -> image::RgbaImage {
    let mut img = image::RgbaImage::new(64, 64);
    for y in 0..32 {
        for x in 0..64 {
            img.put_pixel(x, y, *old.get_pixel(x, y));
        }
    }
    // (source x, source y, destination x, destination y, width, height), each mirrored left to right
    let copies: [(u32, u32, u32, u32, u32, u32); 12] = [
        (4, 16, 20, 48, 4, 4),
        (8, 16, 24, 48, 4, 4),
        (0, 20, 24, 52, 4, 12),
        (4, 20, 20, 52, 4, 12),
        (8, 20, 16, 52, 4, 12),
        (12, 20, 28, 52, 4, 12),
        (44, 16, 36, 48, 4, 4),
        (48, 16, 40, 48, 4, 4),
        (40, 20, 40, 52, 4, 12),
        (44, 20, 36, 52, 4, 12),
        (48, 20, 32, 52, 4, 12),
        (52, 20, 44, 52, 4, 12),
    ];
    for (sx, sy, dx, dy, w, h) in copies {
        for j in 0..h {
            for i in 0..w {
                let p = *old.get_pixel(sx + i, sy + j);
                img.put_pixel(dx + (w - 1 - i), dy + j, p);
            }
        }
    }
    img
}

fn prepare_skin(src: &Path) -> Result<image::RgbaImage, String> {
    let img = image::open(src).map_err(|e| format!("couldn't read the skin file ({e})"))?.to_rgba8();
    match (img.width(), img.height()) {
        (64, 64) => Ok(img),
        (64, 32) => Ok(upgrade_legacy(&img)),
        (w, h) => Err(format!("skins must be 64x64 (or the old 64x32), but this one is {w}x{h}")),
    }
}

/// Adds or removes the skin pack in the instance's options.txt (the game's own settings file).
fn set_pack_enabled(game_dir: &Path, id: &str, enabled: bool) -> Result<(), String> {
    let path = game_dir.join("options.txt");
    let text = fs::read_to_string(&path).unwrap_or_default();
    let mut lines: Vec<String> = text.lines().map(String::from).collect();
    let mut found = false;
    for l in lines.iter_mut() {
        let rest = l.strip_prefix("resourcePacks:").map(|r| r.to_string());
        if let Some(rest) = rest {
            found = true;
            let Ok(mut packs) = serde_json::from_str::<Vec<String>>(&rest) else { continue };
            packs.retain(|p| p != id);
            if enabled {
                packs.push(id.to_string());
            }
            *l = format!("resourcePacks:{}", serde_json::to_string(&packs).map_err(|e| e.to_string())?);
        }
    }
    if !found && enabled {
        let list: Vec<String> = if id.starts_with("file/") { vec!["vanilla".into(), id.to_string()] } else { vec![id.to_string()] };
        lines.push(format!("resourcePacks:{}", serde_json::to_string(&list).map_err(|e| e.to_string())?));
    }
    if lines.is_empty() {
        return Ok(());
    }
    let new_text = lines.join("\n") + "\n";
    if new_text != text {
        fs::write(&path, new_text).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Builds the skin pack from the skin chosen on the Skins page. Returns false when no skin is chosen.
fn apply_offline_skin(root: &Path, game_dir: &Path, version: &str, jar: &Path) -> Result<bool, String> {
    let skins = root.join("skins");
    let active = fs::read_to_string(skins.join("active.txt")).unwrap_or_default();
    let name = active.trim();
    let pack_dir = game_dir.join("resourcepacks").join(SKIN_PACK);
    let id = if legacy_pack_ids(version) { SKIN_PACK.to_string() } else { format!("file/{SKIN_PACK}") };
    let src = skins.join(format!("{name}.png"));
    if name.is_empty() || name.contains(|c: char| matches!(c, '/' | '\\' | ':')) || !src.is_file() {
        let _ = fs::remove_dir_all(&pack_dir);
        set_pack_enabled(game_dir, &id, false)?;
        return Ok(false);
    }

    let img = prepare_skin(&src)?;
    let _ = fs::remove_dir_all(&pack_dir);
    let tex = pack_dir.join("assets").join("minecraft").join("textures").join("entity");
    fs::create_dir_all(&tex).map_err(|e| e.to_string())?;
    let base = tex.join("steve.png");
    img.save_with_format(&base, image::ImageFormat::Png).map_err(|e| e.to_string())?;
    // Every place the game looks for a default skin, so it works on old and new versions alike.
    fs::copy(&base, tex.join("alex.png")).map_err(|e| e.to_string())?;
    let names = ["alex", "ari", "efe", "kai", "makena", "noor", "steve", "sunny", "zuri"];
    for model in ["slim", "wide"] {
        let d = tex.join("player").join(model);
        fs::create_dir_all(&d).map_err(|e| e.to_string())?;
        for n in names {
            fs::copy(&base, d.join(format!("{n}.png"))).map_err(|e| e.to_string())?;
        }
    }
    let pack_format = jar_pack_format(jar, version);
    let mcmeta = serde_json::json!({
        "pack": { "description": "MinimalMc offline skin", "pack_format": pack_format, "min_format": pack_format, "max_format": pack_format }
    });
    fs::write(pack_dir.join("pack.mcmeta"), serde_json::to_string_pretty(&mcmeta).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    set_pack_enabled(game_dir, &id, true)?;
    Ok(true)
}

// ───────────────────────── The launch itself ─────────────────────────

#[tauri::command]
pub async fn launch_instance(app: tauri::AppHandle, id: u64, username: String) -> Result<(), String> {
    let name = username.trim().to_string();
    if name.len() < 3 || name.len() > 16 || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err("Player names must be 3 to 16 letters, numbers or underscores.".into());
    }
    let inst = instances::load(&app)?
        .into_iter()
        .find(|i| i.id == id)
        .ok_or("Instance not found")?;
    let loader = inst.loader.to_lowercase();
    if !matches!(loader.as_str(), "vanilla" | "fabric" | "quilt" | "neoforge" | "forge") {
        return Err(format!("{} instances can't be launched. Vanilla, Fabric, Quilt, Forge and NeoForge are supported.", inst.loader));
    }
    {
        let mut t = table().lock().map_err(|_| "Internal error".to_string())?;
        let e = t.entry(id).or_default();
        if e.busy || e.running {
            return Err("This instance is already starting or running.".into());
        }
        *e = LaunchStatus { stage: "Preparing".into(), busy: true, ..Default::default() };
    }
    let a = app.clone();
    tauri::async_runtime::spawn(async move {
        STRICT.store(false, Ordering::Relaxed);
        if let Err(e) = run(a, inst, name, false).await {
            set(id, |s| {
                s.busy = false;
                s.running = false;
                s.error = Some(e);
            });
        }
    });
    Ok(())
}

/// Re-checks every game file of an instance against its checksum and downloads whatever is missing
/// or damaged. Nothing is launched.
#[tauri::command]
pub async fn repair_instance(app: tauri::AppHandle, id: u64) -> Result<(), String> {
    let inst = instances::load(&app)?
        .into_iter()
        .find(|i| i.id == id)
        .ok_or("Instance not found")?;
    let loader = inst.loader.to_lowercase();
    if !matches!(loader.as_str(), "vanilla" | "fabric" | "quilt" | "neoforge" | "forge") {
        return Err(format!("{} instances can't be repaired yet.", inst.loader));
    }
    {
        let mut t = table().lock().map_err(|_| "Internal error".to_string())?;
        let e = t.entry(id).or_default();
        if e.busy || e.running {
            return Err("This instance is busy right now.".into());
        }
        *e = LaunchStatus { stage: "Repairing".into(), busy: true, ..Default::default() };
    }
    let a = app.clone();
    tauri::async_runtime::spawn(async move {
        STRICT.store(true, Ordering::Relaxed);
        let result = run(a, inst, "Repair".to_string(), true).await;
        STRICT.store(false, Ordering::Relaxed);
        if let Err(e) = result {
            set(id, |s| {
                s.busy = false;
                s.running = false;
                s.error = Some(e);
            });
        }
    });
    Ok(())
}

async fn run(app: tauri::AppHandle, inst: instances::Instance, player: String, repair: bool) -> Result<(), String> {
    let id = inst.id;
    let root = data_dir(&app)?;
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?;

    // 1. Minecraft's own version info.
    set(id, |s| s.stage = "Reading version info".into());
    let vanilla = load_vanilla(&client, &root, &inst.version).await?;

    // 2. Java (what Minecraft itself needs; mod loaders run on the same one).
    let required = vanilla.java_version.as_ref().map(|j| j.major_version).unwrap_or(8);
    let component = vanilla
        .java_version
        .as_ref()
        .and_then(|j| j.component.clone())
        .unwrap_or_else(|| "jre-legacy".to_string());
    let java = resolve_java(&client, &root, id, &component, required).await?;

    // 3. Game jar, asset index and logging config.
    let asset_ref = vanilla.asset_index.clone().ok_or("This version has no asset index")?;
    let client_dl = vanilla
        .downloads
        .as_ref()
        .and_then(|d| d.client.clone())
        .ok_or("This version has no client download")?;
    let jar_path = root.join("versions").join(&inst.version).join(format!("{}.jar", inst.version));
    let index_path = root.join("assets").join("indexes").join(format!("{}.json", asset_ref.id));
    let mut core = vec![
        Job { url: client_dl.url.clone(), path: jar_path.clone(), sha1: client_dl.sha1.clone(), size: client_dl.size },
        Job { url: asset_ref.url.clone(), path: index_path.clone(), sha1: asset_ref.sha1.clone(), size: asset_ref.size },
    ];
    let mut log_arg: Option<String> = None;
    if let Some(lc) = vanilla.logging.as_ref().and_then(|l| l.client.as_ref()) {
        let p = root.join("assets").join("log_configs").join(&lc.file.id);
        core.push(Job { url: lc.file.url.clone(), path: p.clone(), sha1: lc.file.sha1.clone(), size: lc.file.size });
        log_arg = Some(lc.argument.replace("${path}", &p.to_string_lossy()));
    }
    download_all(&client, id, "Downloading Minecraft", core).await?;

    // 4. The mod loader, layered on top of Minecraft.
    let loader = inst.loader.to_lowercase();
    let meta = match loader.as_str() {
        "fabric" | "quilt" => {
            set(id, |s| s.stage = "Reading loader info".into());
            let profile = load_profile(&client, &root, &loader, &inst.version, &inst.loader_version).await?;
            merge(vanilla.clone(), profile)
        }
        "neoforge" => {
            let (profile, ver) = ensure_neoforge(&client, &root, &java, id, &inst.version, &inst.loader_version).await?;
            if inst.loader_version.is_empty() {
                pin_loader_version(&app, id, &ver);
            }
            merge(vanilla.clone(), profile)
        }
        "forge" => {
            let (profile, ver) = ensure_forge(&client, &root, &java, id, &inst.version, &inst.loader_version).await?;
            if inst.loader_version.is_empty() {
                pin_loader_version(&app, id, &ver);
            }
            merge(vanilla.clone(), profile)
        }
        _ => vanilla.clone(),
    };

    // 5. Libraries.
    let plan = plan_libraries(&meta.libraries, &root);
    download_all(&client, id, "Downloading libraries", plan.jobs.clone()).await?;

    // 6. Assets (sounds, languages, textures that aren't in the jar).
    let idx_text = fs::read_to_string(&index_path).map_err(|e| e.to_string())?;
    let idx: AssetIdx = serde_json::from_str(&idx_text).map_err(|e| format!("Unreadable asset index: {e}"))?;
    if idx.is_virtual || idx.map_to_resources {
        return Err("Minecraft versions older than 1.7.3 can't be launched yet.".into());
    }
    let objects_dir = root.join("assets").join("objects");
    let asset_jobs: Vec<Job> = idx
        .objects
        .values()
        .filter(|o| o.hash.len() >= 2)
        .map(|o| Job {
            url: format!("{ASSET_BASE}/{}/{}", &o.hash[..2], o.hash),
            path: objects_dir.join(&o.hash[..2]).join(&o.hash),
            sha1: Some(o.hash.clone()),
            size: o.size,
        })
        .collect();
    download_all(&client, id, "Downloading assets", asset_jobs).await?;

    // 7. Natives and command line.
    set(id, |s| {
        s.stage = "Starting".into();
        s.done = 0;
        s.total = 0;
    });
    let inst_dir = instances::dir_of(&app, id)?;
    let game_dir = inst_dir.join(".minecraft");
    fs::create_dir_all(game_dir.join("logs")).map_err(|e| e.to_string())?;
    let natives_dir = inst_dir.join("natives");
    extract_natives(&plan.natives, &natives_dir)?;
    if repair {
        // Everything is downloaded and verified. Don't start the game.
        set(id, |s| {
            s.busy = false;
            s.running = false;
            s.done = 0;
            s.total = 0;
            s.stage = "Repaired".into();
        });
        return Ok(());
    }

    if inst.auto_backup {
        set(id, |s| s.stage = "Backing up worlds".into());
        let a2 = app.clone();
        let _ = tauri::async_runtime::spawn_blocking(move || crate::backups::auto_backup(&a2, id, 5)).await;
    }

    if let Err(e) = apply_offline_skin(&root, &game_dir, &inst.version, &jar_path) {
        set(id, |s| s.notice = Some(format!("Your skin couldn't be applied: {e}.")));
    }

    let sep = if cfg!(windows) { ";" } else { ":" };
    let mut cp: Vec<String> = plan.classpath.iter().map(|p| p.to_string_lossy().to_string()).collect();
    cp.push(jar_path.to_string_lossy().to_string());
    let classpath = cp.join(sep);

    let mut vars: HashMap<&str, String> = HashMap::new();
    vars.insert("auth_player_name", player.clone());
    // NeoForge ignores the vanilla jar by its file name, which is the Minecraft version.
    vars.insert("version_name", if loader == "neoforge" || loader == "forge" { inst.version.clone() } else { meta.id.clone() });
    vars.insert("game_directory", game_dir.to_string_lossy().to_string());
    vars.insert("assets_root", root.join("assets").to_string_lossy().to_string());
    vars.insert("assets_index_name", asset_ref.id.clone());
    vars.insert("auth_uuid", offline_uuid(&player));
    vars.insert("auth_access_token", "0".into());
    vars.insert("clientid", String::new());
    vars.insert("auth_xuid", "0".into());
    vars.insert("user_type", "legacy".into());
    vars.insert("version_type", meta.kind.clone().unwrap_or_else(|| "release".into()));
    vars.insert("natives_directory", natives_dir.to_string_lossy().to_string());
    vars.insert("launcher_name", "MinimalMc".into());
    vars.insert("launcher_version", "1.0.0".into());
    vars.insert("classpath", classpath.clone());
    vars.insert("library_directory", root.join("libraries").to_string_lossy().to_string());
    vars.insert("classpath_separator", sep.into());
    vars.insert("user_properties", "{}".into());

    let (raw_jvm, raw_game): (Vec<String>, Vec<String>) = match &meta.arguments {
        Some(a) => (expand(&a.jvm), expand(&a.game)),
        None => (
            vec![
                "-Djava.library.path=${natives_directory}".into(),
                "-Dminecraft.launcher.brand=${launcher_name}".into(),
                "-Dminecraft.launcher.version=${launcher_version}".into(),
                "-cp".into(),
                "${classpath}".into(),
            ],
            meta.minecraft_arguments.clone().unwrap_or_default().split_whitespace().map(String::from).collect(),
        ),
    };
    let mut jvm: Vec<String> = vec![format!("-Xmx{}G", inst.ram.max(1))];
    jvm.extend(raw_jvm.iter().map(|a| subst(a, &vars)));
    if !jvm.iter().any(|a| a == "-cp" || a == "-classpath") {
        jvm.push("-cp".into());
        jvm.push(classpath.clone());
    }
    if let Some(l) = log_arg {
        jvm.push(l);
    }
    // The player's own Java arguments go last so they win over the defaults.
    jvm.extend(inst.jvm_args.split_whitespace().map(String::from));
    let mut game: Vec<String> = raw_game.iter().map(|a| subst(a, &vars)).collect();
    if inst.width > 0 && inst.height > 0 {
        game.push("--width".into());
        game.push(inst.width.to_string());
        game.push("--height".into());
        game.push(inst.height.to_string());
    }
    // Join a server straight away. 1.20 and newer use "quick play"; older versions use --server.
    let server = inst.server.trim();
    if !server.is_empty() {
        let mut parts = inst.version.split('.');
        let major = parts.next().and_then(|a| a.parse::<u32>().ok());
        let minor = parts.next().and_then(|b| b.parse::<u32>().ok());
        let modern = match (major, minor) {
            (Some(1), Some(m)) => m >= 20,
            _ => true,
        };
        if modern {
            game.push("--quickPlayMultiplayer".into());
            game.push(server.to_string());
        } else {
            let (host, port) = match server.rsplit_once(':') {
                Some((h, p)) if p.parse::<u16>().is_ok() => (h, p),
                _ => (server, "25565"),
            };
            game.push("--server".into());
            game.push(host.to_string());
            game.push("--port".into());
            game.push(port.to_string());
        }
    }
    let main_class = meta.main_class.clone().ok_or("This version has no main class")?;

    // 8. Go. Everything the game prints is saved so crashes can be investigated.
    let log_path = game_dir.join("logs").join("minimalmc-launch.log");
    let log = fs::File::create(&log_path).map_err(|e| e.to_string())?;
    let log2 = log.try_clone().map_err(|e| e.to_string())?;
    let mut cmd = Command::new(&java);
    cmd.args(&jvm)
        .arg(&main_class)
        .args(&game)
        .current_dir(&game_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(log2));
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000);
    let started = std::time::Instant::now();
    let mut child = cmd.spawn().map_err(|e| format!("Couldn't start Java: {e}"))?;
    set(id, |s| {
        s.stage = "Running".into();
        s.busy = false;
        s.running = true;
    });
    mark_played(&app, id);

    let status = child.wait().await;
    add_playtime(&app, id, started.elapsed().as_secs());
    match status {
        Ok(s) if s.success() => set(id, |st| {
            st.running = false;
            st.stage = "Idle".into();
        }),
        Ok(s) => set(id, |st| {
            st.running = false;
            st.error = Some(format!(
                "Minecraft closed unexpectedly (exit code {}). Details are in {}",
                s.code().unwrap_or(-1),
                log_path.display()
            ));
        }),
        Err(e) => set(id, |st| {
            st.running = false;
            st.error = Some(format!("Lost track of the game process: {e}"));
        }),
    }
    Ok(())
}
