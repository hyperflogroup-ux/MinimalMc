import { useEffect, useRef, useState } from 'react'
import { Blk, Btn, Icon, Input, Panel, PageHead, Pill, PillMenu, Row, Seg, Tip, Toggle, useApp } from './ui'
import { backend, isDesktop, releaseVersions, type CfKeyStatus, type DedupeResult, type JavaEntry, type MrHit } from './backend'
import { LOADERS, type Inst } from './data'
import { ProjectView } from './project'

const TITLES: Record<string, string> = { mods: 'Mods', modpacks: 'Modpacks', shaders: 'Shaders', packs: 'Resource Packs' }
const SORTS: [string, string][] = [['relevance', 'Relevance'], ['downloads', 'Most downloaded'], ['newest', 'Newest'], ['updated', 'Recently updated']]
const fmt = (n: number) => (n >= 1e6 ? `${(n / 1e6).toFixed(1)}M` : n >= 1e3 ? `${Math.round(n / 1e3)}K` : String(n))

// Which site Browse used last, so switching between Mods, Shaders and so on keeps it.
let lastSource: 'modrinth' | 'curseforge' = 'modrinth'

export function Discover({ type }: { type: string }) {
  const app = useApp()
  const desktop = isDesktop()
  const insts: Inst[] = app.insts
  const [q, setQ] = useState('')
  const [dq, setDq] = useState('')
  const [tid, setTid] = useState<number | null>(null)
  const [compat, setCompat] = useState(true)
  const [ver, setVer] = useState('')
  const [loaderSel, setLoaderSel] = useState('')
  const [sort, setSort] = useState('relevance')
  const [hits, setHits] = useState<MrHit[]>([])
  const [total, setTotal] = useState(0)
  const [loading, setLoading] = useState(false)
  const [err, setErr] = useState('')
  const [note, setNote] = useState('')
  const [st, setSt] = useState<Record<string, string>>({})
  const [mcs, setMcs] = useState<string[]>([])
  const req = useRef(0)
  const [detail, setDetail] = useState<MrHit | null>(null)
  const [vbusy, setVbusy] = useState('')
  const [source, setSource] = useState<'modrinth' | 'curseforge'>(lastSource)
  const [cfKey, setCfKey] = useState(false)

  const isPack = type === 'modpacks'
  const inst = insts.find((i) => i.id === (tid ?? app.sel)) ?? insts[0]
  const effVer = isPack ? ver : compat && inst ? inst.mc : ver
  const effLoader = isPack ? loaderSel : type === 'mods' ? (compat && inst ? (inst.loader === 'Vanilla' ? '' : inst.loader) : loaderSel) : ''

  useEffect(() => { const t = setTimeout(() => setDq(q.trim()), 350); return () => clearTimeout(t) }, [q])
  useEffect(() => { if (desktop) releaseVersions().then(setMcs).catch(() => {}) }, [])
  useEffect(() => { if (desktop) backend.curseforgeStatus().then((s) => setCfKey(s.saved)).catch(() => {}) }, [])
  const pickSource = (s: 'modrinth' | 'curseforge') => { lastSource = s; setSource(s); setHits([]); setTotal(0) }

  const run = (offset: number) => {
    if (!desktop) return
    const id = ++req.current
    setLoading(true); setErr('')
    ;(source === 'curseforge' ? backend.searchCurseforge : backend.searchModrinth)(dq, type, effLoader, effVer, sort, offset)
      .then((r) => { if (id !== req.current) return; setHits((p) => (offset ? [...p, ...r.hits] : r.hits)); setTotal(r.total) })
      .catch((e) => { if (id !== req.current) return; setErr(String(e)); if (!offset) { setHits([]); setTotal(0) } })
      .finally(() => { if (id === req.current) setLoading(false) })
  }
  useEffect(() => { run(0) }, [type, dq, effVer, effLoader, sort, source])

  const key = (h: MrHit) => (isPack ? `pack:${h.project_id}` : `${inst?.id}:${h.project_id}`)
  const install = async (h: MrHit, versionId?: string) => {
    if (!inst) return
    setNote(''); setSt((s) => ({ ...s, [key(h)]: 'busy' }))
    try {
      const r = await (source === 'curseforge' ? backend.installCurseforge : backend.installModrinth)(inst.id, h.project_id, type, versionId)
      setSt((s) => ({ ...s, [key(h)]: 'done' }))
      const msg = r.installed.length ? `Installed ${r.installed.join(', ')} in ${inst.name}.` : `Already in ${inst.name}.`
      setNote([msg, ...r.warnings].join(' '))
      if (type === 'mods' && r.installed.length) app.setInsts((p: Inst[]) => p.map((x) => (x.id === inst.id ? { ...x, mods: x.mods + r.installed.length } : x)))
      // Run the compatibility check right away, so problems show up while you're still setting things up.
      if (type === 'mods' && inst.loader !== 'Vanilla') {
        backend.checkCompat(inst.id).then((c) => {
          const n = c.findings.filter((f) => f.level === 'error').length
          if (n) setNote((p) => `${p} Compatibility check: ${n} ${n === 1 ? 'problem' : 'problems'} found. Open the instance's Mods tab to fix ${n === 1 ? 'it' : 'them'}.`)
        }).catch(() => {})
      }
    } catch (e) {
      setSt((s) => { const n = { ...s }; delete n[key(h)]; return n })
      setNote(String(e))
    }
  }

  // A modpack becomes a brand-new instance; its files download in the background.
  const installPack = async (h: MrHit, versionId?: string) => {
    setNote(''); setSt((s) => ({ ...s, [key(h)]: 'busy' }))
    try {
      const b = await (source === 'curseforge' ? backend.installCurseforgePack : backend.installModpack)(h.project_id, versionId)
      app.setInsts((p: Inst[]) => [...p, { id: b.id, name: b.name, group: b.group || 'Modded', mc: b.version, loader: b.loader, mods: b.mods, ram: b.ram, last: b.last }])
      app.openInstance(b.id)
    } catch (e) {
      setSt((s) => { const n = { ...s }; delete n[key(h)]; return n })
      setNote(String(e))
    }
  }

  const startInstall = (h: MrHit, versionId?: string) => {
    setVbusy(versionId ?? 'latest')
    const job = isPack ? installPack(h, versionId) : install(h, versionId)
    Promise.resolve(job).finally(() => setVbusy(''))
  }
  const verOptions = ['Any version', ...Array.from(new Set([...insts.map((i) => i.mc), ...mcs.slice(0, 20)]))]
  if (detail) {
    return (
      <ProjectView
        hit={detail}
        type={type}
        inst={inst}
        insts={insts}
        onPick={(id) => setTid(id)}
        onBack={() => setDetail(null)}
        onInstall={() => startInstall(detail)}
        onInstallVersion={(vid) => startInstall(detail, vid)}
        source={source}
        busy={vbusy}
        note={note}
      />
    )
  }
  return (
    <div className="max-w-5xl mx-auto">
      <PageHead title="Browse" sub={`Mods, modpacks, shaders and resource packs from ${source === 'curseforge' ? 'CurseForge' : 'Modrinth'}.`}>
        <div className="inline-flex p-1 rounded-full bg-s2 gap-0.5">
          <button onClick={() => pickSource('modrinth')} className={`h-10 px-5 rounded-full text-[0.8125rem] font-semibold transition ${source === 'modrinth' ? 'bg-s3 text-fg shadow-sm' : 'text-mu hover:text-fg'}`}>Modrinth</button>
          {cfKey ? (
            <button onClick={() => pickSource('curseforge')} className={`h-10 px-5 rounded-full text-[0.8125rem] font-semibold transition ${source === 'curseforge' ? 'bg-s3 text-fg shadow-sm' : 'text-mu hover:text-fg'}`}>CurseForge</button>
          ) : (
            <Tip label="Add your CurseForge API key in Settings, under CurseForge">
              <span aria-disabled="true" className="h-10 px-5 rounded-full text-[0.8125rem] font-semibold inline-flex items-center gap-2 text-mu opacity-40 cursor-not-allowed select-none">CurseForge<span className="text-[0.6875rem] font-medium">Needs a key</span></span>
            </Tip>
          )}
        </div>
      </PageHead>
      <div className="flex gap-1 mb-5">
        {Object.entries(TITLES).map(([k, t]) => (
          <button key={k} onClick={() => app.go(k)} className={`h-10 px-5 rounded-full text-[0.8125rem] font-medium transition ${type === k ? 'bg-s3 text-fg' : 'text-mu hover:text-fg hover:bg-fg/[.05]'}`}>{t}</button>
        ))}
      </div>
      <Input big icon="search" placeholder={`Search ${TITLES[type].toLowerCase()}`} value={q} onChange={(e: any) => setQ(e.target.value)} className="mb-3" />
      <div className="flex gap-2 flex-wrap mb-5">
        {isPack ? null : insts.length ? (
          <PillMenu label={`Install to: ${inst?.name ?? 'none'}`} icon="box" items={insts.map((i) => ({ label: i.name, icon: inst?.id === i.id ? 'check' : undefined, onClick: () => setTid(i.id) }))} />
        ) : (
          <Btn v="secondary" icon="plus" onClick={() => app.setDialog({ kind: 'create' })}>Create an instance to install into</Btn>
        )}
        <PillMenu label={`Version: ${effVer || 'Any'}`} items={verOptions.map((v) => ({ label: v, icon: (effVer || 'Any version') === v ? 'check' : undefined, onClick: () => { setVer(v === 'Any version' ? '' : v); setCompat(false) } }))} />
        {(type === 'mods' || isPack) && <PillMenu label={`Loader: ${effLoader || 'Any'}`} items={['Any', ...LOADERS.filter((l) => l !== 'Vanilla')].map((l) => ({ label: l, icon: (effLoader || 'Any') === l ? 'check' : undefined, onClick: () => { setLoaderSel(l === 'Any' ? '' : l); setCompat(false) } }))} />}
        <PillMenu label={`Sort: ${SORTS.find((s) => s[0] === sort)?.[1]}`} items={SORTS.map(([id, label]) => ({ label, icon: sort === id ? 'check' : undefined, onClick: () => setSort(id) }))} />
        {inst && !isPack && <Pill active={compat} onClick={() => setCompat(!compat)} icon="check">Compatible</Pill>}
      </div>
      {note && <div className="rounded-2xl bg-s2 px-4 py-3 text-xs text-mu mb-5 leading-relaxed">{note}</div>}
      {!desktop && <div className="rounded-2xl bg-s2 px-5 py-8 text-center text-sm text-mu">Browsing needs the desktop app. Run <span className="font-mono text-fg">npm run tauri dev</span>.</div>}
      {desktop && err && <div className="rounded-2xl bg-s2 px-5 py-8 text-center text-sm text-mu">{err}</div>}
      {desktop && !err && !loading && !hits.length && <div className="py-16 text-center text-sm text-mu">Nothing found. Try another search or loosen the filters.</div>}
      <div className="grid md:grid-cols-2 gap-3">
        {hits.map((h) => {
          const s = st[key(h)]
          return (
            <div key={h.project_id} onClick={() => setDetail(h)} className="group flex gap-4 p-4 rounded-[16px] bg-s1 hover:bg-s2 transition cursor-pointer" style={{ boxShadow: '0 0 0 1px var(--line)' }}>
              {h.icon_url ? <img src={h.icon_url} alt="" loading="lazy" className="size-[52px] shrink-0 rounded-[14px] object-cover bg-s2" /> : <Blk seed={h.title.length} size={52} r={14} />}
              <div className="flex-1 min-w-0">
                <div className="flex items-baseline gap-2"><span className="text-[0.875rem] font-medium truncate">{h.title}</span><span className="text-xs text-mu truncate">by {h.author}</span></div>
                <p className="text-[0.7812rem] text-mu mt-1 line-clamp-2 leading-relaxed">{h.description}</p>
                <div className="flex items-center gap-1.5 mt-3">
                  <Pill icon="download">{fmt(h.downloads)}</Pill>
                  {h.latest_version && <Pill>{h.latest_version}</Pill>}
                  {h.categories[0] && <Pill className="capitalize">{h.categories[0]}</Pill>}
                  <div className="flex-1" />
                  {s === 'done' ? <Pill tone="ok" icon="check">Installed</Pill>
                    : isPack ? <Tip label="Creates a new instance"><Btn v="primary" size="sm" loading={s === 'busy'} onClick={(e: any) => { e.stopPropagation(); installPack(h) }}>Install</Btn></Tip>
                    : <Btn v="primary" size="sm" loading={s === 'busy'} disabled={!inst} onClick={(e: any) => { e.stopPropagation(); install(h) }}>Install</Btn>}
                </div>
              </div>
            </div>
          )
        })}
      </div>
      {loading && <div className="py-8 text-center text-sm text-mu">Searching {source === 'curseforge' ? 'CurseForge' : 'Modrinth'}…</div>}
      {desktop && !loading && hits.length > 0 && hits.length < total && <div className="flex justify-center mt-6"><Btn v="secondary" onClick={() => run(hits.length)}>Load more</Btn></div>}
    </div>
  )
}

