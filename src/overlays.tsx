import { useEffect, useRef, useState } from 'react'
import { Blk, Btn, DialogCard, Icon, Input, Modal, Pill, Select, useApp } from './ui'
import { backend, isDesktop, type Importable } from './backend'
import { GROUPS, LOADERS, type Inst } from './data'

const errText = (e: unknown) => (e instanceof Error ? e.message : String(e))

type Entry = { g: string; n: string; s: string; seed?: number; icon?: string; run: () => void }
const PAGES: [string, string, string][] = [
  ['home', 'Home', 'home'], ['instances', 'Instances', 'box'], ['mods', 'Mods', 'puzzle'], ['modpacks', 'Modpacks', 'layers'],
  ['shaders', 'Shaders', 'sun'], ['packs', 'Resource Packs', 'image'], ['skins', 'Skins', 'user'], ['settings', 'Settings', 'settings'],
]

export function Palette({ onClose }: { onClose: () => void }) {
  const app = useApp()
  const [q, setQ] = useState('')
  const [i, setI] = useState(0)
  const ref = useRef<HTMLInputElement>(null)
  useEffect(() => ref.current?.focus(), [])
  const all: Entry[] = [
    ...(app.insts as Inst[]).map((x) => ({ g: 'Instances', n: x.name, s: `${x.mc} · ${x.loader}`, seed: x.id, run: () => app.openInstance(x.id) })),
    ...PAGES.map(([id, n, icon]) => ({ g: 'Go to', n, s: '', icon, run: () => app.go(id) })),
    { g: 'Actions', n: 'Create instance', s: 'New instance', icon: 'plus', run: () => app.setDialog({ kind: 'create' }) },
  ]
  const res = all.filter((x) => (x.n + x.s + x.g).toLowerCase().includes(q.toLowerCase())).slice(0, 9)
  const groups = [...new Set(res.map((r) => r.g))]
  const pick = (r: Entry) => { onClose(); r.run() }
  return (
    <div className="fixed inset-0 z-[90] bg-black/45 fade grid place-items-start justify-center pt-[14vh]" onMouseDown={onClose}>
      <div onMouseDown={(e) => e.stopPropagation()} style={{ boxShadow: 'var(--shadow)' }} className="pop w-[560px] max-w-[92vw] rounded-[24px] bg-s1 overflow-hidden">
        <div className="flex items-center gap-3 px-5 h-14">
          <Icon n="search" className="text-mu" />
          <input ref={ref} value={q} onChange={(e) => { setQ(e.target.value); setI(0) }} placeholder="Search instances, pages, actions" className="flex-1 bg-transparent outline-none text-[0.9375rem] placeholder:text-mu2"
            onKeyDown={(e) => { if (e.key === 'Escape') onClose(); if (!res.length) return; if (e.key === 'ArrowDown') { e.preventDefault(); setI((i + 1) % res.length) } if (e.key === 'ArrowUp') { e.preventDefault(); setI((i - 1 + res.length) % res.length) } if (e.key === 'Enter' && res[i]) pick(res[i]) }} />
          <Pill>Esc</Pill>
        </div>
        <div className="h-px bg-line" />
        <div className="max-h-[50vh] scroll p-2">
          {groups.map((g) => (
            <div key={g} className="mb-1">
              <div className="px-3 py-1.5 text-[0.6875rem] uppercase tracking-[.1em] text-mu2 font-medium">{g}</div>
              {res.map((r, k) => r.g === g && (
                <button key={k} onMouseMove={() => setI(k)} onClick={() => pick(r)} className={`w-full h-11 px-3 rounded-xl flex items-center gap-3 text-left ${i === k ? 'bg-s3' : ''}`}>
                  {r.icon ? <div className="size-7 rounded-lg bg-fg/[.07] grid place-items-center"><Icon n={r.icon} size={14} /></div> : <Blk seed={r.seed!} size={28} />}
                  <span className="text-[0.8438rem] font-medium flex-1">{r.n}</span><span className="text-xs text-mu">{r.s}</span>
                </button>
              ))}
            </div>
          ))}
          {!res.length && <div className="py-10 text-center text-sm text-mu">Nothing found for “{q}”</div>}
        </div>
      </div>
    </div>
  )
}

const STEPS = ['Name', 'Version', 'Loader', 'Memory', 'Create']
const TITLES = ['Name your instance', 'Minecraft version', 'Choose a loader', 'Memory', 'Ready to create']

