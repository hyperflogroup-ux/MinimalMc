import { useRef, useState } from 'react'
import { Btn, Modal, useApp } from './ui'
import { backend, type ModNode } from './backend'
import type { Inst } from './data'

// Mod Bisect finds the mod behind a crash (or any problem that happens every time) by turning
// mods off in halves and asking whether the problem is still there. A mod's own required mods
// are always switched along with it, so a missing library never causes a different crash.
//
// Which mods were switched is written down in case the app closes halfway, so they can be turned
// back on later from the Mods tab.

const KEY = (id: number) => `minimalmc.bisect.${id}`

export function savedBisect(id: number): string[] | null {
  try {
    const t = localStorage.getItem(KEY(id))
    return t ? (JSON.parse(t) as string[]) : null
  } catch { return null }
}
const saveBisect = (id: number, files: string[]) => { try { localStorage.setItem(KEY(id), JSON.stringify(files)) } catch { /* storage unavailable */ } }
const clearBisect = (id: number) => { try { localStorage.removeItem(KEY(id)) } catch { /* storage unavailable */ } }
const changed = () => window.dispatchEvent(new Event('minimalmc-mods'))

/** Turns every mod that a bisect switched off back on. */
export async function restoreBisect(id: number): Promise<void> {
  const files = savedBisect(id)
  if (!files) return
  for (const f of files) {
    try { await backend.setModEnabled(id, f + '.disabled', true) } catch { /* it was already on */ }
  }
  clearBisect(id)
  changed()
}

type Phase = 'intro' | 'working' | 'ask' | 'done' | 'fail'
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms))
const half = (list: string[]) => list.slice(0, Math.ceil(list.length / 2))

