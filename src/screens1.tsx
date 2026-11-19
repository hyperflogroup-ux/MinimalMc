import { useEffect, useRef, useState, type ReactNode } from 'react'
import { Blk, Btn, Icon, Input, MoreBtn, PageHead, Panel, Pill, PillMenu, Row, Select, Tip, Toggle, useApp, useMenu } from './ui'
import { backend, isDesktop, type Backup, type CompatFinding, type CompatReport, type Diagnosis, type DoctorReport, type InstanceFile, type LaunchStatus, type Shot, type Snapshot, type UpdateReport } from './backend'
import { GROUPS, LOADERS, type Inst } from './data'
import { restoreBisect, savedBisect } from './bisect'
import { addMedia, listMedia, removeMedia, type MediaItem } from './mediaStore'

const lt = (l: string) => (l === 'Vanilla' ? 'n' : 'ac')
const fmtSize = (n: number) => (n >= 1073741824 ? `${(n / 1073741824).toFixed(1)} GB` : n >= 1048576 ? `${(n / 1048576).toFixed(1)} MB` : n === 0 ? '0 KB' : `${Math.max(1, Math.round(n / 1024))} KB`)
const openFolder = (app: any, i: Inst) => backend.openInstanceFolder(i.id).catch((e) => app.notify(String(e)))

/** The "+" on an instance: jump to Modrinth with this instance already chosen as the target. */
function useAddMenu(i: Inst) {
  const app = useApp()
  const go = (route: string) => { app.setSel(i.id); app.go(route) }
  return useMenu([
    { label: 'Add mods', icon: 'puzzle', disabled: i.loader === 'Vanilla', onClick: () => go('mods') },
    { label: 'Add resource packs', icon: 'image', onClick: () => go('packs') },
    { label: 'Add shaders', icon: 'sun', onClick: () => go('shaders') },
  ])
}

function AddBtn({ m }: { m: ReturnType<typeof useMenu> }) {
  return (
    <Tip label="Add mods, packs or shaders">
      <Btn v="ghost" size="sm" icon="plus" onClick={m.openFrom} aria-label="Add content" />
    </Tip>
  )
}

function useInstMenu(i: Inst) {
  const app = useApp()
  return useMenu([
    { label: 'Open', icon: 'box', onClick: () => app.openInstance(i.id) },
    { label: 'Settings', icon: 'edit', onClick: () => app.openInstance(i.id, 'Settings') },
    { label: 'Open folder', icon: 'folder', onClick: () => openFolder(app, i) },
    { label: 'Duplicate', icon: 'copy', onClick: () => app.duplicate(i) },
    { label: 'Rename', icon: 'edit', onClick: () => app.setDialog({ kind: 'rename', inst: i }) },
    { sep: true },
    { label: 'Delete', icon: 'trash', danger: true, onClick: () => app.setDialog({ kind: 'delete', inst: i }) },
  ])
}

/** "Today, 14:05" / "Yesterday" / a date. The backend stores the last launch as unix seconds. */
const fmtLast = (l: string) => {
  if (!/^\d+$/.test(l)) return l === 'Never' ? 'Not played yet' : l
  const d = new Date(Number(l) * 1000)
  const now = new Date()
  if (d.toDateString() === now.toDateString()) return `Today, ${d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}`
  const y = new Date(now.getTime() - 86400000)
  return d.toDateString() === y.toDateString() ? 'Yesterday' : d.toLocaleDateString()
}

const fmtPlayed = (s?: number) => {
  if (!s) return 'None yet'
  const h = Math.floor(s / 3600)
  const m = Math.floor((s % 3600) / 60)
  return h ? `${h}h ${m}m` : m ? `${m}m` : 'Under a minute'
}

let lastNotice = ''

/** Downloads what's missing (first launch only), then starts the game with the offline account. */
export function PlayBtn({ i, size = 'md' }: { i: Inst; size?: string }) {
  const app = useApp()
  const [st, setSt] = useState<LaunchStatus | null>(null)
  const was = useRef(false)
  const poll = () => backend.launchStatus(i.id).then(setSt).catch(() => {})
  const active = !!st && (st.busy || st.running)
  useEffect(() => { if (isDesktop()) poll() }, [i.id, app.pulse])
  useEffect(() => {
    if (!active) return
    const t = setInterval(poll, 700)
    return () => clearInterval(t)
  }, [active, i.id])
  // When a run ends: refresh "last played" and show the reason if it failed.
  useEffect(() => {
    if (was.current && !active) {
      app.reload()
      if (st?.error) app.notify(st.error)
      if (st?.stage === 'Repaired') app.notify('Repair finished. All game files are verified.')
      if (st?.stage === 'Installed' && st.notice) app.notify(st.notice)
      if (st?.error?.startsWith('Minecraft closed unexpectedly')) app.openInstance(i.id, 'Doctor')
    }
    was.current = active
  }, [active])
  // Non-fatal problems (for example, a skin that couldn't be applied) are shown once per launch.
  useEffect(() => {
    if (!active) lastNotice = ''
    else if (st?.running && st.notice && st.notice !== lastNotice) { lastNotice = st.notice; app.notify(st.notice) }
  }, [active, st?.running, st?.notice])
  const play = async () => {
    if (!isDesktop()) return app.notify('Launching needs the desktop app.')
    if (!app.account) { app.notify('Choose a player name first.'); app.go('settings'); return }
    try {
      if (i.loader !== 'Vanilla' && i.mods > 0) {
        const r = await backend.checkCompat(i.id).catch(() => null)
        const errs = r ? r.findings.filter((f) => f.level === 'error').length : 0
        if (errs) app.notify(`Heads up: ${errs} mod ${errs === 1 ? 'problem' : 'problems'} found. Open the Mods tab to fix ${errs === 1 ? 'it' : 'them'}.`)
      }
      await backend.launchInstance(i.id, app.account)
      was.current = true
      setSt({ stage: 'Preparing', done: 0, total: 0, busy: true, running: false, error: null, notice: null })
    } catch (e) { app.notify(String(e)) }
  }
  const pct = st && st.total ? Math.round((st.done / st.total) * 100) : 0
  const label = st?.running ? 'Running' : st?.busy ? `${st.stage}${st.total ? ` ${pct}%` : ''}` : 'Play'
  return (
    <Btn v="primary" size={size} icon={active ? undefined : 'play'} loading={!!st?.busy} disabled={active} className={size === 'sm' ? 'min-w-36' : 'min-w-60'} onClick={play}>{label}</Btn>
  )
}

export function InstCard({ i, view = 'grid', selected, onSelect, compact }: { i: Inst; view?: string; selected?: boolean; onSelect?: () => void; compact?: boolean }) {
  const app = useApp()
  const m = useInstMenu(i)
  const add = useAddMenu(i)
  const ring = selected ? 'ring-2 ring-ac/70' : 'hover:bg-s2'
  const common = { draggable: true, onDragStart: (e: any) => e.dataTransfer.setData('id', String(i.id)), onContextMenu: m.onContext, onClick: onSelect, onDoubleClick: () => app.openInstance(i.id) }
  const open = (e: any) => { e.stopPropagation(); app.openInstance(i.id) }
  if (view === 'list')
    return (
      <div {...common} className={`group flex items-center gap-4 h-14 px-3 rounded-2xl cursor-default transition bg-s1 ${ring}`}>
        <Icon n="grip" size={14} className="text-mu2 opacity-0 group-hover:opacity-100" />
        <Blk seed={i.id} size={34} />
        <div className="w-56 min-w-0">
          <div className="text-[0.8438rem] font-medium truncate">{i.name}</div>
          <div className="text-[0.7188rem] text-mu">{fmtLast(i.last)}</div>
        </div>
        <div className="flex-1 flex gap-1.5"><Pill>{i.mc}</Pill><Pill tone={lt(i.loader)}>{i.loader}</Pill>{i.mods > 0 && <Pill>{i.mods} mods</Pill>}</div>
        <span className="text-xs text-mu w-14 hidden md:block">{i.ram} GB</span>
        <AddBtn m={add} />
        <Btn v="secondary" size="sm" onClick={open}>Open</Btn>
        <MoreBtn m={m} />
        {m.node}
        {add.node}
      </div>
    )
  return (
    <div {...common} className={`group relative rounded-[16px] bg-s1 cursor-default transition ${compact ? 'p-3.5' : 'p-4'} ${ring}`} style={{ boxShadow: selected ? undefined : '0 0 0 1px var(--line)' }}>
      <div className="flex items-start gap-3">
        <Blk seed={i.id} size={compact ? 38 : 44} />
        <div className="min-w-0 flex-1">
          <div className="text-[0.8438rem] font-medium truncate">{i.name}</div>
          <div className="text-[0.7188rem] text-mu mt-0.5">{fmtLast(i.last)}</div>
        </div>
        <div className="flex items-center -mr-1 -mt-1"><AddBtn m={add} /><div className="opacity-0 group-hover:opacity-100 transition"><MoreBtn m={m} /></div></div>
      </div>
      <div className="flex flex-wrap gap-1.5 mt-3.5">
        <Pill>{i.mc}</Pill>
        <Pill tone={lt(i.loader)}>{i.loader}</Pill>
        {!compact && i.mods > 0 && <Pill>{i.mods} mods</Pill>}
        {!compact && <Pill icon="cpu">{i.ram} GB</Pill>}
      </div>
      <div className="mt-3.5 flex justify-end"><Btn v="secondary" size="sm" onClick={open}>Open</Btn></div>
      {m.node}
      {add.node}
    </div>
  )
}

