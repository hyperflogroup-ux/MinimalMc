import { createContext, useContext, useEffect, useState, type ReactNode } from 'react'

export const Ctx = createContext<any>(null)
export const useApp = () => useContext(Ctx)

const P: Record<string, string> = {
  home: 'M3 11l9-8 9 8M5 10v10h14V10',
  box: 'M21 8l-9-5-9 5v8l9 5 9-5zM3 8l9 5 9-5M12 13v8',
  puzzle: 'M10 4a2 2 0 114 0v2h4v4h-2a2 2 0 100 4h2v4h-4v-2a2 2 0 10-4 0v2H6v-4h2a2 2 0 100-4H6V6h4z',
  layers: 'M12 3l9 5-9 5-9-5zM3 13l9 5 9-5M3 17.5l9 5 9-5',
  sun: 'M12 8a4 4 0 100 8 4 4 0 000-8zM12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4',
  image: 'M4 5h16v14H4zM4 16l5-5 4 4 3-3 4 4M9 9h.01',
  globe: 'M12 3a9 9 0 100 18 9 9 0 000-18zM3 12h18M12 3c3 3 3 15 0 18M12 3c-3 3-3 15 0 18',
  download: 'M12 4v11M7 11l5 5 5-5M5 20h14',
  settings: 'M4 7h10M18 7h2M4 17h2M10 17h10M14 5v4M6 15v4',
  search: 'M11 4a7 7 0 100 14 7 7 0 000-14zM20 20l-4-4',
  plus: 'M12 5v14M5 12h14',
  play: 'M7 4l13 8-13 8z',
  more: 'M5 12h.01M12 12h.01M19 12h.01',
  filter: 'M4 6h16M7 12h10M10 18h4',
  sort: 'M7 4v16M3 16l4 4 4-4M17 20V4M13 8l4-4 4 4',
  check: 'M5 12l5 5 9-10',
  x: 'M6 6l12 12M18 6L6 18',
  folder: 'M3 6h6l2 2h10v11H3z',
  copy: 'M8 8h12v12H8zM4 4h12v2M4 4v12h2',
  trash: 'M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13',
  edit: 'M4 20h4L19 9l-4-4L4 16z',
  upload: 'M12 16V4M7 8l5-5 5 5M5 20h14',
  refresh: 'M20 11a8 8 0 00-14-4M4 5v4h4M4 13a8 8 0 0014 4M20 19v-4h-4',
  alert: 'M12 4l9 16H3zM12 10v4M12 17h.01',
  wrench: 'M14 6a4 4 0 005 5l-9 9a2 2 0 01-3-3l9-9a4 4 0 00-2-2z',
  shield: 'M12 3l8 3v6c0 5-4 8-8 9-4-1-8-4-8-9V6z',
  user: 'M12 4a4 4 0 100 8 4 4 0 000-8zM4 20c0-4 4-6 8-6s8 2 8 6',
  chev: 'M9 6l6 6-6 6',
  panel: 'M4 5h16v14H4zM9 5v14',
  chevd: 'M6 9l6 6 6-6',
  grid: 'M4 4h7v7H4zM13 4h7v7h-7zM4 13h7v7H4zM13 13h7v7h-7z',
  list: 'M8 6h12M8 12h12M8 18h12M4 6h.01M4 12h.01M4 18h.01',
  pause: 'M8 5v14M16 5v14',
  cpu: 'M7 7h10v10H7zM9 3v4M15 3v4M9 17v4M15 17v4M3 9h4M3 15h4M17 9h4M17 15h4',
  clock: 'M12 4a8 8 0 100 16 8 8 0 000-16zM12 8v4l3 2',
  camera: 'M4 8h4l2-3h4l2 3h4v11H4zM12 11a3 3 0 100 6 3 3 0 000-6z',
  grip: 'M9 6h.01M15 6h.01M9 12h.01M15 12h.01M9 18h.01M15 18h.01',
  link: 'M10 14a4 4 0 005.7 0l3-3a4 4 0 00-5.7-5.7l-1 1M14 10a4 4 0 00-5.7 0l-3 3a4 4 0 005.7 5.7l1-1',
  ext: 'M14 4h6v6M20 4l-9 9M18 14v6H4V6h6',
  power: 'M12 3v9M6.4 6.4a8 8 0 1011.2 0',
  info: 'M12 4a8 8 0 100 16 8 8 0 000-16zM12 11v5M12 8h.01',
  file: 'M6 3h8l4 4v14H6zM14 3v4h4',
  tree: 'M4 4h6v4H4zM14 10h6v4h-6zM14 16h6v4h-6zM7 8v10h7M7 12h7',
  sparkle: 'M12 3l1.8 5.2L19 10l-5.2 1.8L12 17l-1.8-5.2L5 10l5.2-1.8zM19 16l.7 2 2 .7-2 .7-.7 2-.7-2-2-.7 2-.7z',
  minus: 'M5 12h14',
  square: 'M6 6h12v12H6z',
}

