import { Database, HardDrive, LoaderCircle } from 'lucide-react'
import { useEffect, useState } from 'react'
import type { Volume } from '../types'
import { formatBytes } from '../utils/format'

interface Props {
  open: boolean
  volumes: Volume[]
  onStart: (rootPaths: string[]) => Promise<void>
  onSkip: () => void
}

export function FirstRunModal({ open, volumes, onStart, onSkip }: Props) {
  const [selected, setSelected] = useState<Set<string>>(new Set())
  const [starting, setStarting] = useState(false)
  useEffect(() => {
    if (open) setSelected(new Set(volumes.map(volume => volume.rootPath)))
  }, [open, volumes])
  if (!open) return null

  async function start() {
    setStarting(true)
    try { await onStart([...selected]) }
    finally { setStarting(false) }
  }

  return <div className="modal-backdrop onboarding-backdrop">
    <section className="modal onboarding-modal" role="dialog" aria-modal="true" aria-labelledby="onboarding-title">
      <div className="onboarding-hero"><span><Database /></span><div><p>Willkommen bei Akex</p><h1 id="onboarding-title">Welche Datenträger sollen indexiert werden?</h1><small>Die Indexierung ermöglicht schnelle Suche, Ordnergrössen und Speicheranalysen. Du kannst diese Auswahl später in den Einstellungen ändern.</small></div></div>
      <div className="drive-picker">
        {volumes.map(volume => <label className={selected.has(volume.rootPath) ? 'selected' : ''} key={volume.id}>
          <input type="checkbox" checked={selected.has(volume.rootPath)} onChange={event => setSelected(previous => { const next = new Set(previous); if (event.target.checked) next.add(volume.rootPath); else next.delete(volume.rootPath); return next })} />
          <span className="drive-picker-icon"><HardDrive /></span><span className="drive-picker-copy"><strong>{volume.label || `Laufwerk ${volume.rootPath}`}</strong><small>{volume.rootPath} · {volume.filesystemType || 'Dateisystem unbekannt'} · {formatBytes(volume.totalBytes)}</small></span>
        </label>)}
      </div>
      <footer className="modal-actions"><button className="secondary-button" disabled={starting} onClick={onSkip}>Später</button><button className="primary-button" disabled={starting || selected.size === 0} onClick={() => void start()}>{starting ? <LoaderCircle className="spin" /> : <Database />}{starting ? 'Wird gestartet …' : `${selected.size} Datenträger indexieren`}</button></footer>
    </section>
  </div>
}