const ACCENTS = ['white', 'red', 'orange', 'yellow', 'green', 'cyan', 'blue', 'indigo', 'violet', 'pink']
const CATS = ['Appearance', 'Java', 'Accounts', 'CurseForge', 'Storage', 'About']

function Appearance() {
  const app = useApp()
  const p = app.prefs
  return (
    <div className="divide-y divide-line">
      <Row title="Theme" desc="Follows your system by default"><Seg value={p.theme} onChange={(v) => app.setPref('theme', v)} items={[{ id: 'light', label: 'Light' }, { id: 'dark', label: 'Dark' }, { id: 'system', label: 'System' }]} /></Row>
      <div className="py-3.5">
        <div className="text-[0.8438rem] font-medium">Accent</div>
        <div className="flex gap-2.5 mt-3 flex-wrap">
          {ACCENTS.map((a) => (
            <Tip key={a} label={a[0].toUpperCase() + a.slice(1)}>
              <button onClick={() => app.setPref('accent', a)} aria-label={a} data-accent={a} className={`size-8 rounded-full bg-ac grid place-items-center transition hover:scale-110 ${p.accent === a ? 'ring-2 ring-offset-2 ring-offset-s1 ring-fg' : ''}`}>{p.accent === a && <Icon n="check" size={14} className="text-acfg" />}</button>
            </Tip>
          ))}
        </div>
      </div>
      <Row title="Transparency" desc="Translucent navigation and title bar"><Toggle on={p.glass} onChange={(v) => app.setPref('glass', v)} /></Row>
      <Row title="Animations" desc="Micro-interactions and transitions"><Toggle on={p.anim} onChange={(v) => app.setPref('anim', v)} /></Row>
      <Row title="Compact mode" desc="Tighter spacing and smaller type"><Toggle on={p.compact} onChange={(v) => app.setPref('compact', v)} /></Row>
    </div>
  )
}