const OK_TYPES = ['image/jpeg', 'image/png', 'image/webp', 'video/mp4']
type Slide = { k: string; t: string; d: string; hue?: number; seed?: number; url?: string; kind?: 'image' | 'video'; key?: number }
const DEFAULT_SLIDE: Slide = { k: 'MinimalMc', t: 'Make it yours', d: 'Add your own screenshots or videos with “Add media”.', hue: 200, seed: 3 }

function Scene({ hue, seed }: { hue: number; seed: number }) {
  let s = seed * 7919 + 1
  const r = () => ((s = (s * 1664525 + 1013904223) % 4294967296) / 4294967296)
  const layer = (base: number, amp: number, col: string) => {
    const out: ReactNode[] = []
    let h = base
    for (let x = 0; x < 80; x++) {
      if (r() > 0.55) h = Math.max(base - amp, Math.min(base + amp, h + (r() > 0.5 ? 1 : -1)))
      out.push(<rect key={col + x} x={x} y={h} width="1" height={30 - h} fill={col} />)
    }
    return out
  }
  return (
    <svg viewBox="0 0 80 30" preserveAspectRatio="xMidYMid slice" shapeRendering="crispEdges" className="absolute inset-0 size-full">
      <defs><linearGradient id={`sky${seed}`} x1="0" y1="0" x2="0" y2="1"><stop offset="0" stopColor={`hsl(${hue} 40% 14%)`} /><stop offset="1" stopColor={`hsl(${hue} 45% 36%)`} /></linearGradient></defs>
      <rect width="80" height="30" fill={`url(#sky${seed})`} />
      <rect x="58" y="5" width="5" height="5" fill={`hsl(${hue} 80% 88%)`} opacity=".9" />
      {layer(17, 3, `hsl(${hue} 30% 26%)`)}
      {layer(21, 2, `hsl(${hue} 28% 16%)`)}
      {layer(25, 1, `hsl(${hue} 25% 8%)`)}
    </svg>
  )
}

/** Home hero. Shows a simple built-in scene until you add your own images or videos. */
function MediaBox({ instanceId }: { instanceId: number }) {
  const [media, setMedia] = useState<Slide[]>([])
  const [i, setI] = useState(0)
  const [drag, setDrag] = useState(false)
  const [err, setErr] = useState('')
  const input = useRef<HTMLInputElement>(null)
  const urls = useRef<string[]>([])
  const items = media.length ? media : [DEFAULT_SLIDE]
  const sl = items[Math.min(i, items.length - 1)]

  const toSlide = (m: MediaItem): Slide => {
    const url = URL.createObjectURL(m.blob)
    urls.current.push(url)
    const video = m.type.startsWith('video/')
    return { k: video ? 'Video' : 'Image', t: m.name.replace(/\.[^.]+$/, ''), d: `${(m.blob.size / 1048576).toFixed(1)} MB`, url, kind: video ? 'video' : 'image', key: m.key }
  }
  // Each instance has its own media, loaded from the app's saved data.
  useEffect(() => {
    let live = true
    setMedia([])
    setI(0)
    setErr('')
    listMedia(instanceId).then((list) => { if (live) setMedia(list.map(toSlide)) }).catch(() => {})
    return () => {
      live = false
      urls.current.forEach((u) => URL.revokeObjectURL(u))
      urls.current = []
    }
  }, [instanceId])
  useEffect(() => {
    if (sl.kind === 'video' || items.length < 2) return
    const t = setInterval(() => setI((x) => (x + 1) % items.length), 7000)
    return () => clearInterval(t)
  }, [sl, items.length])
  const add = async (files: FileList | File[] | null) => {
    if (!files) return
    const all = Array.from(files)
    const good = all.filter((f) => OK_TYPES.includes(f.type) || /\.(jpe?g|png|webp|mp4)$/i.test(f.name))
    setErr(good.length < all.length ? 'Only JPG, JPEG, PNG, WEBP and MP4 files are supported.' : '')
    if (!good.length) return
    const added: Slide[] = []
    for (const f of good) {
      const video = f.type === 'video/mp4' || /\.mp4$/i.test(f.name)
      try {
        const saved = await addMedia(instanceId, f, f.name, f.type || (video ? 'video/mp4' : 'image/png'))
        added.push(toSlide(saved))
      } catch {
        setErr("Couldn't save that file, so it will be gone after a restart.")
        const url = URL.createObjectURL(f)
        urls.current.push(url)
        added.push({ k: video ? 'Video' : 'Image', t: f.name.replace(/\.[^.]+$/, ''), d: `${(f.size / 1048576).toFixed(1)} MB`, url, kind: video ? 'video' : 'image' })
      }
    }
    setI(media.length)
    setMedia((p) => [...p, ...added])
  }
  const remove = () => {
    if (sl.key !== undefined) removeMedia(sl.key).catch(() => {})
    setMedia((p) => p.filter((x) => x !== sl))
    setI(0)
  }
  return (
    <div onDragOver={(e) => { e.preventDefault(); setDrag(true) }} onDragLeave={() => setDrag(false)} onDrop={(e) => { e.preventDefault(); setDrag(false); add(e.dataTransfer.files) }} className="relative h-[26rem] rounded-[24px] overflow-hidden mb-4 bg-s1" style={{ boxShadow: '0 0 0 1px var(--line)' }}>
      <input ref={input} type="file" multiple accept=".jpg,.jpeg,.png,.webp,.mp4,image/jpeg,image/png,image/webp,video/mp4" className="hidden" onChange={(e) => { add(e.target.files); e.target.value = '' }} />
      <div key={sl.url || sl.t} className="absolute inset-0 fade">
        {sl.kind === 'video' ? <video src={sl.url} autoPlay muted loop playsInline className="size-full object-cover" /> : sl.kind === 'image' ? <img src={sl.url} alt={sl.t} className="size-full object-cover" /> : <Scene hue={sl.hue!} seed={sl.seed!} />}
      </div>
      <div className="absolute inset-0 pointer-events-none" style={{ background: 'linear-gradient(90deg, rgba(0,0,0,.75), rgba(0,0,0,.1) 70%)' }} />
      <div key={'t' + i} className="absolute left-8 bottom-8 max-w-md fade text-white">
        <Pill className="!bg-white/15 !text-white backdrop-blur" icon={sl.kind === 'video' ? 'play' : undefined}>{sl.k}</Pill>
        <div className="text-[1.5rem] font-semibold tracking-tight mt-3 leading-8 truncate">{sl.t}</div>
        <p className="text-[0.8125rem] text-white/70 mt-1.5 leading-relaxed">{sl.d}</p>
      </div>
      <div className="absolute right-6 top-6 flex gap-2">
        {sl.url && <Tip label="Remove this media"><Btn v="secondary" icon="trash" onClick={remove} aria-label="Remove media" className="!bg-black/40 !text-white backdrop-blur" /></Tip>}
        <Btn v="secondary" icon="plus" onClick={() => input.current?.click()} className="!bg-black/40 !text-white backdrop-blur">Add media</Btn>
      </div>
      {items.length > 1 && (
        <div className="absolute right-6 bottom-6 flex items-center gap-3">
          <div className="flex gap-1.5">{items.map((_, k) => <button key={k} onClick={() => setI(k)} aria-label={`Slide ${k + 1}`} className={`h-2 rounded-full transition-all ${k === i ? 'w-6 bg-white' : 'w-2 bg-white/40'}`} />)}</div>
          <Tip label="Next"><Btn v="secondary" icon="chev" onClick={() => setI((i + 1) % items.length)} aria-label="Next" className="!bg-white/15 !text-white backdrop-blur" /></Tip>
        </div>
      )}
      {err && <div className="absolute left-8 top-6"><Pill tone="bad" icon="alert">{err}</Pill></div>}
      {drag && <div className="absolute inset-3 rounded-[18px] border-2 border-dashed border-white/70 bg-black/60 grid place-items-center text-white text-[0.9375rem] font-semibold fade"><span className="flex items-center gap-2"><Icon n="upload" />Drop JPG, PNG, WEBP or MP4</span></div>}
    </div>
  )
}

