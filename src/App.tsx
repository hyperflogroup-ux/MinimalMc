import { Component, useEffect, useState, type ReactNode } from 'react'
import { Ctx, Icon, Tip } from './ui'
import type { Inst } from './data'
import { Home, InstanceDetail, Instances } from './screens1'
import { Discover, Settings } from './screens2'
import { backend, isDesktop, type BackendInstance } from './backend'
import Skins from './skins'
import { CreateDialog, ImportDialog, Palette, SimpleDialog } from './overlays'
import { BisectDialog } from './bisect'
import { removeAllMedia } from './mediaStore'

const NAV = [
  ['home', 'Home', 'home'],
  ['instances', 'Instances', 'box'],
  ['mods', 'Browse', 'search'],
  ['skins', 'Skins', 'user'],
]

/** If a page crashes, show the reason instead of a blank black window. */
class Boundary extends Component<{ children: ReactNode; resetKey: string }, { error: Error | null }> {
  state = { error: null as Error | null }
  static getDerivedStateFromError(error: Error) { return { error } }
  componentDidUpdate(prev: { resetKey: string }) {
    if (prev.resetKey !== this.props.resetKey && this.state.error) this.setState({ error: null })
  }
  render() {
    if (this.state.error) {
      return (
        <div className="max-w-xl mx-auto py-24 text-center">
          <h1 className="text-[1.5rem] font-semibold tracking-tight">Something went wrong on this page</h1>
          <p className="text-[0.8125rem] text-mu mt-3 font-mono break-words">{this.state.error.message}</p>
        </div>
      )
    }
    return this.props.children
  }
}

// Mods, modpacks, shaders and resource packs are all tabs of the one Browse page.
const BROWSE = ['mods', 'modpacks', 'shaders', 'packs']

type Entry = { route: string; sel: number | null; tab: string }

/** Small arrow and reload icons for the history buttons. */
const NavIcon = ({ d }: { d: string }) => (
  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d={d} /></svg>
)

const PREFS_KEY = 'minimalmc.prefs'
const ACCOUNT_KEY = 'minimalmc.account'
const loadAccount = () => {
  try { return localStorage.getItem(ACCOUNT_KEY) || '' } catch { return '' }
}
const DEFAULT_PREFS = { theme: 'dark', accent: 'white', glass: true, anim: true, compact: false }
const loadPrefs = () => {
  try { return { ...DEFAULT_PREFS, ...JSON.parse(localStorage.getItem(PREFS_KEY) || '{}') } } catch { return DEFAULT_PREFS }
}
const toInst = (b: BackendInstance): Inst => ({ id: b.id, name: b.name, group: b.group || 'Other', mc: b.version, loader: b.loader, mods: b.mods, ram: b.ram, last: b.last, jvmArgs: b.jvm_args ?? '', width: b.width ?? 0, height: b.height ?? 0, played: b.played ?? 0, server: b.server ?? '', autoBackup: b.auto_backup ?? false })
const errText = (e: unknown) => (e instanceof Error ? e.message : String(e))