function JavaSettings() {
  const [list, setList] = useState<JavaEntry[] | null>(null)
  useEffect(() => {
    if (!isDesktop()) { setList([]); return }
    backend.listJava().then(setList).catch(() => setList([]))
  }, [])
  return (
    <div>
      <p className="text-[0.8125rem] text-mu py-3 leading-relaxed">MinimalMc uses the Java version each Minecraft version needs. If it isn’t installed, it’s downloaded automatically the first time you launch (roughly 100 to 200 MB) and reused after that.</p>
      {list === null && <div className="py-6 text-sm text-mu">Looking for Java…</div>}
      {list && !list.length && <div className="py-6 text-sm text-mu">No Java installed yet. One will be downloaded when you first launch an instance.</div>}
      <div className="space-y-2">
        {list?.map((j) => (
          <div key={j.path} className="rounded-2xl bg-s2 p-4">
            <div className="flex items-center gap-2"><span className="text-[0.8438rem] font-medium">Java {j.version}</span>{j.managed && <Pill tone="ac">Managed by MinimalMc</Pill>}</div>
            <div className="text-[0.7188rem] text-mu font-mono mt-1.5 truncate">{j.path}</div>
          </div>
        ))}
      </div>
    </div>
  )
}

function CurseForgeSettings() {
  const [status, setStatus] = useState<CfKeyStatus | null>(null)
  const [key, setKey] = useState('')
  const [msg, setMsg] = useState('')
  const [bad, setBad] = useState(false)
  const [busy, setBusy] = useState('')
  const refresh = () => backend.curseforgeStatus().then(setStatus).catch(() => {})
  useEffect(() => { if (isDesktop()) refresh() }, [])
  const save = async () => {
    setBusy('save'); setMsg('')
    try {
      await backend.curseforgeSetKey(key)
      setKey('')
      setBad(false)
      setMsg('Key saved. Click Test to check that it works.')
      refresh()
    } catch (e) { setBad(true); setMsg(String(e)) } finally { setBusy('') }
  }
  const test = async () => {
    setBusy('test'); setMsg('')
    try { setMsg(await backend.curseforgeTest()); setBad(false) } catch (e) { setBad(true); setMsg(String(e)) } finally { setBusy('') }
  }
  const remove = async () => {
    try { await backend.curseforgeSetKey(''); setBad(false); setMsg('Key removed.'); refresh() } catch (e) { setBad(true); setMsg(String(e)) }
  }
  return (
    <div className="py-3 space-y-4">
      <p className="text-[0.8125rem] text-mu leading-relaxed">
        CurseForge needs a personal API key, which you get from CurseForge. It is saved only on this computer and is only ever sent to CurseForge. Once it's set, a CurseForge button appears in Browse.
      </p>
      <div className="rounded-2xl bg-s2 p-4">
        <div className="flex items-center gap-2">
          <div className="text-[0.875rem] font-medium flex-1">API key</div>
          {status?.saved && <Pill tone="ok" dot>Saved (ends in {status.hint})</Pill>}
        </div>
        <div className="flex gap-2 mt-3">
          <Input type="password" placeholder={status?.saved ? 'Paste a new key to replace it' : 'Paste your API key'} value={key} onChange={(e: any) => setKey(e.target.value)} className="flex-1" />
          <Btn v="primary" loading={busy === 'save'} disabled={!key.trim()} onClick={save}>Save</Btn>
        </div>
        <div className="flex gap-2 mt-3 flex-wrap">
          <Btn v="secondary" size="sm" loading={busy === 'test'} disabled={!status?.saved} onClick={test}>Test key</Btn>
          <Btn v="ghost" size="sm" disabled={!status?.saved} onClick={remove}>Remove key</Btn>
          <Btn v="ghost" size="sm" onClick={() => backend.openUrl('https://console.curseforge.com/').catch(() => {})}>Open CurseForge's developer site</Btn>
        </div>
        {msg && <div className={`mt-3 text-xs break-words ${bad ? 'text-red-500' : 'text-mu'}`}>{msg}</div>}
      </div>
      <p className="text-xs text-mu leading-relaxed">Some authors don't allow their mods to be downloaded through launchers. For those, MinimalMc tells you and points you to the CurseForge page, and never downloads them.</p>
    </div>
  )
}