function EmptyInstances({ title = "You don't have any instances yet." }: { title?: string }) {
  const app = useApp()
  return (
    <div className="max-w-6xl mx-auto text-center py-24">
      <h1 className="text-[2rem] leading-10 font-semibold tracking-tight">Welcome to MinimalMc</h1>
      <p className="text-[0.9375rem] text-mu mt-2 mb-6">{title}</p>
      <div className="flex gap-2 justify-center">
        <Btn v="primary" icon="plus" onClick={() => app.setDialog({ kind: 'create' })}>Create instance</Btn>
        <Btn v="secondary" icon="download" onClick={() => app.setDialog({ kind: 'import' })}>Import from Prism</Btn>
      </div>
    </div>
  )
}

export function Home() {
  const app = useApp()
  const insts: Inst[] = app.insts
  const hour = new Date().getHours()
  if (!app.ready) return null
  if (!insts.length) return <EmptyInstances />
  const f = insts.find((x) => x.id === app.sel) ?? insts[0]
  const others = insts.filter((x) => x.id !== f.id).slice(0, 4)
  return (
    <div className="max-w-6xl mx-auto">
      <div className="mb-8">
        <h1 className="text-[2rem] leading-10 font-semibold tracking-tight">{hour < 12 ? 'Good morning.' : hour < 18 ? 'Good afternoon.' : 'Good evening.'}</h1>
      </div>
      <MediaBox key={f.id} instanceId={f.id} />
      <Panel className="relative overflow-hidden p-7 mb-10">
        <div className="absolute inset-0 opacity-[.5] pointer-events-none" style={{ background: 'radial-gradient(600px 220px at 85% 0%, color-mix(in oklab, var(--ac) 16%, transparent), transparent)' }} />
        <div className="relative flex items-center gap-6 flex-wrap">
          <Blk seed={f.id} size={88} r={22} />
          <div className="flex-1 min-w-60">
            <div className="text-[1.375rem] font-semibold tracking-tight">{f.name}</div>
            <div className="flex flex-wrap gap-1.5 mt-3">
              <Pill>{f.mc}</Pill><Pill tone="ac">{f.loader}</Pill>{f.mods > 0 && <Pill>{f.mods} mods</Pill>}<Pill icon="clock">{fmtLast(f.last)}</Pill>
            </div>
          </div>
          <div className="flex items-center gap-2">
            <Btn v="secondary" onClick={() => app.openInstance(f.id)}>Open</Btn>
            <PlayBtn i={f} />
          </div>
        </div>
      </Panel>
      {others.length > 0 && (
        <>
          <div className="flex items-center justify-between mb-3.5">
            <h2 className="text-[1rem] font-semibold tracking-tight">Your instances</h2>
            <Btn v="ghost" size="sm" onClick={() => app.go('instances')}>View all</Btn>
          </div>
          <div className="grid gap-3 grid-cols-[repeat(auto-fill,minmax(210px,1fr))]">
            {others.map((i) => <InstCard key={i.id} i={i} compact selected={app.sel === i.id} onSelect={() => app.setSel(i.id)} />)}
          </div>
        </>
      )}
    </div>
  )
}

export function Instances() {
  const app = useApp()
  const [view, setView] = useState('grid')
  const [q, setQ] = useState('')
  const [loader, setLoader] = useState('All')
  const [sort, setSort] = useState('Name')
  const [over, setOver] = useState('')
  const all: Inst[] = app.insts
  if (!app.ready) return null
  if (!all.length) return <EmptyInstances />
  const groups = [...GROUPS, ...Array.from(new Set(all.map((i) => i.group))).filter((g) => !GROUPS.includes(g))]
  const list = all
    .filter((i) => i.name.toLowerCase().includes(q.toLowerCase()) && (loader === 'All' || i.loader === loader))
    .sort((a, b) => (sort === 'Name' ? a.name.localeCompare(b.name) : sort === 'Mods' ? b.mods - a.mods : b.id - a.id))
  const sel = all.find((i) => i.id === app.sel)
  return (
    <div className="flex gap-6 max-w-[1200px] mx-auto">
      <div className="flex-1 min-w-0">
        <PageHead title="Instances" sub={`${all.length} ${all.length === 1 ? 'instance' : 'instances'}`}>
          <div className="flex gap-2">
            <Btn v="secondary" icon="download" onClick={() => app.setDialog({ kind: 'import' })}>Import</Btn>
            <Btn v="primary" icon="plus" onClick={() => app.setDialog({ kind: 'create' })}>Create instance</Btn>
          </div>
        </PageHead>
        <div className="flex items-center gap-2 mb-6 flex-wrap">
          <Input icon="search" placeholder="Search instances" value={q} onChange={(e: any) => setQ(e.target.value)} className="w-64" />
          <PillMenu label={loader === 'All' ? 'Filter' : loader} icon="filter" active={loader !== 'All'} items={['All', ...LOADERS].map((l) => ({ label: l, icon: loader === l ? 'check' : undefined, onClick: () => setLoader(l) }))} />
          <PillMenu label={`Sort: ${sort}`} icon="sort" items={['Name', 'Mods', 'Recently added'].map((l) => ({ label: l, icon: sort === l ? 'check' : undefined, onClick: () => setSort(l) }))} />
          <div className="flex-1" />
        </div>
        <div className="space-y-8">
          {groups.map((g) => {
            const items = list.filter((i) => i.group === g)
            if (!items.length && over !== g) return null
            return (
              <section key={g} onDragOver={(e) => { e.preventDefault(); setOver(g) }} onDragLeave={() => setOver('')} onDrop={(e) => { const id = +e.dataTransfer.getData('id'); setOver(''); if (id) app.update(id, { group: g }) }} className={`rounded-2xl transition ${over === g ? 'bg-ac/8 outline-2 outline-dashed outline-ac/40 outline-offset-4' : ''}`}>
                <div className="flex items-center gap-2 mb-3 text-[0.8125rem] font-medium"><span>{g}</span><span className="text-mu2 text-xs">{items.length}</span></div>
                <div className={view === 'grid' ? 'grid gap-3 grid-cols-[repeat(auto-fill,minmax(240px,1fr))]' : 'grid gap-1.5'}>
                  {items.map((i) => <InstCard key={i.id} i={i} view={view} selected={app.sel === i.id} onSelect={() => app.setSel(i.id)} />)}
                </div>
              </section>
            )
          })}
          {!list.length && <div className="text-center text-mu text-sm py-20">No instances match your filters.</div>}
        </div>
        {all.length > 1 && <p className="text-[0.7188rem] text-mu2 mt-8 text-center">Drag an instance onto a group to move it. Right-click for more.</p>}
      </div>
      {sel && (
        <aside className="hidden xl:block w-64 shrink-0 sticky top-0 self-start fade">
          <Panel className="p-5">
            <Blk seed={sel.id} size={56} r={16} />
            <div className="text-[0.9375rem] font-semibold mt-3 tracking-tight">{sel.name}</div>
            <div className="text-xs text-mu mt-0.5">{sel.group}</div>
            <div className="mt-4 space-y-2.5 text-[0.7812rem]">
              {[['Minecraft', sel.mc], ['Loader', sel.loader], ['Mods', sel.mods || '—'], ['Memory', `${sel.ram} GB`]].map(([k, v]) => (
                <div key={k as string} className="flex justify-between"><span className="text-mu">{k}</span><span className="font-medium">{v}</span></div>
              ))}
            </div>
            <div className="flex gap-2 mt-5"><Btn v="secondary" onClick={() => app.openInstance(sel.id)}>Open</Btn></div>
          </Panel>
        </aside>
      )}
    </div>
  )
}

