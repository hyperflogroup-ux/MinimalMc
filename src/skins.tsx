import { Fragment, useEffect, useMemo, useRef, useState } from 'react'
import { backend, isDesktop } from './backend'
import { Btn, Icon, Panel, PageHead, Pill, Seg, Tip } from './ui'

type Skin = { id: number; name: string; url: string; custom: boolean; key?: string }

function defaultSkin(): string {
  const c = document.createElement('canvas')
  c.width = c.height = 64
  const g = c.getContext('2d')!
  const r = (col: string, x: number, y: number, w: number, h: number) => { g.fillStyle = col; g.fillRect(x, y, w, h) }
  const skin = '#c4906a', hair = '#4a3322', shirt = '#2fa8b5', pants = '#3a3f9a', shoe = '#6b6b6b'
  r(skin, 0, 0, 32, 16); r(hair, 8, 0, 16, 8); r(hair, 0, 8, 32, 3); r(hair, 24, 8, 8, 8)
  r('#fff', 10, 12, 2, 2); r('#fff', 12, 12, 0, 0); r('#3b2a8a', 10, 13, 1, 1); r('#fff', 13, 13, 0, 0)
  r('#fff', 12, 13, 2, 1); r('#fff', 16, 13, 0, 0); r('#fff', 18, 13, 2, 1); r('#3b2a8a', 12, 13, 1, 1); r('#3b2a8a', 19, 13, 1, 1)
  r('#8a5a44', 13, 15, 6, 1)
  r(shirt, 16, 16, 24, 16); r(skin, 40, 16, 16, 16); r(shirt, 40, 20, 16, 5)
  r(pants, 0, 16, 16, 16); r(shoe, 0, 28, 16, 4)
  r(skin, 32, 48, 16, 16); r(shirt, 32, 52, 16, 5); r(pants, 16, 48, 16, 16); r(shoe, 16, 60, 16, 4)
  return c.toDataURL()
}

function faceUrl(img: HTMLImageElement, s: number, x: number, y: number, w: number, h: number) {
  const c = document.createElement('canvas')
  c.width = Math.max(1, w * 8); c.height = Math.max(1, h * 8)
  const g = c.getContext('2d')!
  g.imageSmoothingEnabled = false
  g.drawImage(img, x * s, y * s, w * s, h * s, 0, 0, c.width, c.height)
  return c.toDataURL()
}

const U = 15

function Cube({ img, s, u, v, w, h, d, grow = 0, at, over, extra, anim }: { img: HTMLImageElement; s: number; u: number; v: number; w: number; h: number; d: number; grow?: number; at: [number, number, number]; over?: boolean; extra?: React.ReactNode; anim?: string }) {
  const faces = useMemo(() => ({
    right: faceUrl(img, s, u, v + d, d, h),
    front: faceUrl(img, s, u + d, v + d, w, h),
    left: faceUrl(img, s, u + d + w, v + d, d, h),
    back: faceUrl(img, s, u + 2 * d + w, v + d, w, h),
    top: faceUrl(img, s, u + d, v, w, d),
    bottom: faceUrl(img, s, u + d + w, v, w, d),
  }), [img, s, u, v, w, h, d])
  const W = (w + grow * 2) * U, H = (h + grow * 2) * U, D = (d + grow * 2) * U
  const base: React.CSSProperties = { position: 'absolute', backfaceVisibility: over ? 'visible' : 'hidden', imageRendering: 'pixelated', backgroundSize: '100% 100%' }
  const f = (url: string, ww: number, hh: number, t: string): React.CSSProperties => ({ ...base, width: ww, height: hh, left: (W - ww) / 2, top: (H - hh) / 2, backgroundImage: `url(${url})`, transform: t })
  return (
    <div style={{ position: 'absolute', width: W, height: H, left: at[0] * U - W / 2, top: at[1] * U, transformStyle: 'preserve-3d', transform: `translateZ(${at[2] * U}px)` }}>
      <div className={anim} style={{ position: 'absolute', inset: 0, transformStyle: 'preserve-3d', transformOrigin: '50% 8%' }}>
      <div style={f(faces.front, W, H, `translateZ(${D / 2}px)`)} />
      <div style={f(faces.back, W, H, `rotateY(180deg) translateZ(${D / 2}px)`)} />
      <div style={f(faces.right, D, H, `rotateY(-90deg) translateZ(${W / 2}px)`)} />
      <div style={f(faces.left, D, H, `rotateY(90deg) translateZ(${W / 2}px)`)} />
      <div style={f(faces.top, W, D, `rotateX(90deg) translateZ(${H / 2}px)`)} />
      <div style={f(faces.bottom, W, D, `rotateX(-90deg) translateZ(${H / 2}px)`)} />
      {extra}
      </div>
    </div>
  )
}