export function Icon({ n, size = 18, className = '', fill = false }: { n: string; size?: number; className?: string; fill?: boolean }) {
  return (
    <svg width={Math.round(size * 1.15)} height={Math.round(size * 1.15)} viewBox="0 0 24 24" fill={fill ? 'currentColor' : 'none'} stroke="currentColor" strokeWidth={n === 'more' ? 2.6 : 1.7} strokeLinecap="round" strokeLinejoin="round" className={`shrink-0 ${className}`}>
      <path d={P[n] || ''} />
    </svg>
  )
}

const PAL = [
  ['#5fb15a', '#3f8a3f', '#7a5a3a'],
  ['#c9a56b', '#8a6a3f', '#5b4426'],
  ['#6aa8d9', '#3d6f9e', '#2a4a6e'],
  ['#c4553f', '#8a3a2a', '#4a2a22'],
  ['#a574e6', '#6a43a8', '#3b2a5e'],
  ['#8a8f93', '#5c6064', '#33363a'],
]
export function Blk({ seed, size = 40, r }: { seed: number; size?: number; r?: number }) {
  let s = seed * 977 + 13
  const rand = () => ((s = (s * 1664525 + 1013904223) % 4294967296) / 4294967296)
  const pal = PAL[seed % PAL.length]
  const cells: ReactNode[] = []
  for (let y = 0; y < 6; y++)
    for (let x = 0; x < 3; x++) {
      const c = pal[Math.floor(rand() * 3)]
      cells.push(<rect key={`a${x}${y}`} x={x} y={y} width="1" height="1" fill={c} />)
      cells.push(<rect key={`b${x}${y}`} x={5 - x} y={y} width="1" height="1" fill={c} />)
    }
  return (
    <svg width={Math.round(size * 1.15)} height={Math.round(size * 1.15)} viewBox="0 0 6 6" shapeRendering="crispEdges" className="shrink-0" style={{ borderRadius: r ?? size * 0.3 }}>
      {cells}
    </svg>
  )
}

type BtnProps = { v?: string; size?: string; icon?: string; loading?: boolean; className?: string; children?: ReactNode; [k: string]: any }
export function Btn({ v = 'secondary', size = 'md', icon, loading, className = '', children, ...p }: BtnProps) {
  const vs: Record<string, string> = {
    primary: 'bg-ac text-acfg hover:brightness-110',
    secondary: 'bg-s3 text-fg hover:bg-fg/10',
    ghost: 'text-mu hover:text-fg hover:bg-fg/[.07]',
    subtle: 'bg-ac/12 text-act hover:bg-ac/20',
    danger: 'bg-red-500/12 text-red-600 dark:text-red-400 hover:bg-red-500/20',
  }
  const sz = size === 'sm' ? 'h-9 text-xs gap-2 ' + (children ? 'px-5' : 'w-9') : 'h-11 text-[0.8125rem] gap-2.5 ' + (children ? 'px-7' : 'w-11')
  return (
    <button {...p} disabled={p.disabled || loading} className={`inline-flex items-center justify-center rounded-full font-semibold whitespace-nowrap select-none transition active:scale-[.97] outline-none focus-visible:ring-2 ring-ac ring-offset-2 ring-offset-bg disabled:opacity-40 disabled:pointer-events-none ${sz} ${vs[v]} ${className}`}>
      {loading ? <span className="size-3.5 rounded-full border-2 border-current border-t-transparent spin" /> : icon && <Icon n={icon} size={size === 'sm' ? 14 : 16} />}
      {children}
    </button>
  )
}

