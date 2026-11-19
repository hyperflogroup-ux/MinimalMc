import { useEffect, useRef, useState } from 'react'
import { Btn, Icon, Input, Pill, Seg, Select, Tip, useApp } from './ui'
import { backend, isDesktop, releaseVersions, type JavaEntry } from './backend'
import { LOADERS } from './data'

// The setup guide shown the first time the app opens (and from Settings, under About). Every step
// can be skipped, and everything chosen here can be changed later in Settings.

const ACCENTS = ['white', 'red', 'orange', 'yellow', 'green', 'cyan', 'blue', 'indigo', 'violet', 'pink']
const STEPS = ['Welcome', 'Look', 'Java', 'Player', 'Skin', 'Instance', 'CurseForge', 'Done']
const TITLES = [
  'Welcome to MinimalMc',
  'Pick your look',
  'Java',
  'Your player name',
  'Your skin',
  'Your first instance',
  'CurseForge (optional)',
  'You’re all set',
]
const errText = (e: unknown) => (e instanceof Error ? e.message : String(e))

export function Onboarding({ onClose }: { onClose: () => void }) {
  const app = useApp()
  const [step, setStep] = useState(0)

  // Java
  const [javas, setJavas] = useState<JavaEntry[] | null>(null)
  // Player
  const [name, setName] = useState<string>(app.account || '')
  const nameOk = /^[A-Za-z0-9_]{3,16}$/.test(name)
  // Skin
  const fileInput = useRef<HTMLInputElement>(null)
  const [skin, setSkin] = useState('')
  const [skinErr, setSkinErr] = useState('')
  const [skinBusy, setSkinBusy] = useState(false)
  // Instance
  const [instName, setInstName] = useState('My first instance')
  const [versions, setVersions] = useState<string[]>([])
  const [mc, setMc] = useState('')
  const [loader, setLoader] = useState('Fabric')
  const [creating, setCreating] = useState(false)
  const [created, setCreated] = useState('')
  const [instErr, setInstErr] = useState('')
  // CurseForge
  const [cfKey, setCfKey] = useState('')
  const [cfMsg, setCfMsg] = useState('')
  const [cfBad, setCfBad] = useState(false)
  const [cfBusy, setCfBusy] = useState(false)
  const [cfSaved, setCfSaved] = useState(false)

  useEffect(() => {
    if (!isDesktop()) return
    if (step === 2 && javas === null) backend.listJava().then(setJavas).catch(() => setJavas([]))
    if (step === 5 && !versions.length) {
      releaseVersions().then((l) => { setVersions(l); setMc(l[0] ?? '') }).catch(() => {})
    }
    if (step === 6) backend.curseforgeStatus().then((s) => setCfSaved(s.saved)).catch(() => {})
  }, [step])

  const finish = () => onClose()

  const chooseSkin = async (file: File | undefined) => {
    if (!file) return
    setSkinBusy(true)
    setSkinErr('')
    try {
      const saved = await backend.saveSkin(file)
      await backend.setActiveSkin(saved)
      setSkin(saved)
    } catch (e) { setSkinErr(errText(e)) } finally { setSkinBusy(false) }
  }

  const createInstance = async () => {
    setCreating(true)
    setInstErr('')
    try {
      await app.addInstance({ name: instName.trim(), group: 'Other', mc, loader, ram: 4 })
      setCreated(instName.trim())
    } catch (e) { setInstErr(errText(e)) } finally { setCreating(false) }
  }

  const saveKey = async () => {
    setCfBusy(true)
    setCfMsg('')
    try {
      await backend.curseforgeSetKey(cfKey)
      setCfKey('')
      setCfMsg(await backend.curseforgeTest())
      setCfBad(false)
      setCfSaved(true)
    } catch (e) { setCfBad(true); setCfMsg(errText(e)) } finally { setCfBusy(false) }
  }

  const next = () => {
    // The player name is saved when moving on, if one was entered.
    if (step === 3 && name.trim() && nameOk) app.setAccount(name.trim())
    setStep((s) => Math.min(s + 1, STEPS.length - 1))
  }
  const nextBlocked = (step === 3 && name.trim() !== '' && !nameOk) || (step === 5 && creating)
  const p = app.prefs

  return (
    <div className="fixed inset-0 z-[100] bg-bg text-fg flex items-center justify-center p-6 fade overflow-auto">
      <div className="w-full max-w-xl">
        <div className="flex gap-1.5 mb-8">
          {STEPS.map((x, k) => <div key={x} className={`h-1 flex-1 rounded-full transition ${k <= step ? 'bg-ac' : 'bg-fg/10'}`} />)}
        </div>
        <div className="text-[0.6875rem] uppercase tracking-[.12em] text-mu2 font-medium">Step {step + 1} of {STEPS.length}</div>
        <h1 className="text-[1.75rem] font-semibold tracking-tight mt-1">{TITLES[step]}</h1>

        <div className="mt-6 min-h-[270px] text-[0.875rem] leading-relaxed" key={step}>
          {step === 0 && (
            <div className="fade space-y-3 text-mu">
              <p>A few quick choices to get you playing. Every step can be skipped, and everything can be changed later in Settings.</p>
              <p>It takes about a minute: your look, Java, a player name, a skin, your first instance, and an optional CurseForge key.</p>
            </div>
          )}

          {step === 1 && (
            <div className="fade space-y-6">
              <div>
                <div className="text-xs text-mu mb-2">Theme</div>
                <Seg value={p.theme} onChange={(v: string) => app.setPref('theme', v)} items={[{ id: 'light', label: 'Light' }, { id: 'dark', label: 'Dark' }, { id: 'system', label: 'System' }]} />
              </div>
              <div>
                <div className="text-xs text-mu mb-3">Accent color</div>
                <div className="flex gap-2.5 flex-wrap">
                  {ACCENTS.map((a) => (
                    <Tip key={a} label={a[0].toUpperCase() + a.slice(1)}>
                      <button onClick={() => app.setPref('accent', a)} aria-label={a} data-accent={a} className={`size-9 rounded-full bg-ac grid place-items-center transition hover:scale-110 ${p.accent === a ? 'ring-2 ring-offset-2 ring-offset-bg ring-fg' : ''}`}>
                        {p.accent === a && <Icon n="check" size={14} className="text-acfg" />}
                      </button>
                    </Tip>
                  ))}
                </div>
              </div>
            </div>
          )}

          {step === 2 && (
            <div className="fade space-y-4">
              <p className="text-mu">MinimalMc picks the Java each Minecraft version needs and downloads it for you the first time it's missing. There is nothing to install.</p>
              {javas === null && isDesktop() && <div className="text-mu">Looking for Java on this computer…</div>}
              {javas && javas.length > 0 && (
                <div className="space-y-2">
                  <div className="text-xs text-mu">Found on this computer</div>
                  {javas.slice(0, 4).map((j) => (
                    <div key={j.path} className="rounded-2xl bg-s2 px-4 py-3 flex items-center gap-2">
                      <span className="font-medium">Java {j.version}</span>
                      {j.managed && <Pill tone="ac">Managed by MinimalMc</Pill>}
                    </div>
                  ))}
                </div>
              )}
              {javas && javas.length === 0 && <div className="rounded-2xl bg-s2 px-4 py-3 text-mu">No Java found. The right one will be downloaded when you first launch an instance.</div>}
            </div>
          )}

          {step === 3 && (
            <div className="fade space-y-4">
              <p className="text-mu">This is the name your player has in the game. It works offline, so there is no account to sign in to. It can't be used to join servers that check Minecraft accounts.</p>
              <Input autoFocus placeholder="Player name" value={name} onChange={(e: any) => setName(e.target.value)} className="max-w-xs" />
              {name.trim() !== '' && !nameOk && <div className="text-xs text-red-500">3 to 16 letters, numbers or underscores.</div>}
              {name.trim() === '' && <div className="text-xs text-mu">You can skip this now, but you'll need a name before you can play.</div>}
            </div>
          )}

          {step === 4 && (
            <div className="fade space-y-4">
              <p className="text-mu">Offline accounts can't download skins from Minecraft, so MinimalMc applies yours to the game itself. Choose a skin image (a .png file, 64 by 64).</p>
              <input ref={fileInput} type="file" accept="image/png" className="hidden" onChange={(e) => { chooseSkin(e.target.files?.[0]); e.target.value = '' }} />
              <Btn v="secondary" icon="upload" loading={skinBusy} onClick={() => fileInput.current?.click()}>Choose a skin image</Btn>
              {skin && <div className="flex items-center gap-2"><Pill tone="ok" dot>Using “{skin}”</Pill></div>}
              {skinErr && <div className="text-xs text-red-500 break-words">{skinErr}</div>}
              <p className="text-xs text-mu">Skins you add later live on the Skins page.</p>
            </div>
          )}

          {step === 5 && (
            <div className="fade space-y-4">
              {created ? (
                <div className="rounded-2xl bg-s2 px-4 py-4"><Pill tone="ok" dot>Created</Pill><div className="font-medium mt-2">{created}</div><div className="text-xs text-mu mt-1">Find it on the Instances page. Press Play there when you're ready.</div></div>
              ) : (
                <>
                  <p className="text-mu">An instance is one copy of Minecraft with its own mods and worlds. Make your first one now, or skip and do it later.</p>
                  <Input placeholder="Instance name" value={instName} onChange={(e: any) => setInstName(e.target.value)} />
                  <div className="flex gap-3 flex-wrap items-center">
                    <div className="w-40">{versions.length ? <Select value={mc} onChange={setMc} options={versions} className="w-full" /> : <span className="text-mu text-xs">Loading versions…</span>}</div>
                    <div className="w-40"><Select value={loader} onChange={setLoader} options={LOADERS} className="w-full" /></div>
                  </div>
                  <Btn v="primary" icon="plus" loading={creating} disabled={!instName.trim() || !mc} onClick={createInstance}>Create instance</Btn>
                  {instErr && <div className="text-xs text-red-500 break-words">{instErr}</div>}
                </>
              )}
            </div>
          )}

          {step === 6 && (
            <div className="fade space-y-4">
              <p className="text-mu">Modrinth works without anything. To also browse CurseForge, paste your CurseForge API key. It stays on this computer and is only sent to CurseForge.</p>
              <div className="flex gap-2">
                <Input type="password" placeholder={cfSaved ? 'A key is saved. Paste a new one to replace it' : 'Paste your API key'} value={cfKey} onChange={(e: any) => setCfKey(e.target.value)} className="flex-1" />
                <Btn v="primary" loading={cfBusy} disabled={!cfKey.trim()} onClick={saveKey}>Save and test</Btn>
              </div>
              {cfMsg && <div className={`text-xs break-words ${cfBad ? 'text-red-500' : 'text-mu'}`}>{cfMsg}</div>}
              <Btn v="ghost" size="sm" onClick={() => backend.openUrl('https://console.curseforge.com/').catch(() => {})}>Open CurseForge's developer site</Btn>
            </div>
          )}

          {step === 7 && (
            <div className="fade space-y-3">
              <p className="text-mu">Here's what's set up. You can change any of it in Settings.</p>
              <div className="rounded-2xl bg-s2 p-4 space-y-2">
                <div className="flex justify-between"><span className="text-mu">Player name</span><span className="font-medium">{app.account || 'Not set yet'}</span></div>
                <div className="flex justify-between"><span className="text-mu">Skin</span><span className="font-medium">{skin || 'Not chosen'}</span></div>
                <div className="flex justify-between"><span className="text-mu">First instance</span><span className="font-medium">{created || 'Not created'}</span></div>
                <div className="flex justify-between"><span className="text-mu">CurseForge</span><span className="font-medium">{cfSaved ? 'Key saved' : 'No key'}</span></div>
              </div>
            </div>
          )}
        </div>

        <div className="flex items-center gap-2 mt-8">
          {step < STEPS.length - 1 && <Btn v="ghost" onClick={finish}>Skip setup</Btn>}
          <div className="flex-1" />
          {step > 0 && <Btn v="secondary" onClick={() => setStep((s) => s - 1)}>Back</Btn>}
          {step < STEPS.length - 1
            ? <Btn v="primary" disabled={nextBlocked} onClick={next}>{step === 0 ? 'Let’s go' : 'Next'}</Btn>
            : <Btn v="primary" icon="check" onClick={finish}>Start using MinimalMc</Btn>}
        </div>
      </div>
    </div>
  )
}