const TABS = ['Overview', 'Mods', 'Resource Packs', 'Shaders', 'Worlds', 'Screenshots', 'Backups', 'Logs', 'Doctor', 'Settings']
const FILE_KINDS: Record<string, { kind: string; noun: string; route: string; removable: boolean }> = {
  Mods: { kind: 'mods', noun: 'mods', route: 'mods', removable: true },
  'Resource Packs': { kind: 'resourcepacks', noun: 'resource packs', route: 'packs', removable: true },
  Shaders: { kind: 'shaderpacks', noun: 'shaders', route: 'shaders', removable: true },
  Worlds: { kind: 'saves', noun: 'worlds', route: '', removable: false },
}

export function InstanceDetail() {
  const app = useApp()
  const i: Inst | undefined = app.insts.find((x: Inst) => x.id === app.sel) ?? app.insts[0]
  if (!i) return <EmptyInstances />
  return <Detail i={i} />
}

function Detail({ i }: { i: Inst }) {
  const app = useApp()
  const tab: string = TABS.includes(app.tab) ? app.tab : 'Overview'
  const m = useInstMenu(i)
  const add = useAddMenu(i)
  return (
    <div className="max-w-5xl mx-auto">
      <button onClick={() => app.go('instances')} className="text-xs text-mu hover:text-fg inline-flex items-center gap-1 mb-4"><Icon n="chev" size={12} className="rotate-180" />Instances</button>
      <div className="flex items-center gap-4 mb-6">
        <Blk seed={i.id} size={56} r={16} />
        <div className="flex-1 min-w-0">
          <h1 className="text-[1.5rem] font-semibold tracking-tight leading-7">{i.name}</h1>
          <div className="flex gap-1.5 mt-2"><Pill>{i.mc}</Pill><Pill tone={lt(i.loader)}>{i.loader}</Pill><Pill>{i.group}</Pill></div>
        </div>
        <Btn v="secondary" icon="plus" onClick={add.openFrom}>Add</Btn>
        <MoreBtn m={m} />
        <PlayBtn i={i} />
        {m.node}
        {add.node}
      </div>
      <div className="flex gap-1 mb-7 overflow-x-auto scroll pb-1">
        {TABS.map((t) => (
          <button key={t} onClick={() => app.setTab(t)} className={`h-10 px-5 rounded-full text-[0.8125rem] font-medium whitespace-nowrap transition ${tab === t ? 'bg-s3 text-fg' : 'text-mu hover:text-fg hover:bg-fg/[.05]'}`}>{t}</button>
        ))}
      </div>
      <div key={tab + i.id} className="fade">
        {tab === 'Overview' && <Overview i={i} />}
        {FILE_KINDS[tab] && <FilesTab i={i} tab={tab} />}
        {tab === 'Screenshots' && <ScreenshotsTab i={i} />}
        {tab === 'Backups' && <BackupsTab i={i} />}
        {tab === 'Logs' && <LogsTab i={i} />}
        {tab === 'Doctor' && <DoctorTab i={i} />}
        {tab === 'Settings' && <InstSettings key={i.id} i={i} />}
      </div>
    </div>
  )
}

function Overview({ i }: { i: Inst }) {
  const app = useApp()
  const [done, setDone] = useState<{ path: string; files: number; size: number } | null>(null)
  const [mrBusy, setMrBusy] = useState(false)
  const doMrpack = async () => {
    setMrBusy(true)
    try {
      const r = await backend.exportMrpack(i.id)
      setDone({ path: r.path, files: r.linked + r.bundled, size: r.size })
      app.notify(`Exported for Modrinth: ${r.linked} files linked, ${r.bundled} included in the pack.`)
    } catch (e) { app.notify(String(e)) } finally { setMrBusy(false) }
  }
  const repair = async () => {
    try {
      await backend.repairInstance(i.id)
      app.kick()
      app.notify('Repairing. The Play button shows the progress.')
    } catch (e) { app.notify(String(e)) }
  }
  const rows: [string, string | number][] = [['Minecraft', i.mc], ['Loader', i.loader], ['Memory', `${i.ram} GB`], ['Mods', i.mods || 'None'], ['Group', i.group], ['Last played', fmtLast(i.last)], ['Play time', fmtPlayed(i.played)]]
  return (
    <Panel className="p-5 max-w-xl">
      <div className="divide-y divide-line">
        {rows.map(([k, v]) => (
          <div key={k} className="flex justify-between py-2.5 text-[0.8125rem]"><span className="text-mu">{k}</span><span className="font-medium">{v}</span></div>
        ))}
      </div>
      <div className="flex gap-2 mt-6 flex-wrap">
        <Btn v="secondary" size="sm" icon="folder" onClick={() => openFolder(app, i)}>Open folder</Btn>
        <Btn v="secondary" size="sm" icon="copy" onClick={() => app.duplicate(i)}>Duplicate</Btn>
        <Btn v="secondary" size="sm" icon="edit" onClick={() => app.setDialog({ kind: 'rename', inst: i })}>Rename</Btn>
        <Btn v="danger" size="sm" icon="trash" onClick={() => app.setDialog({ kind: 'delete', inst: i })}>Delete</Btn>
      </div>
      <div className="mt-6 pt-5 border-t border-line">
        <div className="text-[0.8438rem] font-medium">Share, back up or fix this instance</div>
        <p className="text-xs text-mu mt-1 leading-relaxed">Export saves the instance as a Modrinth modpack (.mrpack) that can be shared and imported elsewhere. It needs internet, to look your mods up. Repair re-checks the game files and downloads anything missing or damaged.</p>
        <div className="flex items-center gap-3 mt-3 flex-wrap">
          <Btn v="secondary" size="sm" icon="upload" loading={mrBusy} onClick={doMrpack}>Export for Modrinth (.mrpack)</Btn>
          <Btn v="secondary" size="sm" icon="wrench" onClick={repair}>Repair game files</Btn>
        </div>
        {done && (
          <div className="mt-3 flex items-center gap-2 text-xs text-mu">
            <span className="break-all flex-1">Saved to {done.path}</span>
            <Btn v="ghost" size="sm" onClick={() => backend.openExportsFolder().catch((e) => app.notify(String(e)))}>Show folder</Btn>
          </div>
        )}
      </div>
    </Panel>
  )
}