export function Pill({ tone = 'n', icon, children, active, onClick, className = '', dot }: { tone?: string; icon?: string; children?: ReactNode; active?: boolean; onClick?: (e: any) => void; className?: string; dot?: boolean }) {
  const t: Record<string, string> = {
    n: 'bg-fg/[.06] text-mu',
    ac: 'bg-ac/14 text-act',
    ok: 'bg-emerald-500/12 text-emerald-700 dark:text-emerald-400',
    warn: 'bg-amber-500/14 text-amber-700 dark:text-amber-400',
    bad: 'bg-red-500/12 text-red-700 dark:text-red-400',
    info: 'bg-sky-500/12 text-sky-700 dark:text-sky-400',
  }
  const Tag: any = onClick ? 'button' : 'span'
  return (
    <Tag onClick={onClick} className={`inline-flex items-center gap-2 h-8 px-4 rounded-full text-[0.7812rem] font-semibold whitespace-nowrap transition ring-1 ring-inset ring-fg/[.08] ${active ? 'bg-fg text-bg' : t[tone]} ${onClick && !active ? 'hover:bg-fg/10 hover:text-fg' : ''} ${className}`}>
      {dot && <span className="size-1.5 rounded-full bg-current" />}
      {icon && <Icon n={icon} size={14} />}
      {children}
    </Tag>
  )
}

export function Tip({ label, children, side = 'top', off }: { label: string; children: ReactNode; side?: 'top' | 'right' | 'bottom'; off?: boolean }) {
  if (off) return <>{children}</>
  const pos = side === 'right' ? 'left-full ml-2 top-1/2 -translate-y-1/2' : side === 'bottom' ? 'top-full mt-1.5 left-1/2 -translate-x-1/2' : 'bottom-full mb-1.5 left-1/2 -translate-x-1/2'
  return (
    <span className="has-tip relative inline-flex">
      {children}
      <span className={`tip absolute z-50 ${pos} px-2 py-1 rounded-lg bg-fg text-bg text-[0.6875rem] font-medium whitespace-nowrap`}>{label}</span>
    </span>
  )
}

export function Toggle({ on, onChange, disabled }: { on: boolean; onChange?: (v: boolean) => void; disabled?: boolean }) {
  return (
    <button role="switch" aria-checked={on} disabled={disabled} onClick={() => onChange?.(!on)} className={`relative w-11 h-6 rounded-full transition shrink-0 outline-none focus-visible:ring-2 ring-ac disabled:opacity-40 ${on ? 'bg-ac' : 'bg-fg/20'}`}>
      <span className={`absolute top-0.5 left-0.5 size-5 rounded-full bg-white shadow transition-transform ${on ? 'translate-x-5' : ''}`} />
    </button>
  )
}

export function Check({ on, onChange, label }: { on: boolean; onChange?: (v: boolean) => void; label?: string }) {
  return (
    <button onClick={() => onChange?.(!on)} className="inline-flex items-center gap-2 text-[0.8125rem] outline-none">
      <span className={`size-4 rounded-[5px] grid place-items-center transition ${on ? 'bg-ac text-acfg' : 'bg-fg/10'}`}>{on && <Icon n="check" size={11} />}</span>
      {label}
    </button>
  )
}
export function Radio({ on, onChange, label }: { on: boolean; onChange?: () => void; label?: string }) {
  return (
    <button onClick={onChange} className="inline-flex items-center gap-2 text-[0.8125rem] outline-none">
      <span className={`size-4 rounded-full grid place-items-center transition ${on ? 'bg-ac' : 'bg-fg/10'}`}>{on && <span className="size-1.5 rounded-full bg-acfg" />}</span>
      {label}
    </button>
  )
}