export default function App() {
  const [route, setRoute] = useState('home')
  const [insts, setInsts] = useState<Inst[]>([])
  const [ready, setReady] = useState(!isDesktop())
  const [sel, setSel] = useState<number | null>(null)
  const [tab, setTab] = useState('Overview')
  const [dialog, setDialog] = useState<any>(null)
  const [open, setOpen] = useState(false)
  const [palette, setPalette] = useState(false)
  const [toast, setToast] = useState('')
  const [prefs, setPrefs] = useState(loadPrefs)
  const [account, setAccountState] = useState(loadAccount)
  const [hist, setHist] = useState<Entry[]>([{ route: 'home', sel: null, tab: 'Overview' }])
  const [pos, setPos] = useState(0)
  const [nonce, setNonce] = useState(0)
  const [pulse, setPulse] = useState(0)
  const [lastBrowse, setLastBrowse] = useState('mods')

  // Moving between pages keeps a history, so Back and Forward work like in a browser.
  const apply = (e: Entry) => { setRoute(e.route); setSel(e.sel); setTab(e.tab) }
  const navigate = (r: string, o?: { sel?: number | null; tab?: string }) => {
    const e: Entry = { route: r, sel: o && 'sel' in o ? (o.sel ?? null) : sel, tab: o?.tab ?? tab }
    apply(e)
    const cur = hist[pos]
    if (cur && cur.route === e.route && cur.sel === e.sel && cur.tab === e.tab) return
    setHist([...hist.slice(0, pos + 1), e])
    setPos(pos + 1)
  }
  const back = () => { if (pos > 0) { setPos(pos - 1); apply(hist[pos - 1]) } }
  const forward = () => { if (pos < hist.length - 1) { setPos(pos + 1); apply(hist[pos + 1]) } }
  const refresh = () => {
    if (isDesktop()) backend.listInstances().then((l) => setInsts(l.map(toInst))).catch((e) => setToast(errText(e)))
    setNonce((n) => n + 1)
  }

  useEffect(() => {
    const el = document.documentElement
    const mq = window.matchMedia('(prefers-color-scheme: dark)')
    const apply = () => el.setAttribute('data-theme', prefs.theme === 'system' ? (mq.matches ? 'dark' : 'light') : prefs.theme)
    apply()
    mq.addEventListener('change', apply)
    el.setAttribute('data-accent', prefs.accent)
    el.setAttribute('data-glass', String(prefs.glass))
    el.setAttribute('data-anim', String(prefs.anim))
    el.setAttribute('data-compact', String(prefs.compact))
    try { localStorage.setItem(PREFS_KEY, JSON.stringify(prefs)) } catch { /* storage unavailable */ }
    return () => mq.removeEventListener('change', apply)
  }, [prefs])

  useEffect(() => {
    if (!isDesktop()) return
    backend.listInstances()
      .then((l) => setInsts(l.map(toInst)))
      .catch((e) => setToast(`Couldn't load instances: ${errText(e)}`))
      .finally(() => setReady(true))
  }, [])

  useEffect(() => {
    if (BROWSE.includes(route)) setLastBrowse(route)
  }, [route])

  useEffect(() => {
    if (!toast) return
    const t = setTimeout(() => setToast(''), 5000)
    return () => clearTimeout(t)
  }, [toast])

  useEffect(() => {
    const k = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') { e.preventDefault(); setPalette((p) => !p) }
    }
    window.addEventListener('keydown', k)
    return () => window.removeEventListener('keydown', k)
  }, [])

  const app = {
    insts, setInsts, sel, setSel, tab, setTab, prefs, ready, account,
    setAccount: (n: string) => { setAccountState(n); try { localStorage.setItem(ACCOUNT_KEY, n) } catch { /* storage unavailable */ } },
    reload: () => { backend.listInstances().then((l) => setInsts(l.map(toInst))).catch(() => {}) },
    setPref: (k: string, v: any) => setPrefs((p: typeof DEFAULT_PREFS) => ({ ...p, [k]: v })),
    go: (r: string) => navigate(r),
    openInstance: (id: number, t = 'Overview') => navigate('instance', { sel: id, tab: t }),
    pulse,
    kick: () => setPulse((n) => n + 1),
    setDialog,
    notify: (m: string) => setToast(m),
    duplicate: async (i: Inst) => {
      try { const b = await backend.duplicateInstance(i.id); setInsts((p) => [...p, toInst(b)]) } catch (e) { setToast(errText(e)) }
    },
    // Throws on failure so the Create dialog can show the reason.
    addInstance: async (i: { name: string; mc: string; loader: string; group: string; ram: number }) => {
      const b = await backend.createInstance(i.name, i.mc, i.loader, i.group, i.ram)
      setInsts((p) => [...p, toInst(b)]); navigate('instances', { sel: b.id })
    },
    removeInstance: async (id: number) => {
      try { await backend.deleteInstance(id) } catch (e) { setToast(errText(e)); return }
      removeAllMedia(id).catch(() => {})
      setInsts((p) => p.filter((x) => x.id !== id))
      if (route === 'instance') navigate('instances', { sel: null })
      else setSel(null)
    },
    rename: async (id: number, name: string) => {
      try { await backend.renameInstance(id, name); setInsts((p) => p.map((x) => (x.id === id ? { ...x, name: name.trim() } : x))) } catch (e) { setToast(errText(e)) }
    },
    update: async (id: number, patch: { group?: string; ram?: number; jvmArgs?: string; width?: number; height?: number; server?: string }) => {
      try { await backend.updateInstance(id, patch); setInsts((p) => p.map((x) => (x.id === id ? { ...x, ...patch } : x))) } catch (e) { setToast(errText(e)) }
    },
  }

  const active = BROWSE.includes(route) ? 'mods' : route === 'instance' ? 'instances' : route
  const page = () => {
    switch (route) {
      case 'home': return <Home />
      case 'instances': return <Instances />
      case 'instance': return <InstanceDetail />
      case 'mods': case 'modpacks': case 'shaders': case 'packs': return <Discover type={route} />
      case 'skins': return <Skins />
      case 'settings': return <Settings />
    }
  }

  return (
    <Ctx.Provider value={app}>
      <div className="h-screen flex flex-col bg-bg text-fg text-[0.8125rem] relative overflow-hidden">
        <div className="absolute -top-40 right-0 w-[700px] h-[420px] pointer-events-none opacity-60" style={{ background: 'radial-gradient(closest-side, color-mix(in oklab, var(--ac) 14%, transparent), transparent)' }} />
        <header className="glass relative z-30 h-12 shrink-0 flex items-center px-5 gap-3 border-b border-line">
          <div className="flex items-center gap-2 w-40"><div className="size-4 rounded-[5px] bg-ac" /><span className="text-[0.8125rem] font-semibold tracking-tight">MinimalMc</span></div>
          <button onClick={() => setPalette(true)} className="mx-auto h-9 w-[420px] max-w-[40vw] rounded-full bg-fg/[.06] hover:bg-fg/10 transition flex items-center gap-2 px-3 text-mu text-xs">
            <Icon n="search" size={13} />Search<span className="ml-auto font-mono text-[0.6562rem] text-mu2">Ctrl K</span>
          </button>
          <div className="w-40" />
        </header>
        <div className="flex-1 min-h-0 flex relative">
          <nav className={`glass z-20 shrink-0 flex flex-col py-4 px-3 gap-1 border-r border-line transition-[width] duration-200 ${open ? 'w-60' : 'w-[76px]'}`}>
            <div className={`flex mb-2 ${open ? 'gap-1 px-1' : 'flex-col items-center gap-1'}`}>
              {[
                { label: 'Back', d: 'M15 18l-6-6 6-6', on: back, off: pos === 0 },
                { label: 'Forward', d: 'M9 18l6-6-6-6', on: forward, off: pos >= hist.length - 1 },
                { label: 'Reload', d: 'M21 12a9 9 0 1 1-3-6.7M21 4v5h-5', on: refresh, off: false },
              ].map((b) => (
                <Tip key={b.label} label={b.label} side={open ? 'bottom' : 'right'}>
                  <button onClick={b.on} disabled={b.off} aria-label={b.label} className="size-9 rounded-full grid place-items-center text-mu hover:text-fg hover:bg-fg/[.07] transition disabled:opacity-30 disabled:pointer-events-none">
                    <NavIcon d={b.d} />
                  </button>
                </Tip>
              ))}
            </div>
            <button onClick={() => setOpen(!open)} aria-label="Toggle sidebar" className={`h-12 rounded-full flex items-center gap-3 text-mu hover:text-fg hover:bg-fg/[.07] transition mb-2 ${open ? 'px-4 w-full' : 'w-12 justify-center'}`}>
              <Icon n="panel" size={20} />{open && <span className="text-[0.8125rem] font-semibold flex-1 text-left">Collapse</span>}
            </button>
            {[...NAV, ['spacer'], ['settings', 'Settings', 'settings']].map(([id, label, icon]) =>
              id === 'spacer' ? <div key={id} className="flex-1" /> : (
                <Tip key={id} label={label} side="right" off={open}>
                  <button onClick={() => navigate(id === 'mods' ? lastBrowse : id)} aria-label={label} className={`h-12 rounded-full flex items-center gap-3.5 transition outline-none focus-visible:ring-2 ring-ac active:scale-[.98] ${open ? 'px-4 w-full' : 'w-12 justify-center'} ${active === id ? 'bg-ac/15 text-act' : 'text-mu hover:text-fg hover:bg-fg/[.07]'}`}>
                    <Icon n={icon} size={20} />{open && <span className="text-[0.875rem] font-semibold truncate">{label}</span>}
                  </button>
                </Tip>
              ),
            )}
            {open && <div className="px-4 pt-2 font-mono text-[0.6875rem] text-mu2">v1.0.0</div>}
          </nav>
          <main key={route + nonce} className="scroll flex-1 min-w-0 p-10 pb-8 fade"><Boundary resetKey={route}>{page()}</Boundary></main>
        </div>
      </div>
      {palette && <Palette onClose={() => setPalette(false)} />}
      {dialog?.kind === 'create' && <CreateDialog onClose={() => setDialog(null)} />}
      {dialog?.kind === 'bisect' && <BisectDialog inst={dialog.inst} onClose={() => setDialog(null)} />}
      {dialog?.kind === 'import' && <ImportDialog onClose={() => setDialog(null)} />}
      {(dialog?.kind === 'delete' || dialog?.kind === 'rename') && <SimpleDialog d={dialog} onClose={() => setDialog(null)} />}
      {toast && <div role="status" className="pop fixed bottom-6 left-1/2 -translate-x-1/2 z-[95] max-w-[80vw] rounded-2xl bg-fg text-bg px-5 py-3 text-[0.8125rem] font-medium">{toast}</div>}
    </Ctx.Provider>
  )
}