function Storage() {
  const [busy, setBusy] = useState(false)
  const [res, setRes] = useState<DedupeResult | null>(null)
  const [err, setErr] = useState('')
  const run = () => {
    if (!isDesktop()) { setErr('This needs the desktop app.'); return }
    setBusy(true)
    setErr('')
    backend.dedupeStorage().then(setRes).catch((e) => setErr(String(e))).finally(() => setBusy(false))
  }
  const mb = res ? (res.saved / 1048576).toFixed(res.saved > 104857600 ? 0 : 1) : ''
  return (
    <div className="py-3">
      <p className="text-[0.8125rem] text-mu leading-relaxed">
        When several instances contain the exact same mod, resource pack or shader, the copies are merged into one file that they all share. Each instance still sees its own normal file, and removing it from one instance never affects the others. This also happens by itself after you install mods.
      </p>
      <div className="mt-4"><Btn v="secondary" loading={busy} onClick={run}>Save disk space now</Btn></div>
      {err && <div className="mt-3 text-xs text-red-500 break-words">{err}</div>}
      {res && (
        <div className="mt-3 text-[0.8125rem]">
          {res.merged === 0 ? 'Nothing to merge right now. Everything is already stored once.' : `Merged ${res.merged} duplicate ${res.merged === 1 ? 'file' : 'files'}, saving about ${mb} MB.`}
        </div>
      )}
    </div>
  )
}