export function Seg({ items, value, onChange, className = '' }: { items: { id: string; label: string; icon?: string }[]; value: string; onChange: (v: string) => void; className?: string }) {
  return (
    <div className={`inline-flex p-1 rounded-full bg-s2 gap-0.5 ${className}`}>
      {items.map((i) => (
        <button key={i.id} onClick={() => onChange(i.id)} className={`h-10 px-5 rounded-full text-[0.8125rem] font-semibold inline-flex items-center gap-2 transition outline-none focus-visible:ring-2 ring-ac ${value === i.id ? 'bg-s3 text-fg shadow-sm' : 'text-mu hover:text-fg'}`}>
          {i.icon && <Icon n={i.icon} size={14} />}
          {i.label}
        </button>
      ))}
    </div>
  )
}

export function Input({ icon, className = '', big, ...p }: { icon?: string; big?: boolean; className?: string; [k: string]: any }) {
  return (
    <label className={`flex items-center gap-2.5 rounded-full bg-s2 px-5 text-[0.8125rem] transition focus-within:ring-2 ring-ac/60 ${big ? 'h-14 text-sm' : 'h-11'} ${className}`}>
      {icon && <Icon n={icon} size={big ? 18 : 15} className="text-mu" />}
      <input {...p} className="flex-1 min-w-0 bg-transparent outline-none placeholder:text-mu2" />
    </label>
  )
}

export function Select({ value, onChange, options, className = '' }: { value: string; onChange?: (v: string) => void; options: string[]; className?: string }) {
  return (
    <div className={`relative inline-flex ${className}`}>
      <select value={value} onChange={(e) => onChange?.(e.target.value)} className="appearance-none h-11 w-full rounded-full bg-s2 pl-5 pr-10 text-[0.8125rem] outline-none focus-visible:ring-2 ring-ac/60 cursor-pointer">
        {options.map((o) => (
          <option key={o} className="bg-s1">{o}</option>
        ))}
      </select>
      <Icon n="chevd" size={14} className="absolute right-3.5 top-1/2 -translate-y-1/2 text-mu pointer-events-none" />
    </div>
  )
}

export function Progress({ v, className = '', tone = 'bg-ac' }: { v: number; className?: string; tone?: string }) {
  return (
    <div className={`h-2 rounded-full bg-fg/10 overflow-hidden ${className}`}>
      <div className={`h-full rounded-full ${tone} transition-all duration-300`} style={{ width: `${v}%` }} />
    </div>
  )
}

export type MenuItem = { label?: string; icon?: string; danger?: boolean; sep?: boolean; onClick?: () => void; disabled?: boolean }
export function useMenu(items: MenuItem[]) {
  const [pos, setPos] = useState<{ x: number; y: number } | null>(null)
  useEffect(() => {
    if (!pos) return
    const c = () => setPos(null)
    const k = (e: KeyboardEvent) => e.key === 'Escape' && c()
    window.addEventListener('click', c)
    window.addEventListener('contextmenu', c, true)
    window.addEventListener('keydown', k)
    return () => {
      window.removeEventListener('click', c)
      window.removeEventListener('contextmenu', c, true)
      window.removeEventListener('keydown', k)
    }
  }, [pos])
  const node = pos && (
    <div onClick={(e) => e.stopPropagation()} style={{ left: Math.min(pos.x, window.innerWidth - 210), top: Math.min(pos.y, window.innerHeight - items.length * 34 - 20), boxShadow: 'var(--shadow)' }} className="pop fixed z-[80] w-48 p-1.5 rounded-2xl bg-s2">
      {items.map((it, i) =>
        it.sep ? (
          <div key={i} className="my-1 h-px bg-line" />
        ) : (
          <button key={i} disabled={it.disabled} onClick={() => { setPos(null); it.onClick?.() }} className={`w-full h-8 px-2.5 rounded-lg flex items-center gap-2.5 text-[0.8125rem] text-left transition disabled:opacity-40 ${it.danger ? 'text-red-600 dark:text-red-400 hover:bg-red-500/12' : 'hover:bg-fg/[.07]'}`}>
            {it.icon && <Icon n={it.icon} size={15} className={it.danger ? '' : 'text-mu'} />}
            {it.label}
          </button>
        ),
      )}
    </div>
  )
  return {
    node,
    onContext: (e: any) => { e.preventDefault(); e.stopPropagation(); setPos({ x: e.clientX, y: e.clientY }) },
    openFrom: (e: any) => { e.stopPropagation(); const r = e.currentTarget.getBoundingClientRect(); setPos({ x: r.left, y: r.bottom + 6 }) },
  }
}