function Model({ url, slim, walk }: { url: string; slim: boolean; walk: boolean }) {
  const [img, setImg] = useState<HTMLImageElement | null>(null)
  useEffect(() => {
    const i = new Image()
    i.onload = () => setImg(i)
    i.src = url
  }, [url])
  if (!img) return null
  const s = img.naturalWidth / 64
  const legacy = img.naturalHeight < img.naturalWidth
  const aw = slim ? 3 : 4
  const eye = (() => {
    const c = document.createElement('canvas'); c.width = c.height = 1
    const g = c.getContext('2d')!
    g.drawImage(img, Math.floor(11 * s), Math.floor(13 * s), 1, 1, 0, 0, 1, 1)
    const [r, gg, b] = g.getImageData(0, 0, 1, 1).data
    return `rgb(${r},${gg},${b})`
  })()
  const lids = [1, 5].map((x) => (
    <div key={x} className="blink" style={{ position: 'absolute', left: x * U, top: 4 * U, width: 2 * U, height: 2 * U, background: eye, backfaceVisibility: 'hidden', transform: `translateZ(${4 * U + 0.8}px)` }} />
  ))
  const parts: { u: number; v: number; w: number; h: number; d: number; at: [number, number, number]; o?: [number, number]; anim?: string }[] = [
    { u: 0, v: 0, w: 8, h: 8, d: 8, at: [0, 0, 0], o: [32, 0] },
    { u: 16, v: 16, w: 8, h: 12, d: 4, at: [0, 8, 0], o: [16, 32] },
    { u: 40, v: 16, w: aw, h: 12, d: 4, at: [-(4 + aw / 2), 8, 0], o: [40, 32], anim: 'swa' },
    { u: legacy ? 40 : 32, v: legacy ? 16 : 48, w: aw, h: 12, d: 4, at: [4 + aw / 2, 8, 0], o: legacy ? undefined : [48, 48], anim: 'swb' },
    { u: 0, v: 16, w: 4, h: 12, d: 4, at: [-2, 20, 0], o: [0, 32], anim: 'swb' },
    { u: legacy ? 0 : 16, v: legacy ? 16 : 48, w: 4, h: 12, d: 4, at: [2, 20, 0], o: legacy ? undefined : [0, 48], anim: 'swa' },
  ]
  return (
    <div style={{ position: 'absolute', left: 0, top: -16 * U, transformStyle: 'preserve-3d' }}>
      {parts.map((p, i) => {
        const an = walk ? p.anim : undefined
        return (
          <Fragment key={i}>
            <Cube img={img} s={s} u={p.u} v={p.v} w={p.w} h={p.h} d={p.d} at={p.at} anim={an} extra={i === 0 ? lids : undefined} />
            {p.o && <Cube img={img} s={s} u={p.o[0]} v={p.o[1]} w={p.w} h={p.h} d={p.d} grow={0.25} at={[p.at[0], p.at[1] - 0.25, p.at[2]]} over anim={an} />}
          </Fragment>
        )
      })}
    </div>
  )
}

