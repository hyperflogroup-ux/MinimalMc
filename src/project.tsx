import { useEffect, useState } from 'react'
import { Blk, Btn, Icon, Pill, PillMenu } from './ui'
import { backend, type MrHit, type ProjectDetail, type ProjectGallery, type ProjectVersion } from './backend'
import type { Inst } from './data'
import { Markdown } from './markdown'

// A project's own page, like Modrinth's: description, pictures, who made it, details, and every
// version, each with its own Install button.

const TABS = ['Description', 'Gallery', 'Versions']
const KIND: Record<string, string> = { mods: 'mod', modpacks: 'modpack', shaders: 'shader', packs: 'resourcepack' }
const CF_KIND: Record<string, string> = { mods: 'mc-mods', modpacks: 'modpacks', shaders: 'shaders', packs: 'texture-packs' }
const LABEL: Record<string, string> = { mods: 'mods', modpacks: 'modpacks', shaders: 'shaders', packs: 'resource packs' }

const fmt = (n: number) => (n >= 1e6 ? `${(n / 1e6).toFixed(1)}M` : n >= 1e3 ? `${Math.round(n / 1e3)}K` : String(n))
const day = (iso: string) => {
  const d = new Date(iso)
  return Number.isNaN(d.getTime()) ? '' : d.toLocaleDateString()
}
const cap = (s: string) => s.charAt(0).toUpperCase() + s.slice(1)

type Props = {
  hit: MrHit
  type: string
  inst: Inst | undefined
  insts: Inst[]
  onPick: (id: number) => void
  onBack: () => void
  onInstall: () => void
  onInstallVersion: (versionId: string) => void
  source: 'modrinth' | 'curseforge'
  /** '' when idle, 'latest' while the newest version installs, or the id of the version being installed. */
  busy: string
  note: string
}