/** The mod compatibility check. It re-runs whenever the mod list changes, and always says what happened. */
function CompatPanel({ i, sig, onChanged }: { i: Inst; sig: string; onChanged: () => void }) {
  const app = useApp()
  const [rep, setRep] = useState<CompatReport | null>(null)
  const [err, setErr] = useState('')
  const [checking, setChecking] = useState(false)
  const [busy, setBusy] = useState('')
  const seq = useRef(0)
  const run = () => {
    if (!isDesktop()) { setErr('The checker needs the desktop app.'); return }
    const id = ++seq.current
    setChecking(true)
    setErr('')
    const timer = setTimeout(() => { if (id === seq.current) setErr('The check is taking too long. Click "Check again" to retry.') }, 20000)
    backend.checkCompat(i.id)
      .then((r) => { if (id === seq.current) { setRep(r); setErr('') } })
      .catch((e) => { if (id === seq.current) setErr(`The check failed: ${String(e)}`) })
      .finally(() => { clearTimeout(timer); if (id === seq.current) setChecking(false) })
  }
  useEffect(() => { run() }, [i.id, sig])
  const fix = async (f: CompatFinding) => {
    if (!f.fix) return
    setBusy(f.message)
    try {
      if (f.fix.kind === 'disable') await backend.setModEnabled(i.id, f.fix.target, false)
      else await backend.installModrinth(i.id, f.fix.target, 'mods')
      onChanged()
    } catch (e) {
      app.notify(f.fix.kind === 'install' ? `Couldn't install ${f.fix.target} automatically. Try searching for it in Mods. (${e})` : String(e))
    } finally { setBusy('') }
  }
  const n = rep ? rep.findings.length : 0
  const title = !rep
    ? err ? 'The compatibility check could not run' : 'Checking your mods…'
    : rep.checked === 0 ? 'No mods to check yet'
    : n ? `${n} possible ${n === 1 ? 'problem' : 'problems'} found`
    : `No problems found in ${rep.checked} ${rep.checked === 1 ? 'mod' : 'mods'}`
  return (
    <div className="rounded-2xl bg-s2 p-4 mb-5">
      <div className="flex items-center gap-2">
        <span className="text-[0.8438rem] font-medium flex-1">{title}</span>
        {checking && rep && <span className="text-xs text-mu">Checking…</span>}
        <Btn v="ghost" size="sm" onClick={run}>Check again</Btn>
      </div>
      {err && <div className="mt-2 text-xs text-red-500 leading-relaxed break-words">{err}</div>}
      {n > 0 && (
        <div className="mt-3 space-y-2">
          {rep!.findings.map((f) => (
            <div key={f.level + f.message} className="flex items-center gap-3 rounded-xl bg-s1 px-3 py-2.5">
              <Pill tone={f.level === 'error' ? 'bad' : 'warn'}>{f.level === 'error' ? 'Problem' : 'Warning'}</Pill>
              <span className="flex-1 min-w-0 text-[0.7812rem] leading-relaxed">{f.message}</span>
              {f.fix && <Btn v="secondary" size="sm" loading={busy === f.message} onClick={() => fix(f)}>{f.fix.label}</Btn>}
            </div>
          ))}
        </div>
      )}
    </div>
  )
}

/** Checks Modrinth for newer versions of the installed mods. The old file is kept so the update can be undone. */
function UpdatesPanel({ i, onChanged }: { i: Inst; onChanged: () => void }) {
  const app = useApp()
  const [rep, setRep] = useState<UpdateReport | null>(null)
  const [state, setState] = useState<'idle' | 'checking' | 'updating'>('idle')
  const [err, setErr] = useState('')
  const check = () => {
    if (!isDesktop()) return
    setState('checking')
    setErr('')
    backend.checkModUpdates(i.id).then(setRep).catch((e) => setErr(String(e))).finally(() => setState('idle'))
  }
  const apply = async (files: string[]) => {
    setState('updating')
    setErr('')
    try {
      try { await backend.createSnapshot(i.id, 'Before updating mods') } catch { /* the update can still go ahead */ }
      const r = await backend.updateMods(i.id, files)
      app.notify(`${r.updated.length} ${r.updated.length === 1 ? 'mod' : 'mods'} updated${r.failed.length ? `, ${r.failed.length} failed` : ''}.`)
      onChanged()
      check()
    } catch (e) { setErr(String(e)); setState('idle') }
  }
  const undo = async () => {
    try {
      const n = await backend.rollbackModUpdates(i.id)
      app.notify(`Put back ${n} previous ${n === 1 ? 'version' : 'versions'}.`)
      onChanged()
      check()
    } catch (e) { app.notify(String(e)) }
  }
  const n = rep ? rep.updates.length : 0
  const busy = state !== 'idle'
  return (
    <div className="rounded-2xl bg-s2 p-4 mb-5">
      <div className="flex items-center gap-2 flex-wrap">
        <div className="flex-1 min-w-0">
          <div className="text-[0.8438rem] font-medium">
            {!rep ? 'Mod updates' : n ? `${n} ${n === 1 ? 'update' : 'updates'} available` : `No updates found (${rep.checked} ${rep.checked === 1 ? 'mod' : 'mods'} checked)`}
          </div>
          {!rep && <div className="text-xs text-mu mt-0.5">Checks Modrinth for newer versions that fit this instance. The old version is kept, so you can undo.</div>}
        </div>
        {rep && rep.can_undo > 0 && <Btn v="ghost" size="sm" disabled={busy} onClick={undo}>Undo last update</Btn>}
        {n > 1 && <Btn v="primary" size="sm" loading={state === 'updating'} disabled={state === 'checking'} onClick={() => apply([])}>Update all</Btn>}
        <Btn v="secondary" size="sm" loading={state === 'checking'} disabled={state === 'updating'} onClick={check}>{rep ? 'Check again' : 'Check for updates'}</Btn>
      </div>
      {err && <div className="mt-2 text-xs text-red-500 break-words">{err}</div>}
      {rep && n > 0 && (
        <div className="mt-3 space-y-2">
          {rep.updates.map((u) => (
            <div key={u.file} className="flex items-center gap-3 rounded-xl bg-s1 px-3 py-2.5">
              <div className="flex-1 min-w-0">
                <div className="text-[0.7812rem] font-medium truncate">{u.file}</div>
                <div className="text-xs text-mu truncate">→ {u.new_file}{u.new_version ? ` (${u.new_version})` : ''}</div>
              </div>
              <Btn v="secondary" size="sm" disabled={busy} onClick={() => apply([u.file])}>Update</Btn>
            </div>
          ))}
        </div>
      )}
    </div>
  )
}

/** Copies of worlds kept inside the instance. Restoring never loses the world it replaces. */
function BackupsPanel({ i, refreshKey, onChanged }: { i: Inst; refreshKey: number; onChanged: () => void }) {
  const app = useApp()
  const [list, setList] = useState<Backup[] | null>(null)
  const [confirm, setConfirm] = useState('')
  const [busy, setBusy] = useState('')
  const load = () => backend.listBackups(i.id).then(setList).catch((e) => { setList([]); app.notify(String(e)) })
  useEffect(() => { if (isDesktop()) load(); else setList([]) }, [i.id, refreshKey])
  const restore = async (b: Backup) => {
    setBusy(b.name)
    try {
      await backend.restoreBackup(i.id, b.name)
      app.notify(`Restored ${b.world}. The version it replaced was saved as a backup.`)
      setConfirm('')
      load()
      onChanged()
    } catch (e) { app.notify(String(e)) } finally { setBusy('') }
  }
  const del = async (b: Backup) => {
    try { await backend.deleteBackup(i.id, b.name); setConfirm(''); load() } catch (e) { app.notify(String(e)) }
  }
  if (!list) return null
  if (!list.length) return <p className="text-xs text-mu mt-8">No backups yet. Click "Back up" on a world to save a copy of it.</p>
  return (
    <div className="mt-8">
      <div className="text-[0.8438rem] font-medium mb-2">Backups</div>
      <div className="space-y-0.5">
        {list.map((b) => (
          <div key={b.name} className="flex items-center gap-4 h-14 px-3 rounded-2xl hover:bg-s2 transition">
            <div className="flex-1 min-w-0">
              <div className="text-[0.8438rem] font-medium truncate">{b.world}</div>
              <div className="text-xs text-mu">{new Date(b.time).toLocaleString()}</div>
            </div>
            <span className="text-xs text-mu w-20 text-right">{fmtSize(b.size)}</span>
            {confirm === 'r' + b.name
              ? <Btn v="primary" size="sm" loading={busy === b.name} onClick={() => restore(b)}>Confirm restore</Btn>
              : <Btn v="secondary" size="sm" onClick={() => setConfirm('r' + b.name)}>Restore</Btn>}
            {confirm === 'd' + b.name
              ? <Btn v="danger" size="sm" onClick={() => del(b)}>Confirm delete</Btn>
              : <Tip label="Delete this backup"><Btn v="ghost" size="sm" icon="trash" aria-label="Delete backup" onClick={() => setConfirm('d' + b.name)} /></Tip>}
          </div>
        ))}
      </div>
    </div>
  )
}