export function CreateDialog({ onClose }: { onClose: () => void }) {
  const app = useApp()
  const [s, setS] = useState(0)
  const [name, setName] = useState('')
  const [group, setGroup] = useState('Other')
  const [mc, setMc] = useState('')
  const [loader, setLoader] = useState('Fabric')
  const [ram, setRam] = useState(4)
  const [busy, setBusy] = useState(false)
  const [snaps, setSnaps] = useState(false)
  const [versions, setVersions] = useState<string[]>([])
  const [vState, setVState] = useState<'loading' | 'ok' | 'err'>('loading')
  const [retry, setRetry] = useState(0)
  const [err, setErr] = useState('')

  useEffect(() => {
    let live = true
    setVState('loading')
    backend.fetchVersions(snaps)
      .then((l) => {
        if (!live) return
        const ids = l.map((v) => v.id)
        setVersions(ids)
        setMc((cur) => (ids.includes(cur) ? cur : ids[0] ?? ''))
        setVState('ok')
      })
      .catch(() => { if (live) setVState('err') })
    return () => { live = false }
  }, [snaps, retry])

  const create = async () => {
    setBusy(true); setErr('')
    try {
      await app.addInstance({ name: name.trim(), group, mc, loader, ram })
      onClose()
    } catch (e) {
      setErr(errText(e)); setBusy(false)
    }
  }
  const blocked = (s === 0 && !name.trim()) || (s === 1 && !mc)
  return (
    <Modal onClose={onClose} w={500}>
      <div className="p-7 pb-0">
        <div className="flex gap-1.5 mb-6">{STEPS.map((x, k) => <div key={x} className={`h-1 flex-1 rounded-full transition ${k <= s ? 'bg-ac' : 'bg-fg/10'}`} />)}</div>
        <div className="text-[0.6875rem] uppercase tracking-[.12em] text-mu2 font-medium">Step {s + 1} of {STEPS.length}</div>
        <h2 className="text-[1.375rem] font-semibold tracking-tight mt-1">{TITLES[s]}</h2>
      </div>
      <div className="p-7 min-h-[240px]" key={s}>
        <div className="fade">
          {s === 0 && (
            <div className="space-y-5">
              <Input autoFocus value={name} onChange={(e: any) => setName(e.target.value)} placeholder="Instance name" />
              <div>
                <div className="text-xs text-mu mb-2">Group</div>
                <Select value={group} onChange={setGroup} options={GROUPS} className="w-full" />
              </div>
            </div>
          )}
          {s === 1 && (
            <div className="space-y-4">
              {vState === 'ok' && <Select value={mc} onChange={setMc} options={versions} className="w-full" />}
              {vState === 'loading' && <div className="text-sm text-mu">Loading versions…</div>}
              {vState === 'err' && (
                <div className="space-y-3">
                  <Pill tone="warn">Couldn't load Minecraft versions</Pill>
                  <div><Btn v="secondary" size="sm" onClick={() => setRetry((n) => n + 1)}>Try again</Btn></div>
                </div>
              )}
              <div className="flex gap-1.5"><Pill active={snaps} onClick={() => setSnaps(!snaps)}>Snapshots {snaps ? 'on' : 'off'}</Pill></div>
            </div>
          )}
          {s === 2 && <div className="grid grid-cols-3 gap-2">{LOADERS.map((l) => <button key={l} onClick={() => setLoader(l)} className={`h-16 rounded-2xl text-[0.8438rem] font-medium transition ${loader === l ? 'bg-ac/14 text-act ring-2 ring-ac/60' : 'bg-s2 hover:bg-s3'}`}>{l}</button>)}</div>}
          {s === 3 && (
            <div>
              <div className="text-[2.5rem] font-semibold tracking-tight leading-none">{ram} <span className="text-base text-mu font-normal">GB</span></div>
              <input type="range" min={1} max={16} value={ram} onChange={(e) => setRam(+e.target.value)} className="w-full mt-6" />
              <div className="flex justify-between text-xs text-mu mt-1"><span>1 GB</span><span>16 GB</span></div>
            </div>
          )}
          {s === 4 && (
            <div className="rounded-2xl bg-s2 p-5">
              <div className="text-[0.9375rem] font-medium">{name.trim()}</div>
              <div className="flex gap-1.5 mt-2 flex-wrap"><Pill>{mc}</Pill><Pill tone="ac">{loader}</Pill><Pill>{ram} GB</Pill><Pill>{group}</Pill></div>
            </div>
          )}
        </div>
      </div>
      {err && <div className="px-7 pb-3 text-xs text-red-500">{err}</div>}
      <div className="px-7 pb-7 flex justify-between">
        <Btn v="ghost" onClick={s ? () => setS(s - 1) : onClose}>{s ? 'Back' : 'Cancel'}</Btn>
        {s < STEPS.length - 1
          ? <Btn v="primary" disabled={blocked} onClick={() => setS(s + 1)}>Continue</Btn>
          : <Btn v="primary" icon="check" loading={busy} onClick={create}>Create instance</Btn>}
      </div>
    </Modal>
  )
}