export function ProjectView(p: Props) {
  const { hit, type, inst } = p
  const isPack = type === 'modpacks'
  const [d, setD] = useState<ProjectDetail | null>(null)
  const [err, setErr] = useState('')
  const [tab, setTab] = useState('Description')
  const [compatOnly, setCompatOnly] = useState(!isPack)
  const [open, setOpen] = useState('')
  const [zoom, setZoom] = useState<ProjectGallery | null>(null)
  const [allVersions, setAllVersions] = useState(false)

  useEffect(() => {
    setD(null)
    setErr('')
    ;(p.source === 'curseforge' ? backend.curseforgeProject : backend.modrinthProject)(hit.project_id).then(setD).catch((e) => setErr(String(e)))
  }, [hit.project_id, p.source])
  useEffect(() => {
    if (!zoom) return
    const k = (e: KeyboardEvent) => { if (e.key === 'Escape') setZoom(null) }
    window.addEventListener('keydown', k)
    return () => window.removeEventListener('keydown', k)
  }, [zoom])

  const pr = d?.project
  const title = pr?.title || hit.title
  const summary = pr?.description || hit.description
  const icon = pr?.icon_url || hit.icon_url
  const owner = d?.members.find((m) => m.role.toLowerCase() === 'owner') ?? d?.members[0]
  const author = owner?.user.username || hit.author
  const cf = p.source === 'curseforge'
  const slug = pr?.slug || hit.slug || hit.project_id
  const page = cf ? `https://www.curseforge.com/minecraft/${CF_KIND[type] ?? 'mc-mods'}/${slug}` : `https://modrinth.com/${KIND[type] ?? 'mod'}/${slug}`
  const open_ = (url: string) => backend.openUrl(url).catch(() => {})

  // Does a version fit the selected instance?
  const fits = (v: ProjectVersion) => {
    if (!inst || isPack) return false
    if (!v.game_versions.includes(inst.mc)) return false
    if (type !== 'mods') return true
    const l = inst.loader.toLowerCase()
    if (l === 'vanilla') return false
    return v.loaders.includes(l) || (l === 'quilt' && v.loaders.includes('fabric'))
  }
  const versions = (d?.versions ?? []).filter((v) => isPack || !compatOnly || fits(v))
  const gameVersions = pr?.game_versions ?? []
  const shownGV = allVersions ? gameVersions : gameVersions.slice(-8).reverse()

  const links: [string, string | null | undefined][] = [
    ['Source code', pr?.source_url],
    ['Report an issue', pr?.issues_url],
    ['Wiki', pr?.wiki_url],
    ['Discord', pr?.discord_url],
  ]

  return (
    <div className="max-w-5xl mx-auto">
      <button onClick={p.onBack} className="text-xs text-mu hover:text-fg inline-flex items-center gap-1 mb-4">
        <Icon n="chev" size={12} className="rotate-180" />Back to {LABEL[type] ?? 'results'}
      </button>

      <div className="flex gap-5 items-start flex-wrap">
        {icon ? <img src={icon} alt="" className="size-20 shrink-0 rounded-[20px] object-cover bg-s2" /> : <Blk seed={title.length} size={80} r={20} />}
        <div className="flex-1 min-w-64">
          <h1 className="text-[1.625rem] font-semibold tracking-tight leading-8">{title}</h1>
          <p className="text-[0.875rem] text-mu mt-1 leading-relaxed">{summary}</p>
          <div className="flex flex-wrap items-center gap-1.5 mt-3">
            <Pill icon="user">{author}</Pill>
            <Pill icon="download">{fmt(pr?.downloads ?? hit.downloads)} downloads</Pill>
            {pr && pr.followers > 0 && <Pill>{fmt(pr.followers)} followers</Pill>}
            {(pr?.categories ?? hit.categories).slice(0, 4).map((c) => <Pill key={c} className="capitalize">{c}</Pill>)}
          </div>
        </div>
        <div className="flex flex-col gap-2 items-stretch min-w-52">
          {!isPack && (p.insts.length ? (
            <PillMenu label={`Install to: ${inst?.name ?? 'none'}`} icon="box" items={p.insts.map((i) => ({ label: i.name, icon: inst?.id === i.id ? 'check' : undefined, onClick: () => p.onPick(i.id) }))} />
          ) : (
            <div className="text-xs text-mu">Create an instance to install into.</div>
          ))}
          <Btn v="primary" loading={p.busy === 'latest'} disabled={(!isPack && !inst) || !!p.busy} onClick={p.onInstall}>
            {isPack ? 'Install as a new instance' : 'Install latest that fits'}
          </Btn>
          <Btn v="secondary" onClick={() => open_(page)}>{cf ? 'Open on CurseForge' : 'Open on Modrinth'}</Btn>
        </div>
      </div>

      {p.note && <div className="rounded-2xl bg-s2 px-4 py-3 text-xs text-mu mt-5 leading-relaxed">{p.note}</div>}

      <div className="flex gap-1 mt-7 mb-6 overflow-x-auto scroll pb-1">
        {TABS.map((t) => (
          <button key={t} onClick={() => setTab(t)} className={`h-10 px-5 rounded-full text-[0.8125rem] font-medium whitespace-nowrap transition ${tab === t ? 'bg-s3 text-fg' : 'text-mu hover:text-fg hover:bg-fg/[.05]'}`}>
            {t}{t === 'Gallery' && pr ? ` (${pr.gallery.length})` : ''}{t === 'Versions' && d ? ` (${d.versions.length})` : ''}
          </button>
        ))}
      </div>

      {err && <div className="rounded-2xl bg-s2 px-5 py-8 text-center text-sm text-mu break-words">{err}</div>}
      {!d && !err && <div className="py-16 text-center text-sm text-mu">Loading…</div>}

      {d && pr && tab === 'Description' && (
        <div className="grid gap-8 lg:grid-cols-[minmax(0,1fr)_260px]">
          <div>{pr.body.trim() ? <Markdown text={pr.body} /> : <p className="text-sm text-mu">This project has no description.</p>}</div>
          <aside className="space-y-6 text-[0.8125rem]">
            <div>
              <div className="font-medium mb-2">Details</div>
              <div className="space-y-1.5 text-mu">
                {pr.license && pr.license.name && <div>License: <span className="text-fg">{pr.license.name}</span></div>}
                <div>Published: <span className="text-fg">{day(pr.published)}</span></div>
                <div>Updated: <span className="text-fg">{day(pr.updated)}</span></div>
                {pr.client_side && <div>Client: <span className="text-fg">{cap(pr.client_side)}</span></div>}
                {pr.server_side && <div>Server: <span className="text-fg">{cap(pr.server_side)}</span></div>}
              </div>
            </div>
            {pr.loaders.length > 0 && (
              <div>
                <div className="font-medium mb-2">Works with</div>
                <div className="flex flex-wrap gap-1.5">{pr.loaders.map((l) => <Pill key={l}>{cap(l)}</Pill>)}</div>
              </div>
            )}
            {gameVersions.length > 0 && (
              <div>
                <div className="font-medium mb-2">Minecraft versions</div>
                <div className="flex flex-wrap gap-1.5">
                  {shownGV.map((g) => <Pill key={g}>{g}</Pill>)}
                  {gameVersions.length > 8 && <Pill active={allVersions} onClick={() => setAllVersions(!allVersions)}>{allVersions ? 'Show fewer' : `+${gameVersions.length - 8} older`}</Pill>}
                </div>
              </div>
            )}
            {links.some((l) => l[1]) && (
              <div>
                <div className="font-medium mb-2">Links</div>
                <div className="flex flex-col items-start gap-1">
                  {links.filter((l) => l[1]).map(([name, url]) => (
                    <button key={name} onClick={() => open_(url as string)} className="text-act hover:underline">{name}</button>
                  ))}
                </div>
              </div>
            )}
            {d.members.length > 0 && (
              <div>
                <div className="font-medium mb-2">Team</div>
                <div className="space-y-2">
                  {d.members.slice(0, 8).map((m) => (
                    <div key={m.user.username} className="flex items-center gap-2.5">
                      {m.user.avatar_url ? <img src={m.user.avatar_url} alt="" className="size-7 rounded-full object-cover bg-s2" /> : <Blk seed={m.user.username.length} size={28} r={999} />}
                      <div className="min-w-0"><div className="truncate">{m.user.username}</div><div className="text-xs text-mu">{m.role}</div></div>
                    </div>
                  ))}
                </div>
              </div>
            )}
          </aside>
        </div>
      )}

      {d && pr && tab === 'Gallery' && (
        pr.gallery.length ? (
          <div className="grid gap-3 grid-cols-[repeat(auto-fill,minmax(260px,1fr))]">
            {pr.gallery.map((g) => (
              <button key={g.url} onClick={() => setZoom(g)} className="text-left rounded-2xl overflow-hidden bg-s1 hover:bg-s2 transition" style={{ boxShadow: '0 0 0 1px var(--line)' }}>
                <img src={g.url} alt={g.title ?? ''} loading="lazy" className="w-full aspect-video object-cover bg-s2" />
                {(g.title || g.description) && (
                  <div className="px-3 py-2">
                    {g.title && <div className="text-[0.78rem] font-medium">{g.title}</div>}
                    {g.description && <div className="text-xs text-mu line-clamp-2">{g.description}</div>}
                  </div>
                )}
              </button>
            ))}
          </div>
        ) : <div className="py-16 text-center text-sm text-mu">This project has no images.</div>
      )}

      {d && tab === 'Versions' && (
        <div>
          {!isPack && (
            <div className="flex items-center gap-2 mb-4">
              <Pill active={compatOnly} onClick={() => setCompatOnly(!compatOnly)} icon="check">
                {inst ? `Only versions that fit ${inst.name}` : 'Only compatible versions'}
              </Pill>
              <span className="text-xs text-mu">{versions.length} shown</span>
            </div>
          )}
          {!versions.length && <div className="py-12 text-center text-sm text-mu">No versions to show{compatOnly && !isPack ? '. Turn off the filter to see all of them' : ''}.</div>}
          <div className="space-y-2">
            {versions.map((v) => (
              <div key={v.id} className="rounded-2xl bg-s2 p-4">
                <div className="flex items-center gap-3 flex-wrap">
                  <Pill tone={v.version_type === 'release' ? 'ok' : v.version_type === 'beta' ? 'warn' : 'bad'}>{cap(v.version_type || 'release')}</Pill>
                  <div className="flex-1 min-w-48">
                    <div className="text-[0.875rem] font-medium truncate">{v.name || v.version_number}</div>
                    <div className="text-xs text-mu truncate">{v.version_number} · {day(v.date_published)} · {fmt(v.downloads)} downloads</div>
                  </div>
                  {fits(v) && <Pill tone="ac" icon="check">Fits</Pill>}
                  <Btn v="primary" size="sm" loading={p.busy === v.id} disabled={(!isPack && !inst) || !!p.busy} onClick={() => p.onInstallVersion(v.id)}>Install</Btn>
                  {!cf && <Btn v="ghost" size="sm" onClick={() => setOpen(open === v.id ? '' : v.id)}>{open === v.id ? 'Hide changelog' : 'Changelog'}</Btn>}
                </div>
                <div className="flex gap-1.5 flex-wrap mt-2.5">
                  {v.loaders.map((l) => <Pill key={l}>{cap(l)}</Pill>)}
                  {v.game_versions.slice(0, 5).map((g) => <Pill key={g}>{g}</Pill>)}
                  {v.game_versions.length > 5 && <span className="text-xs text-mu self-center">+{v.game_versions.length - 5} more</span>}
                </div>
                {open === v.id && (
                  <div className="mt-3 pt-3 border-t border-line">
                    {v.changelog && v.changelog.trim() ? <Markdown text={v.changelog} /> : <p className="text-xs text-mu">This version has no changelog.</p>}
                  </div>
                )}
              </div>
            ))}
          </div>
        </div>
      )}

      {zoom && (
        <div className="fixed inset-0 z-[90] bg-black/80 grid place-items-center fade p-6" onMouseDown={() => setZoom(null)}>
          <div onMouseDown={(e) => e.stopPropagation()} className="max-w-[92vw] max-h-[92vh] flex flex-col gap-3 items-center">
            <img src={zoom.url} alt={zoom.title ?? ''} className="max-w-[92vw] max-h-[78vh] object-contain rounded-xl" />
            {(zoom.title || zoom.description) && (
              <div className="text-white text-center max-w-xl">
                {zoom.title && <div className="text-sm font-medium">{zoom.title}</div>}
                {zoom.description && <div className="text-xs text-white/70 mt-1">{zoom.description}</div>}
              </div>
            )}
            <Btn v="secondary" size="sm" onClick={() => setZoom(null)}>Close</Btn>
          </div>
        </div>
      )}
    </div>
  )
}
