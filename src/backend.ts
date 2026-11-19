export type Version = { id: string; kind: string; released: string }
export type JavaInstall = { path: string; version: string }
export type JavaEntry = { path: string; version: string; major: number; managed: boolean }
export type SkinFile = { name: string; png_base64: string; active: boolean }
export type InstanceFile = { name: string; size: number; is_dir: boolean }
export type LaunchStatus = { stage: string; done: number; total: number; busy: boolean; running: boolean; error: string | null; notice: string | null }
export type BackendInstance = { id: number; name: string; version: string; loader: string; last: string; group: string; ram: number; mods: number; loader_version?: string; played?: number; jvm_args?: string; width?: number; height?: number; server?: string; auto_backup?: boolean }
export type CompatFix = { kind: string; label: string; target: string }
export type CompatFinding = { level: string; file: string; mod_name: string; message: string; fix: CompatFix | null }
export type CompatReport = { checked: number; findings: CompatFinding[] }
export type Backup = { name: string; world: string; time: number; size: number }
export type ModUpdate = { file: string; new_file: string; new_version: string }
export type UpdateReport = { checked: number; updates: ModUpdate[]; can_undo: number }
export type UpdateResult = { updated: string[]; failed: string[] }
export type Importable = { name: string; path: string; mc: string; loader: string; source: string }
export type ModNode = { file: string; id: string; name: string; requires: string[]; provides: string[] }
export type Shot = { name: string; size: number; time: number }
export type DedupeResult = { merged: number; saved: number }
export type ProjectGallery = { url: string; featured: boolean; title: string | null; description: string | null }
export type ProjectInfo = { id: string; slug: string; title: string; description: string; body: string; categories: string[]; loaders: string[]; game_versions: string[]; downloads: number; followers: number; icon_url: string | null; issues_url: string | null; source_url: string | null; wiki_url: string | null; discord_url: string | null; license: { id: string; name: string; url: string | null } | null; gallery: ProjectGallery[]; published: string; updated: string; client_side: string; server_side: string; project_type: string }
export type ProjectMember = { user: { username: string; avatar_url: string | null }; role: string }
export type ProjectVersion = { id: string; name: string; version_number: string; version_type: string; game_versions: string[]; loaders: string[]; date_published: string; downloads: number; changelog: string | null }
export type ProjectDetail = { project: ProjectInfo; members: ProjectMember[]; versions: ProjectVersion[] }
export type Snapshot = { name: string; label: string; time: number; minecraft: string; loader: string; mods: number }
export type MrpackResult = { path: string; size: number; linked: number; bundled: number }
export type CfKeyStatus = { saved: boolean; hint: string }
export type Diagnosis = { level: string; title: string; detail: string; fix: CompatFix | null }
export type DoctorReport = { has_data: boolean; source: string; diagnoses: Diagnosis[]; tail: string }

export type MrHit = { project_id: string; slug: string; title: string; description: string; author: string; downloads: number; icon_url: string | null; categories: string[]; project_type: string; latest_version: string | null }
export type MrSearch = { hits: MrHit[]; total: number }
export type MrInstall = { installed: string[]; already_present: string[]; warnings: string[] }

export const isDesktop = () => typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!isDesktop()) throw new Error('Desktop backend unavailable in browser preview')
  const internals = (window as any).__TAURI_INTERNALS__
  return internals.invoke(cmd, args) as Promise<T>
}