export function SimpleDialog({ d, onClose }: { d: any; onClose: () => void }) {
  const app = useApp()
  const [name, setName] = useState(d.inst?.name || '')
  if (d.kind === 'delete')
    return <Modal onClose={onClose} w={420}><DialogCard kind="warn" title={`Delete “${d.inst.name}”?`} body="Its mods, worlds and settings will be permanently removed. This can’t be undone." actions={<><Btn v="secondary" onClick={onClose}>Cancel</Btn><Btn v="danger" icon="trash" onClick={async () => { await app.removeInstance(d.inst.id); onClose() }}>Delete</Btn></>} /></Modal>
  return <Modal onClose={onClose} w={420}><DialogCard title="Rename instance" actions={<><Btn v="secondary" onClick={onClose}>Cancel</Btn><Btn v="primary" disabled={!name.trim()} onClick={async () => { await app.rename(d.inst.id, name); onClose() }}>Save</Btn></>}><Input autoFocus value={name} onChange={(e: any) => setName(e.target.value)} /></DialogCard></Modal>
}

/** Brings in instances from Prism Launcher, PolyMC or MultiMC. The originals are copied, never changed. */
export function ImportDialog({ onClose }: { onClose: () => void }) {
  const app = useApp()
  const [list, setList] = useState<Importable[] | null>(null)
  const [path, setPath] = useState('')
  const [busy, setBusy] = useState('')
  const [err, setErr] = useState('')
  useEffect(() => {
    if (!isDesktop()) { setList([]); return }
    backend.listImportable().then(setList).catch(() => setList([]))
  }, [])
  const go = async (p: string) => {
    setBusy(p); setErr('')
    try {
      const clean = p.trim().replace(/"/g, '').toLowerCase()
      const isFile = clean.endsWith('.zip') || clean.endsWith('.mrpack')
      const b = isFile ? await backend.importFile(p) : await backend.importInstance(p)
      app.setInsts((prev: any[]) => [...prev, { id: b.id, name: b.name, group: b.group || 'Imported', mc: b.version, loader: b.loader, mods: b.mods, ram: b.ram, last: b.last, jvmArgs: b.jvm_args ?? '', width: b.width ?? 0, height: b.height ?? 0, played: b.played ?? 0, server: b.server ?? '' }])
      app.openInstance(b.id)
      onClose()
    } catch (e) { setErr(errText(e)); setBusy('') }
  }
  return (
    <Modal onClose={onClose} w={540}>
      <div className="p-7">
        <h2 className="text-[1.375rem] font-semibold tracking-tight">Import an instance or modpack</h2>
        <p className="text-[0.8125rem] text-mu mt-1.5 leading-relaxed">Copies an instance from Prism Launcher, PolyMC or MultiMC, with its mods, worlds and settings. You can also import a modpack file: a Modrinth pack (.mrpack), a CurseForge pack (.zip), or a Prism export (.zip). The original isn't changed.</p>
        <div className="mt-5 space-y-2 max-h-72 overflow-auto scroll">
          {list === null && <div className="text-sm text-mu py-4">Looking for instances…</div>}
          {list && !list.length && <div className="rounded-2xl bg-s2 px-4 py-5 text-sm text-mu">No instances found in the usual places. Paste the instance's folder below instead.</div>}
          {list?.map((x) => (
            <div key={x.path} className="flex items-center gap-3 rounded-2xl bg-s2 px-4 py-3">
              <div className="flex-1 min-w-0">
                <div className="text-[0.8438rem] font-medium truncate">{x.name}</div>
                <div className="text-xs text-mu mt-0.5">{x.mc} · {x.loader} · {x.source}</div>
              </div>
              <Btn v="primary" size="sm" loading={busy === x.path} disabled={!!busy && busy !== x.path} onClick={() => go(x.path)}>Import</Btn>
            </div>
          ))}
        </div>
        <div className="mt-5">
          <div className="text-xs text-mu mb-2">Or paste an instance folder, or the path of a .zip or .mrpack file</div>
          <div className="flex gap-2">
            <Input placeholder="C:\Users\you\AppData\Roaming\PrismLauncher\instances\My Pack" value={path} onChange={(e: any) => setPath(e.target.value)} className="flex-1" />
            <Btn v="secondary" loading={!!path && busy === path} disabled={!path.trim() || (!!busy && busy !== path)} onClick={() => go(path)}>Import</Btn>
          </div>
        </div>
        {err && <div className="mt-3 text-xs text-red-500 break-words">{err}</div>}
      </div>
      <div className="px-7 pb-7 flex justify-end"><Btn v="ghost" onClick={onClose}>Close</Btn></div>
    </Modal>
  )
}