function Accounts() {
  const app = useApp()
  const [name, setName] = useState<string>(app.account)
  const valid = /^[A-Za-z0-9_]{3,16}$/.test(name)
  return (
    <div className="space-y-3">
      <div className="rounded-2xl bg-s2 p-4">
        <div className="flex items-center gap-2">
          <div className="text-[0.875rem] font-medium flex-1">Offline account</div>
          {app.account && <Pill tone="ok" dot>Active: {app.account}</Pill>}
        </div>
        <div className="text-xs text-mu mt-1 leading-relaxed">Play without signing in. Offline accounts can't join servers that check Minecraft accounts.</div>
        <div className="flex gap-2 mt-4">
          <Input placeholder="Player name" value={name} onChange={(e: any) => setName(e.target.value)} className="w-64" />
          <Btn v="primary" disabled={!valid || name === app.account} onClick={() => app.setAccount(name)}>Save</Btn>
        </div>
        {name && !valid && <div className="text-xs text-red-500 mt-2">3 to 16 letters, numbers or underscores.</div>}
      </div>
      <div className="flex items-center gap-4 rounded-2xl bg-s2/60 p-4 opacity-70">
        <div className="size-10 rounded-full bg-fg/10 grid place-items-center"><Icon n="user" size={18} /></div>
        <div className="flex-1"><div className="text-[0.875rem] font-medium">Microsoft account</div><div className="text-xs text-mu">Sign-in isn’t available yet</div></div>
        <Pill>Coming soon</Pill>
      </div>
    </div>
  )
}