function Viewer({ url, slim }: { url: string; slim: boolean }) {
  const [walk, setWalk] = useState(true)
  const [rot, setRot] = useState({ x: -12, y: 28 })
  const [auto, setAuto] = useState(true)
  const [zoom, setZoom] = useState(1)
  const drag = useRef<{ x: number; y: number } | null>(null)
  const grabbing = useRef(false)

  useEffect(() => {
    if (!auto) return
    let raf = 0, last = performance.now()
    const tick = (t: number) => {
      if (!grabbing.current) setRot((r) => ({ ...r, y: r.y + (t - last) * 0.035 }))
      last = t
      raf = requestAnimationFrame(tick)
    }
    raf = requestAnimationFrame(tick)
    return () => cancelAnimationFrame(raf)
  }, [auto])

  return (
    <div
      className="relative h-[34rem] rounded-[24px] bg-s1 overflow-hidden select-none touch-none cursor-grab active:cursor-grabbing"
      style={{ boxShadow: '0 0 0 1px var(--line)', perspective: 1400 }}
      onPointerDown={(e) => { drag.current = { x: e.clientX, y: e.clientY }; grabbing.current = true; e.currentTarget.setPointerCapture(e.pointerId) }}
      onPointerMove={(e) => {
        if (!drag.current) return
        const dx = e.clientX - drag.current.x, dy = e.clientY - drag.current.y
        drag.current = { x: e.clientX, y: e.clientY }
        setRot((r) => ({ y: r.y + dx * 0.6, x: Math.max(-80, Math.min(80, r.x - dy * 0.5)) }))
      }}
      onPointerUp={() => { drag.current = null; grabbing.current = false }}
      onWheel={(e) => setZoom((z) => Math.max(0.5, Math.min(1.8, z - e.deltaY * 0.001)))}
    >
      <div className="absolute inset-0 pointer-events-none" style={{ background: 'radial-gradient(closest-side at 50% 55%, rgba(255,255,255,.07), transparent)' }} />
      <div className="absolute left-1/2 top-1/2" style={{ transformStyle: 'preserve-3d', transform: `scale(${zoom}) rotateX(${rot.x}deg) rotateY(${rot.y}deg)` }}>
        <Model url={url} slim={slim} walk={walk} />
      </div>
      <div className="absolute left-1/2 -translate-x-1/2 bottom-24 w-40 h-4 rounded-full pointer-events-none" style={{ background: 'radial-gradient(closest-side, rgba(255,255,255,.12), transparent)' }} />
      <div className="absolute top-4 left-4 text-xs text-mu pointer-events-none">Drag to rotate · Scroll to zoom</div>
      <div className="absolute bottom-4 left-1/2 -translate-x-1/2 flex gap-2" onPointerDown={(e) => e.stopPropagation()}>
        <Btn v={auto ? 'primary' : 'secondary'} size="sm" icon="refresh" onClick={() => setAuto(!auto)}>Auto-rotate</Btn>
        <Btn v={walk ? 'primary' : 'secondary'} size="sm" onClick={() => setWalk(!walk)}>Walk</Btn>
        <Btn v="secondary" size="sm" onClick={() => { setRot({ x: -12, y: 28 }); setZoom(1) }}>Reset view</Btn>
      </div>
    </div>
  )
}

