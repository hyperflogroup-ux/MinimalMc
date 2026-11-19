export type Inst = { id: number; name: string; group: string; mc: string; loader: string; mods: number; ram: number; last: string; jvmArgs?: string; width?: number; height?: number; played?: number; server?: string; autoBackup?: boolean }

export const GROUPS = ['Survival', 'Modded', 'Testing', 'Servers', 'Other']
export const LOADERS = ['Vanilla', 'Fabric', 'Forge', 'NeoForge', 'Quilt']
