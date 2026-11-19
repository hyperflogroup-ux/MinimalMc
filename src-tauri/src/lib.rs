mod backups;
mod compat;
mod curseforge;
mod doctor;
mod exporter;
mod instances;
mod importer;
mod java;
mod launch;
mod modpack;
mod modrinth;
mod runtime;
mod snapshots;
mod screenshots;
mod storage;
mod updates;
mod skins;
mod versions;

use std::path::PathBuf;
use tauri::Manager;

pub fn data_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

pub fn run() {
    // Workaround for a black-screen bug on older Intel GPU drivers (e.g. HD Graphics 5xx
    // with pre-2023 drivers) where WebView2's GPU-accelerated compositor fails to paint.
    // Must be set before the WebView2 environment is created.
    #[cfg(windows)]
    if std::env::var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS").is_err() {
        std::env::set_var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS", "--disable-gpu");
    }

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            instances::list_instances,
            instances::create_instance,
            instances::rename_instance,
            instances::duplicate_instance,
            instances::delete_instance,
            instances::list_instance_files,
            instances::remove_instance_file,
            instances::open_instance_folder,
            instances::update_instance,
            instances::set_mod_enabled,
            instances::read_launch_log,
            compat::check_compat,
            compat::mod_graph,
            doctor::diagnose,
            backups::backup_world,
            backups::list_backups,
            backups::restore_backup,
            backups::delete_backup,
            backups::backup_all_worlds,
            instances::set_auto_backup,
            snapshots::create_snapshot,
            snapshots::list_snapshots,
            snapshots::restore_snapshot,
            snapshots::delete_snapshot,
            updates::check_mod_updates,
            updates::update_mods,
            updates::rollback_mod_updates,
            importer::list_importable,
            importer::import_instance,
            importer::import_file,
            exporter::open_exports_folder,
            exporter::export_mrpack,
            exporter::open_url,
            screenshots::list_screenshots,
            screenshots::screenshot_thumb,
            screenshots::screenshot_full,
            storage::dedupe_storage,
            versions::fetch_versions,
            java::detect_java,
            launch::launch_instance,
            launch::launch_status,
            launch::repair_instance,
            runtime::list_java,
            modpack::install_modpack,
            modrinth::modrinth_search,
            modrinth::modrinth_project,
            curseforge::curseforge_status,
            curseforge::curseforge_set_key,
            curseforge::curseforge_test,
            curseforge::curseforge_search,
            curseforge::curseforge_project,
            curseforge::curseforge_install,
            curseforge::curseforge_install_modpack,
            modrinth::modrinth_install,
            skins::list_skins,
            skins::save_skin,
            skins::delete_skin,
            skins::set_active_skin,
        ])
        .run(tauri::generate_context!())
        .expect("error while running MinimalMc");
}