export function PillMenu({ label, icon, items, active }: { label: string; icon?: string; items: MenuItem[]; active?: boolean }) {
  const m = useMenu(items)
  return (
    <>
      <button onClick={m.openFrom} className={`h-11 px-5 rounded-full inline-flex items-center gap-2 text-[0.8125rem] font-semibold transition ${active ? 'bg-ac/14 text-act' : 'bg-s2 text-mu hover:text-fg hover:bg-s3'}`}>
        {icon && <Icon n={icon} size={15} />}
        {label}
        <Icon n="chevd" size={13} />
      </button>
      {m.node}
    </>
  )
}

export function MoreBtn({ m }: { m: ReturnType<typeof useMenu> }) {
  return (
    <Tip label="More">
      <Btn v="ghost" size="sm" icon="more" onClick={m.openFrom} aria-label="More" />
    </Tip>
  )
}

export function Row({ title, desc, children }: { title: string; desc?: string; children?: ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-6 py-3.5">
      <div className="min-w-0">
        <div className="text-[0.8438rem] font-medium">{title}</div>
        {desc && <div className="text-xs text-mu mt-0.5">{desc}</div>}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  )
}

export function Panel({ children, className = '' }: { children: ReactNode; className?: string }) {
  return <div className={`rounded-[20px] bg-s1 ${className}`} style={{ boxShadow: '0 0 0 1px var(--line)' }}>{children}</div>
}

export function PageHead({ title, sub, children }: { title: string; sub?: string; children?: ReactNode }) {
  return (
    <div className="flex items-end justify-between gap-4 mb-6">
      <div>
        <h1 className="text-[1.625rem] leading-8 font-semibold tracking-tight">{title}</h1>
        {sub && <p className="text-[0.8125rem] text-mu mt-1">{sub}</p>}
      </div>
      <div className="flex items-center gap-2 flex-wrap justify-end">{children}</div>
    </div>
  )
}

export function Modal({ children, onClose, w = 520 }: { children: ReactNode; onClose: () => void; w?: number }) {
  useEffect(() => {
    const k = (e: KeyboardEvent) => e.key === 'Escape' && onClose()
    window.addEventListener('keydown', k)
    return () => window.removeEventListener('keydown', k)
  }, [onClose])
  return (
    <div className="fixed inset-0 z-[70] grid place-items-center bg-black/45 fade" onMouseDown={onClose}>
      <div onMouseDown={(e) => e.stopPropagation()} style={{ width: w, maxWidth: '92vw', boxShadow: 'var(--shadow)' }} className="pop rounded-[24px] bg-s1 max-h-[90vh] flex flex-col">
        {children}
      </div>
    </div>
  )
}

export function DialogCard({ kind, title, body, actions, children, static: st }: { kind?: 'info' | 'warn' | 'error' | 'ok'; title: string; body?: string; actions: ReactNode; children?: ReactNode; static?: boolean }) {
  const k: Record<string, [string, string]> = { info: ['info', 'bg-sky-500/12 text-sky-500'], warn: ['alert', 'bg-amber-500/14 text-amber-500'], error: ['x', 'bg-red-500/12 text-red-500'], ok: ['check', 'bg-emerald-500/12 text-emerald-500'] }
  return (
    <div className={`p-6 ${st ? 'rounded-[24px] bg-s1' : ''}`} style={st ? { boxShadow: 'var(--shadow)' } : {}}>
      {kind && <div className={`size-10 rounded-full grid place-items-center mb-4 ${k[kind][1]}`}><Icon n={k[kind][0]} size={18} /></div>}
      <h3 className="text-[1.0625rem] font-semibold tracking-tight">{title}</h3>
      {body && <p className="text-[0.8125rem] text-mu mt-1.5 leading-relaxed">{body}</p>}
      {children && <div className="mt-4">{children}</div>}
      <div className="flex justify-end gap-2 mt-6">{actions}</div>
    </div>
  )
}