function FilesTab({ i, tab }: { i: Inst; tab: string }) {
  const app = useApp()
  const cfg = FILE_KINDS[tab]
  const isMods = cfg.kind === 'mods'
  const [files, setFiles] = useState<InstanceFile[] | null>(null)
  const [confirm, setConfirm] = useState('')
  const [q, setQ] = useState('')
  const [bk, setBk] = useState(0)
  const [busyW, setBusyW] = useState('')
  const [pending, setPending] = useState(false)
  const load = () =>
    backend.listInstanceFiles(i.id, cfg.kind)
      .then((l) => {
        setFiles(l)
        if (cfg.kind === 'mods') {
          const n = l.filter((f) => f.name.endsWith('.jar')).length
          app.setInsts((p: Inst[]) => p.map((x) => (x.id === i.id && x.mods !== n ? { ...x, mods: n } : x)))
        }
      })
      .catch((e) => { setFiles([]); app.notify(String(e)) })
  useEffect(() => {
    setFiles(null); setConfirm(''); setQ('')
    if (isDesktop()) load(); else setFiles([])
  }, [i.id, cfg.kind])
  useEffect(() => { setPending(isMods && savedBisect(i.id) !== null) }, [i.id, cfg.kind, files])
  useEffect(() => {
    const h = () => { if (isDesktop()) load() }
    window.addEventListener('minimalmc-mods', h)
    return () => window.removeEventListener('minimalmc-mods', h)
  }, [i.id, cfg.kind])
  const remove = async (name: string) => {
    try { await backend.removeInstanceFile(i.id, cfg.kind, name); setConfirm(''); load() } catch (e) { app.notify(String(e)) }
  }
  const toggle = async (name: string, on: boolean) => {
    try { await backend.setModEnabled(i.id, name, on); load() } catch (e) { app.notify(String(e)) }
  }
  const backup = async (world: string) => {
    setBusyW(world)
    try {
      await backend.backupWorld(i.id, world)
      app.notify(`Backed up ${world}.`)
      setBk((n) => n + 1)
    } catch (e) { app.notify(String(e)) } finally { setBusyW('') }
  }
  const shown = (files ?? []).filter((f) => f.name.toLowerCase().includes(q.toLowerCase()))
  return (
    <div>
      {isMods && i.loader !== 'Vanilla' && files && <CompatPanel i={i} sig={files.map((f) => f.name).join('|')} onChanged={load} />}
      {isMods && pending && (
        <div className="rounded-2xl bg-s2 px-4 py-3 mb-5 flex items-center gap-3">
          <span className="text-[0.8125rem] flex-1">A Mod Bisect was interrupted, so some mods may still be turned off.</span>
          <Btn v="primary" size="sm" onClick={() => restoreBisect(i.id).then(() => { setPending(false); load() })}>Restore my mods</Btn>
        </div>
      )}
      {isMods && i.loader !== 'Vanilla' && <UpdatesPanel i={i} onChanged={load} />}
      <div className="flex items-center gap-2 mb-4 flex-wrap">
        <p className="text-[0.8125rem] text-mu flex-1">{files === null ? 'Loading…' : `${files.length} ${cfg.noun}`}</p>
        {isMods && i.loader !== 'Vanilla' && i.mods > 1 && <Btn v="secondary" size="sm" icon="search" onClick={() => app.setDialog({ kind: 'bisect', inst: i })}>Find the broken mod</Btn>}
        <Btn v="secondary" size="sm" icon="folder" onClick={() => openFolder(app, i)}>Open folder</Btn>
        {cfg.route && <Btn v="primary" size="sm" icon="plus" onClick={() => app.go(cfg.route)}>Browse {cfg.noun}</Btn>}
      </div>
      {files && files.length > 8 && <Input icon="search" placeholder={`Search ${cfg.noun}`} value={q} onChange={(e: any) => setQ(e.target.value)} className="mb-3 w-72" />}
      {files && !files.length && <div className="py-16 text-center text-sm text-mu">No {cfg.noun} yet.</div>}
      {files && files.length > 0 && !shown.length && <div className="py-10 text-center text-sm text-mu">Nothing matches “{q}”.</div>}
      <div className="space-y-0.5">
        {shown.map((f) => {
          const off = f.name.endsWith('.disabled')
          return (
            <div key={f.name} className={`flex items-center gap-4 h-14 px-3 rounded-2xl hover:bg-s2 transition ${off ? 'opacity-60' : ''}`}>
              <Blk seed={f.name.length} size={34} />
              <div className="flex-1 min-w-0 text-[0.8438rem] font-medium truncate">{off ? f.name.replace(/\.disabled$/, '') : f.name}</div>
              {off && <Pill>Off</Pill>}
              <span className="text-xs text-mu w-20 text-right">{fmtSize(f.size)}</span>
              {isMods && f.name.includes('.jar') && (
                <Tip label={off ? 'Turn on' : 'Turn off'}><span><Toggle on={!off} onChange={(v: boolean) => toggle(f.name, v)} /></span></Tip>
              )}
              {cfg.kind === 'saves' && f.is_dir && <Btn v="secondary" size="sm" loading={busyW === f.name} onClick={() => backup(f.name)}>Back up</Btn>}
              {cfg.removable && (confirm === f.name
                ? <Btn v="danger" size="sm" onClick={() => remove(f.name)}>Confirm remove</Btn>
                : <Tip label="Remove"><Btn v="ghost" size="sm" icon="trash" aria-label="Remove" onClick={() => setConfirm(f.name)} /></Tip>)}
            </div>
          )
        })}
      </div>
      {cfg.kind === 'saves' && <BackupsPanel i={i} refreshKey={bk} onChanged={load} />}
    </div>
  )
}

function ShotCard({ i, s, onOpen }: { i: Inst; s: Shot; onOpen: () => void }) {
  const [src, setSrc] = useState('')
  useEffect(() => {
    let live = true
    setSrc('')
    backend.screenshotThumb(i.id, s.name).then((b) => { if (live) setSrc(`data:image/png;base64,${b}`) }).catch(() => {})
    return () => { live = false }
  }, [i.id, s.name])
  return (
    <button onClick={onOpen} className="text-left rounded-2xl overflow-hidden bg-s1 hover:bg-s2 transition" style={{ boxShadow: '0 0 0 1px var(--line)' }}>
      <div className="aspect-video bg-s2 grid place-items-center overflow-hidden">
        {src ? <img src={src} alt={s.name} className="size-full object-cover" /> : <span className="text-xs text-mu">Loading…</span>}
      </div>
      <div className="px-3 py-2">
        <div className="text-[0.75rem] font-medium truncate">{s.name}</div>
        <div className="text-[0.6875rem] text-mu">{new Date(s.time).toLocaleString()}</div>
      </div>
    </button>
  )
}