export function Settings() {
  const app = useApp()
  const [cat, setCat] = useState(app.account ? 'Appearance' : 'Accounts')
  const body = () => {
    switch (cat) {
      case 'Appearance': return <Appearance />
      case 'Java': return <JavaSettings />
      case 'Accounts': return <Accounts />
      case 'CurseForge': return <CurseForgeSettings />
      case 'Storage': return <Storage />
      default: return <div className="py-6 text-[0.8125rem] text-mu space-y-1"><div className="text-[1.0625rem] font-semibold text-fg">MinimalMc</div><div>Version 1.0.0</div></div>
    }
  }
  return (
    <div className="max-w-4xl mx-auto">
      <PageHead title="Settings" />
      <div className="grid md:grid-cols-[180px_1fr] gap-8">
        <div className="flex md:flex-col gap-0.5 overflow-x-auto scroll">
          {CATS.map((c) => <button key={c} onClick={() => setCat(c)} className={`h-10 px-5 rounded-full text-[0.8125rem] text-left whitespace-nowrap font-medium transition ${cat === c ? 'bg-s3 text-fg' : 'text-mu hover:text-fg hover:bg-fg/[.05]'}`}>{c}</button>)}
        </div>
        <Panel className="px-6 py-3 min-h-80"><div key={cat} className="fade">{body()}</div></Panel>
      </div>
    </div>
  )
}