export default function Skins() {
  const [skins, setSkins] = useState<Skin[]>(() => [{ id: 1, name: 'Steve_Dev', url: defaultSkin(), custom: false }])
  const [sel, setSel] = useState(1)
  const [active, setActive] = useState(1)
  const [slim, setSlim] = useState('classic')
  const [over, setOver] = useState(false)
  const [err, setErr] = useState('')
  const inp = useRef<HTMLInputElement>(null)
  useEffect(() => {
    if (!isDesktop()) return
    backend.listSkins().then((l) => {
      if (!l.length) return
      const n = l.map((f, i) => ({ id: 100 + i, name: f.name, url: `data:image/png;base64,${f.png_base64}`, custom: true, key: f.name }))
      setSkins((p) => [...p, ...n])
      const a = l.findIndex((f) => f.active)
      if (a >= 0) { setActive(100 + a); setSel(100 + a) }
    }).catch(() => {})
  }, [])
  const cur = skins.find((s) => s.id === sel) ?? skins[0]

  const add = (files: FileList | File[]) => {
    const list = Array.from(files).filter((f) => f.type.startsWith('image/'))
    if (!list.length) { setErr('That file is not an image.'); return }
    setErr('')
    const n: Skin[] = list.map((f, i) => ({ id: Date.now() + i, name: f.name.replace(/\.[^.]+$/, ''), url: URL.createObjectURL(f), custom: true }))
    setSkins((p) => [...p, ...n])
    setSel(n[n.length - 1].id)
    if (isDesktop()) list.forEach((f, i) => backend.saveSkin(f).then((key) => setSkins((p) => p.map((s) => (s.id === n[i].id ? { ...s, key } : s)))).catch((e) => setErr(String(e))))
  }
  const choose = (s: Skin) => { setActive(s.id); if (s.key) backend.setActiveSkin(s.key).catch(() => {}) }
  const remove = (id: number) => {
    const k = skins.find((s) => s.id === id)?.key
    if (k) backend.deleteSkin(k).catch(() => {})
    setSkins((p) => p.filter((s) => s.id !== id))
    if (sel === id) setSel(1)
    if (active === id) setActive(1)
  }

  return (
    <div className="max-w-6xl mx-auto" onDragOver={(e) => { e.preventDefault(); setOver(true) }} onDragLeave={() => setOver(false)} onDrop={(e) => { e.preventDefault(); setOver(false); add(e.dataTransfer.files) }}>
      <PageHead title="Skins" sub="Upload any image and preview it in 3D.">
        <Btn v="primary" icon="upload" onClick={() => inp.current?.click()}>Upload skin</Btn>
      </PageHead>
      <input ref={inp} type="file" accept="image/*" multiple hidden onChange={(e) => { if (e.target.files) add(e.target.files); e.target.value = '' }} />
      <div className="grid lg:grid-cols-[1fr_22rem] gap-5">
        <div className="relative">
          <Viewer url={cur.url} slim={slim === 'slim'} />
          {over && <div className="absolute inset-0 rounded-[24px] bg-bg/80 flex items-center justify-center text-[0.9375rem] font-semibold pointer-events-none" style={{ boxShadow: '0 0 0 2px var(--ac) inset' }}>Drop image to add skin</div>}
        </div>
        <div className="space-y-4">
          <Panel className="p-5">
            <div className="flex items-center gap-2 mb-1">
              <div className="text-[0.9375rem] font-semibold truncate">{cur.name}</div>
              {active === cur.id && <Pill tone="ok" icon="check">Active</Pill>}
            </div>
            <div className="text-xs text-mu mb-4">Model type</div>
            <Seg value={slim} onChange={setSlim} items={[{ id: 'classic', label: 'Classic' }, { id: 'slim', label: 'Slim' }]} />
            <div className="flex gap-2 mt-5">
              <Btn v="primary" icon="check" disabled={active === cur.id} onClick={() => choose(cur)}>Set active</Btn>
              {cur.custom && <Tip label="Remove"><Btn v="secondary" icon="trash" onClick={() => remove(cur.id)} /></Tip>}
            </div>
            {err && <div className="text-xs text-red-500 mt-3">{err}</div>}
          </Panel>
          <div>
            <div className="text-[0.8125rem] font-medium mb-2">Library <span className="text-mu2 text-xs ml-1">{skins.length}</span></div>
            <div className="grid grid-cols-3 gap-2.5">
              {skins.map((s) => (
                <button key={s.id} onClick={() => setSel(s.id)} className={`relative aspect-square rounded-2xl bg-s1 overflow-hidden transition hover:bg-s2 ${sel === s.id ? 'ring-2 ring-ac' : 'ring-1 ring-line'}`}>
                  <div className="absolute inset-0 bg-no-repeat" style={{ backgroundImage: `url(${s.url})`, backgroundSize: 'contain', backgroundPosition: 'center', imageRendering: 'pixelated' }} />
                  {active === s.id && <span className="absolute top-1.5 right-1.5 size-5 rounded-full bg-ac text-acfg flex items-center justify-center"><Icon n="check" size={11} /></span>}
                </button>
              ))}
              <button onClick={() => inp.current?.click()} className="aspect-square rounded-2xl border border-dashed border-line text-mu hover:text-fg hover:bg-fg/[.05] transition flex flex-col items-center justify-center gap-1.5 text-xs">
                <Icon n="plus" size={18} />Add
              </button>
            </div>
            <div className="text-xs text-mu2 mt-3">Any image type works. 64×64 and 64×32 skins map to the model; other images are scaled onto it.</div>
          </div>
        </div>
      </div>
    </div>
  )
}