/** The screenshots taken in the game, newest first. */
function ScreenshotsTab({ i }: { i: Inst }) {
  const app = useApp()
  const [shots, setShots] = useState<Shot[] | null>(null)
  const [show, setShow] = useState(24)
  const [view, setView] = useState<{ s: Shot; src: string } | null>(null)
  const [confirm, setConfirm] = useState(false)
  const load = () => backend.listScreenshots(i.id).then(setShots).catch((e) => { setShots([]); app.notify(String(e)) })
  useEffect(() => {
    setShots(null); setShow(24); setView(null)
    if (isDesktop()) load(); else setShots([])
  }, [i.id])
  useEffect(() => {
    if (!view) return
    const k = (e: KeyboardEvent) => { if (e.key === 'Escape') setView(null) }
    window.addEventListener('keydown', k)
    return () => window.removeEventListener('keydown', k)
  }, [view])
  const mime = (n: string) => (/\.png$/i.test(n) ? 'image/png' : 'image/jpeg')
  const open = async (s: Shot) => {
    setConfirm(false)
    setView({ s, src: '' })
    try {
      const b = await backend.screenshotFull(i.id, s.name)
      setView({ s, src: `data:${mime(s.name)};base64,${b}` })
    } catch (e) { app.notify(String(e)); setView(null) }
  }
  const del = async () => {
    if (!view) return
    try {
      await backend.removeInstanceFile(i.id, 'screenshots', view.s.name)
      setView(null)
      load()
    } catch (e) { app.notify(String(e)) }
  }
  const useOnHome = async () => {
    if (!view || !view.src) return
    try {
      const blob = await (await fetch(view.src)).blob()
      await addMedia(i.id, blob, view.s.name, blob.type || mime(view.s.name))
      app.notify("Added to this instance's Home banner.")
    } catch (e) { app.notify(String(e)) }
  }
  return (
    <div>
      <div className="flex items-center gap-2 mb-4 flex-wrap">
        <p className="text-[0.8125rem] text-mu flex-1">{shots === null ? 'Loading…' : `${shots.length} ${shots.length === 1 ? 'screenshot' : 'screenshots'}`}</p>
        <Btn v="secondary" size="sm" onClick={load}>Refresh</Btn>
        <Btn v="secondary" size="sm" icon="folder" onClick={() => openFolder(app, i)}>Open folder</Btn>
      </div>
      {shots && !shots.length && <div className="py-16 text-center text-sm text-mu">No screenshots yet. Press F2 in the game to take one.</div>}
      {shots && shots.length > 0 && (
        <div className="grid gap-3 grid-cols-[repeat(auto-fill,minmax(220px,1fr))]">
          {shots.slice(0, show).map((s) => <ShotCard key={s.name} i={i} s={s} onOpen={() => open(s)} />)}
        </div>
      )}
      {shots && shots.length > show && <div className="flex justify-center mt-6"><Btn v="secondary" onClick={() => setShow(show + 24)}>Show more</Btn></div>}
      {view && (
        <div className="fixed inset-0 z-[90] bg-black/80 grid place-items-center fade" onMouseDown={() => setView(null)}>
          <div onMouseDown={(e) => e.stopPropagation()} className="max-w-[92vw] max-h-[92vh] flex flex-col gap-3">
            <div className="flex items-center gap-2 text-white">
              <span className="text-sm truncate flex-1">{view.s.name}</span>
              <Btn v="secondary" size="sm" disabled={!view.src} onClick={useOnHome}>Use on Home</Btn>
              {confirm
                ? <Btn v="danger" size="sm" onClick={del}>Confirm delete</Btn>
                : <Btn v="secondary" size="sm" icon="trash" onClick={() => setConfirm(true)}>Delete</Btn>}
              <Btn v="secondary" size="sm" onClick={() => setView(null)}>Close</Btn>
            </div>
            {view.src
              ? <img src={view.src} alt={view.s.name} className="max-w-[92vw] max-h-[80vh] object-contain rounded-xl" />
              : <div className="text-white/70 text-sm py-24 text-center">Loading…</div>}
          </div>
        </div>
      )}
    </div>
  )
}

/** Snapshots of the whole instance, plus world backups, in one place. */
function BackupsTab({ i }: { i: Inst }) {
  const app = useApp()
  const [snaps, setSnaps] = useState<Snapshot[] | null>(null)
  const [label, setLabel] = useState('')
  const [busy, setBusy] = useState('')
  const [confirm, setConfirm] = useState('')
  const [bk, setBk] = useState(0)
  const load = () => backend.listSnapshots(i.id).then(setSnaps).catch((e) => { setSnaps([]); app.notify(String(e)) })
  useEffect(() => {
    setSnaps(null)
    if (isDesktop()) load(); else setSnaps([])
  }, [i.id])
  const create = async () => {
    setBusy('create')
    try {
      await backend.createSnapshot(i.id, label)
      setLabel('')
      app.notify('Snapshot saved.')
      load()
    } catch (e) { app.notify(String(e)) } finally { setBusy('') }
  }
  const restore = async (s: Snapshot) => {
    setBusy(s.name)
    try {
      await backend.restoreSnapshot(i.id, s.name)
      setConfirm('')
      app.notify('Snapshot restored. How things were just before is saved as a snapshot too.')
      app.reload()
      load()
    } catch (e) { app.notify(String(e)) } finally { setBusy('') }
  }
  const del = async (s: Snapshot) => {
    try { await backend.deleteSnapshot(i.id, s.name); setConfirm(''); load() } catch (e) { app.notify(String(e)) }
  }
  const allWorlds = async () => {
    setBusy('worlds')
    try {
      const n = await backend.backupAllWorlds(i.id)
      app.notify(n ? `Backed up ${n} ${n === 1 ? 'world' : 'worlds'}.` : 'There are no worlds to back up yet.')
      setBk((x) => x + 1)
    } catch (e) { app.notify(String(e)) } finally { setBusy('') }
  }
  const toggleAuto = async (v: boolean) => {
    try {
      await backend.setAutoBackup(i.id, v)
      app.setInsts((p: Inst[]) => p.map((x) => (x.id === i.id ? { ...x, autoBackup: v } : x)))
    } catch (e) { app.notify(String(e)) }
  }
  return (
    <div className="max-w-3xl space-y-10">
      <section>
        <div className="text-[1rem] font-semibold tracking-tight">Instance snapshots</div>
        <p className="text-[0.8125rem] text-mu mt-1 leading-relaxed">
          Saves your mods, configs, resource packs, shaders and options. If a change goes wrong, restoring puts them all back. Worlds aren't touched. A snapshot is also taken automatically before you update all your mods.
        </p>
        <div className="flex gap-2 mt-4">
          <Input placeholder="Name (optional), for example “before the big update”" value={label} onChange={(e: any) => setLabel(e.target.value)} className="flex-1" />
          <Btn v="primary" icon="plus" loading={busy === 'create'} onClick={create}>Save a snapshot</Btn>
        </div>
        {snaps === null && <div className="py-6 text-sm text-mu">Loading…</div>}
        {snaps && !snaps.length && <p className="text-xs text-mu mt-4">No snapshots yet.</p>}
        <div className="mt-3 space-y-0.5">
          {snaps?.map((s) => (
            <div key={s.name} className="flex items-center gap-4 min-h-14 px-3 py-2 rounded-2xl hover:bg-s2 transition">
              <div className="flex-1 min-w-0">
                <div className="text-[0.8438rem] font-medium truncate">{s.label}</div>
                <div className="text-xs text-mu">{new Date(s.time).toLocaleString()} · {s.mods} {s.mods === 1 ? 'mod' : 'mods'} · {s.minecraft} {s.loader}</div>
              </div>
              {(s.minecraft !== i.mc || s.loader !== i.loader) && <Tip label="Made for a different Minecraft version or loader than this instance now uses"><span><Pill tone="warn">Different setup</Pill></span></Tip>}
              {confirm === 'r' + s.name
                ? <Btn v="primary" size="sm" loading={busy === s.name} onClick={() => restore(s)}>Confirm restore</Btn>
                : <Btn v="secondary" size="sm" onClick={() => setConfirm('r' + s.name)}>Restore</Btn>}
              {confirm === 'd' + s.name
                ? <Btn v="danger" size="sm" onClick={() => del(s)}>Confirm delete</Btn>
                : <Tip label="Delete this snapshot"><Btn v="ghost" size="sm" icon="trash" aria-label="Delete snapshot" onClick={() => setConfirm('d' + s.name)} /></Tip>}
            </div>
          ))}
        </div>
      </section>

      <section>
        <div className="text-[1rem] font-semibold tracking-tight">World backups</div>
        <p className="text-[0.8125rem] text-mu mt-1 leading-relaxed">Copies of your worlds. You can also back up a single world from the Worlds tab.</p>
        <div className="flex items-center gap-3 mt-4 flex-wrap">
          <Toggle on={!!i.autoBackup} onChange={(v: boolean) => toggleAuto(v)} />
          <span className="text-[0.8125rem] flex-1 min-w-48">Back up worlds that changed every time this instance launches (keeps the 5 newest of each world)</span>
          <Btn v="secondary" size="sm" loading={busy === 'worlds'} onClick={allWorlds}>Back up all worlds now</Btn>
        </div>
        <BackupsPanel i={i} refreshKey={bk} onChanged={() => {}} />
      </section>
    </div>
  )
}