export function BisectDialog({ inst, onClose }: { inst: Inst; onClose: () => void }) {
  const app = useApp()
  const [phase, setPhase] = useState<Phase>('intro')
  const [msg, setMsg] = useState('')
  const [round, setRound] = useState(0)
  const [left, setLeft] = useState(0)
  const [crashed, setCrashed] = useState(false)
  const [culprit, setCulprit] = useState<ModNode | null>(null)
  const nodes = useRef<ModNode[]>([])
  const on = useRef<Map<string, boolean>>(new Map())
  const tested = useRef<string[]>([])
  const suspects = useRef<string[]>([])

  /** The given mods plus everything they need, directly or indirectly. */
  const closure = (roots: string[]): Set<string> => {
    const byFile = new Map(nodes.current.map((n) => [n.file, n] as [string, ModNode]))
    const providers = new Map<string, string[]>()
    nodes.current.forEach((n) => n.provides.forEach((p) => {
      const k = p.toLowerCase()
      providers.set(k, [...(providers.get(k) ?? []), n.file])
    }))
    const out = new Set(roots)
    const stack = [...roots]
    while (stack.length) {
      const n = byFile.get(stack.pop() as string)
      if (!n) continue
      for (const r of n.requires) {
        for (const f of providers.get(r.toLowerCase()) ?? []) {
          if (!out.has(f)) { out.add(f); stack.push(f) }
        }
      }
    }
    return out
  }

  const setMods = async (enabled: Set<string>) => {
    for (const n of nodes.current) {
      const want = enabled.has(n.file)
      if (want !== (on.current.get(n.file) ?? true)) {
        await backend.setModEnabled(inst.id, want ? n.file + '.disabled' : n.file, want)
        on.current.set(n.file, want)
      }
    }
  }

  /** Puts every mod back, except one that should stay off. */
  const restoreAll = async (keepOff?: string) => {
    const keep = new Set(nodes.current.map((n) => n.file).filter((f) => f !== keepOff))
    let ok = true
    try { await setMods(keep) } catch { ok = false }
    // If something went wrong, the saved record stays so the Mods tab can finish the job.
    if (ok) clearBisect(inst.id)
    changed()
  }

  const fail = (e: unknown) => { setMsg(e instanceof Error ? e.message : String(e)); setPhase('fail') }

  const runRound = async (test: string[], all: string[]) => {
    tested.current = test
    suspects.current = all
    setRound((n) => n + 1)
    setLeft(all.length)
    setPhase('working')
    setMsg('Setting up your mods…')
    try {
      await setMods(closure(test))
      changed()
      setMsg('Launching Minecraft…')
      await backend.launchInstance(inst.id, app.account)
      let s = await backend.launchStatus(inst.id)
      while (s.busy || s.running) {
        setMsg(s.running ? 'Minecraft is running. Look for the problem, then close the game.' : `${s.stage}…`)
        await sleep(900)
        s = await backend.launchStatus(inst.id)
      }
      if (s.error && !s.error.startsWith('Minecraft closed unexpectedly')) throw new Error(s.error)
      setCrashed(!!s.error)
      setPhase('ask')
    } catch (e) { fail(e) }
  }

  const start = async () => {
    if (!app.account) { setMsg('Choose a player name in Settings, under Accounts, first.'); setPhase('fail'); return }
    setPhase('working')
    setMsg('Reading your mods…')
    try {
      const list = await backend.modGraph(inst.id)
      nodes.current = list
      // Mods that no other mod needs. Needed mods (libraries) simply follow along.
      const needed = (n: ModNode) =>
        list.some((m) => m !== n && m.requires.some((r) => n.provides.some((p) => p.toLowerCase() === r.toLowerCase())))
      const roots = list.filter((n) => !needed(n)).map((n) => n.file)
      if (roots.length < 2) {
        setMsg('There are too few separate mods to narrow down.')
        setPhase('fail')
        return
      }
      on.current = new Map(list.map((n) => [n.file, true] as [string, boolean]))
      saveBisect(inst.id, list.map((n) => n.file))
      await runRound(half(roots), roots)
    } catch (e) { fail(e) }
  }

  const answer = async (problem: boolean) => {
    const test = tested.current
    const all = suspects.current
    const next = problem ? test : all.filter((f) => !test.includes(f))
    if (next.length <= 1) {
      setCulprit(nodes.current.find((m) => m.file === next[0]) ?? null)
      setPhase('done')
      return
    }
    await runRound(half(next), next)
  }

  const cancel = async () => {
    if (nodes.current.length) await restoreAll()
    onClose()
  }
  const finish = async (keepOff: boolean) => {
    await restoreAll(keepOff && culprit ? culprit.file : undefined)
    app.notify(keepOff && culprit ? `${culprit.name} is turned off. Everything else is back on.` : 'All your mods are back on.')
    onClose()
  }

  return (
    <Modal onClose={cancel} w={520}>
      <div className="p-7">
        <h2 className="text-[1.375rem] font-semibold tracking-tight">Find the broken mod</h2>

        {phase === 'intro' && (
          <>
            <p className="text-[0.8125rem] text-mu mt-2 leading-relaxed">
              This turns your mods off in halves and launches the game each time. After every launch you say whether the problem is still there, and it narrows down to the one causing it. It works best for a crash or a problem that happens every time.
            </p>
            <p className="text-[0.8125rem] text-mu mt-2 leading-relaxed">Your mods are all turned back on at the end. It usually takes 4 to 6 launches.</p>
            <div className="flex justify-end gap-2 mt-6">
              <Btn v="ghost" onClick={cancel}>Cancel</Btn>
              <Btn v="primary" onClick={start}>Start</Btn>
            </div>
          </>
        )}

        {phase === 'working' && (
          <>
            <div className="text-[0.75rem] uppercase tracking-[.12em] text-mu2 font-medium mt-3">Round {round}</div>
            <p className="text-[0.9375rem] mt-2 leading-relaxed">{msg}</p>
            <p className="text-xs text-mu mt-3">{left} possible {left === 1 ? 'cause' : 'causes'} left to check</p>
          </>
        )}

        {phase === 'ask' && (
          <>
            <div className="text-[0.75rem] uppercase tracking-[.12em] text-mu2 font-medium mt-3">Round {round}</div>
            <p className="text-[0.9375rem] mt-2 leading-relaxed">
              The game {crashed ? 'crashed' : 'closed normally'}. Did the problem happen this time?
            </p>
            <div className="flex gap-2 mt-6">
              <Btn v="primary" onClick={() => answer(true)}>Yes, it happened</Btn>
              <Btn v="secondary" onClick={() => answer(false)}>No, it was fine</Btn>
            </div>
            <p className="text-xs text-mu mt-3">{left} possible {left === 1 ? 'cause' : 'causes'} before this answer</p>
          </>
        )}

        {phase === 'done' && (
          <>
            <p className="text-[0.9375rem] mt-3 leading-relaxed">
              {culprit ? <>Most likely cause: <span className="font-semibold">{culprit.name}</span></> : 'No single mod could be pinned down.'}
            </p>
            {culprit && <p className="text-xs text-mu mt-1 font-mono break-all">{culprit.file}</p>}
            <p className="text-[0.8125rem] text-mu mt-3 leading-relaxed">
              If this mod only works together with others, one of those could be involved too. Turn it off and launch once to confirm.
            </p>
            <div className="flex justify-end gap-2 mt-6 flex-wrap">
              <Btn v="secondary" onClick={() => finish(false)}>Turn everything back on</Btn>
              {culprit && <Btn v="primary" onClick={() => finish(true)}>Keep it turned off</Btn>}
            </div>
          </>
        )}

        {phase === 'fail' && (
          <>
            <p className="text-[0.8125rem] mt-3 leading-relaxed break-words">{msg}</p>
            <div className="flex justify-end mt-6">
              <Btn v="primary" onClick={cancel}>Restore my mods and close</Btn>
            </div>
          </>
        )}
      </div>
    </Modal>
  )
}