export const backend = {
  listInstances: () => call<BackendInstance[]>('list_instances'),
  createInstance: (name: string, version: string, loader: string, group: string, ram: number) => call<BackendInstance>('create_instance', { name, version, loader, group, ram }),
  renameInstance: (id: number, name: string) => call<void>('rename_instance', { id, name }),
  duplicateInstance: (id: number) => call<BackendInstance>('duplicate_instance', { id }),
  deleteInstance: (id: number) => call<void>('delete_instance', { id }),
  fetchVersions: (includeSnapshots = false) => call<Version[]>('fetch_versions', { includeSnapshots }),
  detectJava: () => call<JavaInstall[]>('detect_java'),
  listJava: () => call<JavaEntry[]>('list_java'),
  listSkins: () => call<SkinFile[]>('list_skins'),
  saveSkin: async (file: File) => call<string>('save_skin', { name: file.name.replace(/\.[^.]+$/, ''), data: Array.from(new Uint8Array(await file.arrayBuffer())) }),
  deleteSkin: (name: string) => call<void>('delete_skin', { name }),
  listInstanceFiles: (id: number, kind: string) => call<InstanceFile[]>('list_instance_files', { id, kind }),
  removeInstanceFile: (id: number, kind: string, name: string) => call<void>('remove_instance_file', { id, kind, name }),
  openInstanceFolder: (id: number) => call<void>('open_instance_folder', { id }),
  updateInstance: (id: number, p: { group?: string; ram?: number; jvmArgs?: string; width?: number; height?: number; server?: string }) => call<void>('update_instance', { id, ...p }),
  setModEnabled: (id: number, name: string, enabled: boolean) => call<void>('set_mod_enabled', { id, name, enabled }),
  readLaunchLog: (id: number) => call<string>('read_launch_log', { id }),
  checkCompat: (id: number) => call<CompatReport>('check_compat', { id }),
  diagnose: (id: number) => call<DoctorReport>('diagnose', { id }),
  backupWorld: (id: number, world: string) => call<Backup>('backup_world', { id, world }),
  listBackups: (id: number) => call<Backup[]>('list_backups', { id }),
  restoreBackup: (id: number, backup: string) => call<void>('restore_backup', { id, backup }),
  deleteBackup: (id: number, backup: string) => call<void>('delete_backup', { id, backup }),
  checkModUpdates: (id: number) => call<UpdateReport>('check_mod_updates', { id }),
  updateMods: (id: number, files: string[]) => call<UpdateResult>('update_mods', { id, files }),
  rollbackModUpdates: (id: number) => call<number>('rollback_mod_updates', { id }),
  modGraph: (id: number) => call<ModNode[]>('mod_graph', { id }),
  listScreenshots: (id: number) => call<Shot[]>('list_screenshots', { id }),
  screenshotThumb: (id: number, name: string) => call<string>('screenshot_thumb', { id, name }),
  screenshotFull: (id: number, name: string) => call<string>('screenshot_full', { id, name }),
  openExportsFolder: () => call<void>('open_exports_folder'),
  dedupeStorage: () => call<DedupeResult>('dedupe_storage'),
  repairInstance: (id: number) => call<void>('repair_instance', { id }),
  createSnapshot: (id: number, label: string) => call<Snapshot>('create_snapshot', { id, label }),
  listSnapshots: (id: number) => call<Snapshot[]>('list_snapshots', { id }),
  restoreSnapshot: (id: number, name: string) => call<void>('restore_snapshot', { id, name }),
  deleteSnapshot: (id: number, name: string) => call<void>('delete_snapshot', { id, name }),
  backupAllWorlds: (id: number) => call<number>('backup_all_worlds', { id }),
  setAutoBackup: (id: number, enabled: boolean) => call<void>('set_auto_backup', { id, enabled }),
  exportMrpack: (id: number) => call<MrpackResult>('export_mrpack', { id }),
  listImportable: () => call<Importable[]>('list_importable'),
  importInstance: (path: string) => call<BackendInstance>('import_instance', { path }),
  launchInstance: (id: number, username: string) => call<void>('launch_instance', { id, username }),
  launchStatus: (id: number) => call<LaunchStatus>('launch_status', { id }),
  installCurseforgePack: (projectId: string, versionId?: string) => call<BackendInstance>('curseforge_install_modpack', { projectId, versionId }),
  importFile: (path: string) => call<BackendInstance>('import_file', { path }),
  installModpack: (projectId: string, versionId?: string) => call<BackendInstance>('install_modpack', { projectId, versionId }),
  searchModrinth: (query: string, projectType: string, loader: string, gameVersion: string, sort: string, offset: number) => call<MrSearch>('modrinth_search', { query, projectType, loader, gameVersion, sort, offset }),
  installModrinth: (instanceId: number, projectId: string, projectType: string, versionId?: string) => call<MrInstall>('modrinth_install', { instanceId, projectId, projectType, versionId }),
  modrinthProject: (projectId: string) => call<ProjectDetail>('modrinth_project', { projectId }),
  openUrl: (url: string) => call<void>('open_url', { url }),
  curseforgeStatus: () => call<CfKeyStatus>('curseforge_status'),
  curseforgeSetKey: (key: string) => call<void>('curseforge_set_key', { key }),
  curseforgeTest: () => call<string>('curseforge_test'),
  searchCurseforge: (query: string, projectType: string, loader: string, gameVersion: string, sort: string, offset: number) => call<MrSearch>('curseforge_search', { query, projectType, loader, gameVersion, sort, offset }),
  curseforgeProject: (projectId: string) => call<ProjectDetail>('curseforge_project', { projectId }),
  installCurseforge: (instanceId: number, projectId: string, projectType: string, versionId?: string) => call<MrInstall>('curseforge_install', { instanceId, projectId, projectType, versionId }),
  setActiveSkin: (name: string) => call<void>('set_active_skin', { name }),
}

let versionCache: string[] | null = null
/** Official Minecraft release ids from Mojang, newest first. Cached after the first call. */
export async function releaseVersions(): Promise<string[]> {
  if (versionCache) return versionCache
  const l = await backend.fetchVersions(false)
  versionCache = l.map((v) => v.id)
  return versionCache
}