/** What the game printed the last time it was launched. */
function LogsTab({ i }: { i: Inst }) {
  const app = useApp()
  const [text, setText] = useState<string | null>(null)
  const load = () => backend.readLaunchLog(i.id).then(setText).catch((e) => { setText(''); app.notify(String(e)) })
  useEffect(() => {
    setText(null)
    if (isDesktop()) load(); else setText('')
  }, [i.id])
  return (
    <div>
      <div className="flex items-center gap-2 mb-4 flex-wrap">
        <p className="text-[0.8125rem] text-mu flex-1">Output of the last launch</p>
        <Btn v="secondary" size="sm" onClick={load}>Refresh</Btn>
        <Btn v="secondary" size="sm" disabled={!text} onClick={() => navigator.clipboard?.writeText(text || '').then(() => app.notify('Log copied'))}>Copy</Btn>
        <Btn v="secondary" size="sm" icon="folder" onClick={() => openFolder(app, i)}>Open folder</Btn>
      </div>
      {text === null && <div className="py-16 text-center text-sm text-mu">Loading…</div>}
      {text === '' && <div className="py-16 text-center text-sm text-mu">No log yet. It appears after you launch this instance.</div>}
      {text && <pre className="rounded-2xl bg-s2 p-4 text-[0.7188rem] font-mono leading-relaxed overflow-auto max-h-[28rem] whitespace-pre-wrap break-words">{text}</pre>}
    </div>
  )
}

/** Crash Doctor: explains the last crash in plain words and offers one-click fixes. */
function DoctorTab({ i }: { i: Inst }) {
  const app = useApp()
  const [rep, setRep] = useState<DoctorReport | null>(null)
  const [err, setErr] = useState('')
  const [busy, setBusy] = useState('')
  const [showLog, setShowLog] = useState(false)
  const run = () => {
    if (!isDesktop()) { setErr('The Doctor needs the desktop app.'); return }
    setErr('')
    backend.diagnose(i.id).then(setRep).catch((e) => setErr(`The Doctor couldn't run: ${String(e)}`))
  }
  useEffect(() => { setRep(null); setShowLog(false); run() }, [i.id])
  const fix = async (d: Diagnosis) => {
    const f = d.fix
    if (!f) return
    setBusy(d.title)
    try {
      if (f.kind === 'install') await backend.installModrinth(i.id, f.target, 'mods')
      else if (f.kind === 'disable') await backend.setModEnabled(i.id, f.target, false)
      else if (f.kind === 'memory') await app.update(i.id, { ram: parseInt(f.target, 10) })
      else if (f.kind === 'clear_args') await app.update(i.id, { jvmArgs: '' })
      app.notify('Done. Launch again to test it.')
      run()
    } catch (e) {
      app.notify(f.kind === 'install' ? `Couldn't install ${f.target} automatically. Try searching for it in Mods. (${e})` : String(e))
    } finally { setBusy('') }
  }
  return (
    <div className="max-w-3xl">
      <div className="flex items-center gap-2 mb-4">
        <p className="text-[0.8125rem] text-mu flex-1">
          {rep && rep.has_data ? `Based on ${rep.source}` : 'Explains why the last launch went wrong'}
        </p>
        <Btn v="secondary" size="sm" onClick={run}>Check again</Btn>
      </div>
      {err && <div className="rounded-2xl bg-s2 px-4 py-3 text-xs text-red-500 mb-4 break-words">{err}</div>}
      {!rep && !err && <div className="py-16 text-center text-sm text-mu">Reading the log…</div>}
      {rep && !rep.has_data && (
        <div className="rounded-2xl bg-s2 px-5 py-10 text-center text-sm text-mu">
          Nothing to look at yet. After a launch, the Doctor reads the game's log and any crash report and explains what went wrong.
        </div>
      )}
      {rep && rep.has_data && (
        <div className="space-y-3">
          {rep.diagnoses.map((d) => (
            <div key={d.level + d.title} className="rounded-2xl bg-s2 p-4">
              <div className="flex items-start gap-3">
                <Pill tone={d.level === 'error' ? 'bad' : d.level === 'warn' ? 'warn' : undefined}>{d.level === 'error' ? 'Cause' : d.level === 'warn' ? 'Possible' : 'Note'}</Pill>
                <div className="flex-1 min-w-0">
                  <div className="text-[0.875rem] font-medium leading-snug break-words">{d.title}</div>
                  {d.detail && <p className="text-[0.7812rem] text-mu mt-1 leading-relaxed">{d.detail}</p>}
                </div>
                {d.fix && <Btn v="primary" size="sm" loading={busy === d.title} onClick={() => fix(d)}>{d.fix.label}</Btn>}
              </div>
            </div>
          ))}
          {rep.tail && (
            <div>
              <Btn v="ghost" size="sm" onClick={() => setShowLog(!showLog)}>{showLog ? 'Hide' : 'Show'} the last lines of the log</Btn>
              {showLog && <pre className="mt-2 rounded-2xl bg-s2 p-4 text-[0.7188rem] font-mono leading-relaxed overflow-auto max-h-[22rem] whitespace-pre-wrap break-words">{rep.tail}</pre>}
            </div>
          )}
        </div>
      )}
    </div>
  )
}

const PRESETS: { label: string; ram: number; args: string }[] = [
  { label: 'Low-end PC', ram: 2, args: '-XX:+UseSerialGC' },
  { label: 'Balanced', ram: 4, args: '-XX:+UseG1GC -XX:+ParallelRefProcEnabled -XX:MaxGCPauseMillis=200 -XX:+DisableExplicitGC' },
  { label: 'Many mods', ram: 8, args: '-XX:+UseG1GC -XX:+ParallelRefProcEnabled -XX:MaxGCPauseMillis=130 -XX:+DisableExplicitGC' },
  { label: 'Reset to default', ram: 4, args: '' },
]

export function InstSettings({ i }: { i: Inst }) {
  const app = useApp()
  const [ram, setRam] = useState(i.ram)
  const [args, setArgs] = useState(i.jvmArgs ?? '')
  const [w, setW] = useState(i.width ? String(i.width) : '')
  const [h, setH] = useState(i.height ? String(i.height) : '')
  const [server, setServer] = useState(i.server ?? '')
  const groups = GROUPS.includes(i.group) ? GROUPS : [...GROUPS, i.group]
  const num = (v: string) => { const n = parseInt(v, 10); return Number.isFinite(n) && n > 0 ? n : 0 }
  const dirty = ram !== i.ram || args.trim() !== (i.jvmArgs ?? '') || num(w) !== (i.width ?? 0) || num(h) !== (i.height ?? 0) || server.trim() !== (i.server ?? '')
  return (
    <div className="max-w-2xl">
      <Panel className="px-6 divide-y divide-line">
        <Row title="Group" desc="Where this instance appears on the Instances page">
          <Select value={i.group} onChange={(g) => app.update(i.id, { group: g })} options={groups} />
        </Row>
        <Row title="Memory" desc={`${ram} GB`}>
          <input type="range" min={1} max={16} value={ram} onChange={(e) => setRam(+e.target.value)} className="w-44" />
        </Row>
        <Row title="Performance preset" desc="Fills in memory and Java arguments. Click Save changes to apply">
          <PillMenu label="Choose a preset" icon="cpu" items={PRESETS.map((p) => ({ label: p.label, onClick: () => { setRam(p.ram); setArgs(p.args) } }))} />
        </Row>
        <Row title="Window size" desc="In pixels. Leave empty for the game's default">
          <div className="flex items-center gap-2">
            <Input inputMode="numeric" placeholder="Width" value={w} onChange={(e: any) => setW(e.target.value)} className="w-24" />
            <span className="text-mu">×</span>
            <Input inputMode="numeric" placeholder="Height" value={h} onChange={(e: any) => setH(e.target.value)} className="w-24" />
          </div>
        </Row>
        <Row title="Java arguments" desc="Advanced. Added to the end of the launch command">
          <Input placeholder="-XX:+UseZGC" value={args} onChange={(e: any) => setArgs(e.target.value)} className="w-64" />
        </Row>
        <Row title="Join server on launch" desc="Optional. For example play.example.com or play.example.com:25566">
          <Input placeholder="server address" value={server} onChange={(e: any) => setServer(e.target.value)} className="w-64" />
        </Row>
      </Panel>
      <div className="mt-4">
        <Btn v="primary" disabled={!dirty} onClick={() => app.update(i.id, { ram, jvmArgs: args.trim(), width: num(w), height: num(h), server: server.trim() })}>Save changes</Btn>
      </div>
    </div>
  )
}
