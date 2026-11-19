// The images and videos chosen for each instance's Home banner. They are kept in the app's own
// database (IndexedDB), so they are still there after a restart, and each instance has its own.

const DB_NAME = 'minimalmc'
const STORE = 'media'

export type MediaItem = { key: number; instanceId: number; name: string; type: string; blob: Blob }

function openDb(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, 1)
    req.onupgradeneeded = () => {
      const db = req.result
      if (!db.objectStoreNames.contains(STORE)) {
        const s = db.createObjectStore(STORE, { keyPath: 'key', autoIncrement: true })
        s.createIndex('instanceId', 'instanceId')
      }
    }
    req.onsuccess = () => resolve(req.result)
    req.onerror = () => reject(req.error)
  })
}

function wrap<T>(r: IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    r.onsuccess = () => resolve(r.result)
    r.onerror = () => reject(r.error)
  })
}

export async function listMedia(instanceId: number): Promise<MediaItem[]> {
  const db = await openDb()
  const tx = db.transaction(STORE, 'readonly')
  const items = await wrap(tx.objectStore(STORE).index('instanceId').getAll(instanceId))
  return items as MediaItem[]
}

export async function addMedia(instanceId: number, blob: Blob, name: string, type: string): Promise<MediaItem> {
  const db = await openDb()
  const tx = db.transaction(STORE, 'readwrite')
  const record = { instanceId, name, type, blob }
  const key = await wrap(tx.objectStore(STORE).add(record))
  return { ...record, key: key as number }
}

export async function removeMedia(key: number): Promise<void> {
  const db = await openDb()
  const tx = db.transaction(STORE, 'readwrite')
  await wrap(tx.objectStore(STORE).delete(key))
}

/** Used when an instance is deleted, so its media doesn't pile up. */
export async function removeAllMedia(instanceId: number): Promise<void> {
  const items = await listMedia(instanceId)
  for (const m of items) await removeMedia(m.key)
}
