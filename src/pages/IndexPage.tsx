import { CircleCheck, Database, LoaderCircle, RefreshCw, ShieldCheck, Square } from 'lucide-react'
import { listen } from '@tauri-apps/api/event'
import { useEffect, useState } from 'react'
import { backend, errorMessage } from '../services/backend'
import type { ScanProgress, Volume } from '../types'
import { formatBytes, formatCount, formatDate } from '../utils/format'

export function IndexPage({ volumes, onChanged, onError, embedded = false }: { volumes: Volume[]; onChanged: () => void; onError: (message: string) => void; embedded?: boolean }) {
  const [scans, setScans] = useState<Record<string, ScanProgress>>({})
  useEffect(() => {
    const disposers = [
      listen<ScanProgress>('index:progress', event => setScans(previous => ({ ...previous, [event.payload.rootPath]: event.payload }))),
      listen<ScanProgress>('index:complete', event => { setScans(previous => ({ ...previous, [event.payload.rootPath]: event.payload })); onChanged(); void backend.startWatchers() }),
      listen<{ scanId: string; message: string }>('index:error', event => { onError(event.payload.message); onChanged() }),
      listen<{ scanId: string }>('index:cancelled', () => onChanged()),
    ]
    return () => { void Promise.all(disposers).then(values => values.forEach(dispose => dispose())) }
  }, []) // eslint-disable-line react-hooks/exhaustive-deps
  async function start(volume: Volume) {
    if (volume.indexStatus === 'Ready' && !window.confirm(`Index für ${volume.rootPath} vollständig neu aufbauen?`)) return
    try {
      const scanId = await backend.startIndex(volume.rootPath)
      setScans(previous => ({ ...previous, [volume.rootPath]: { scanId, rootPath: volume.rootPath, entriesFound: 0, bytesFound: 0, currentPath: volume.rootPath, phase: 'Scanning', errors: 0 } }))
      onChanged()
    } catch (error) { onError(errorMessage(error)) }
  }
  async function verify(volume: Volume) {
    try { const result = await backend.verifyIndex(volume.id); window.alert(result.ok ? `Index ${volume.rootPath} ist konsistent.` : `Probleme gefunden\nOrphans: ${result.orphanCount}\nGrössenabweichungen: ${result.sizeMismatchCount}\nSQLite: ${result.integrityMessage}`) }
    catch (error) { onError(errorMessage(error)) }
  }
  return <section className={embedded ? 'settings-index' : 'page index-page'}><div className="page-heading"><div><h1><Database />Index-Verwaltung</h1><p>Laufwerke auswählen, Zustand prüfen und Index reparieren</p></div></div>
    <div className="index-list">{volumes.map(volume => {
      const scan = scans[volume.rootPath]
      const active = scan?.phase === 'Scanning' || volume.indexStatus === 'Indexing'
      return <article className="index-card" key={volume.id}><div className="index-card-top"><div className="drive-badge">{volume.rootPath.slice(0, 2)}</div><div><h2>{volume.label || `Laufwerk ${volume.rootPath}`}</h2><p>{volume.filesystemType || 'Unbekannt'} · {formatBytes(volume.totalBytes)}</p></div><span className={`status-pill ${volume.indexStatus.toLocaleLowerCase()}`}>{active && <LoaderCircle className="spin" />}{volume.indexStatus}</span></div>
        {active && scan ? <div className="scan-progress"><div><strong>{formatCount(scan.entriesFound)} Einträge gefunden</strong><span>{scan.percent?.toFixed(0) ?? '…'} %</span></div><progress max={100} value={scan.percent ?? undefined} /><p title={scan.currentPath}>{scan.currentPath}</p><small>{formatBytes(scan.bytesFound)} · {scan.errors} übersprungene Fehler</small></div> : <div className="index-facts"><span><strong>{formatCount(volume.entryCount)}</strong>Einträge</span><span><strong>{formatDate(volume.lastFullScan)}</strong>Letzter Vollscan</span><span><strong>{formatBytes((volume.totalBytes ?? 0) - (volume.freeBytes ?? 0))}</strong>Belegt</span></div>}
        {volume.lastError && <div className="inline-error">{volume.lastError}</div>}
        <div className="card-actions">{active && scan ? <button className="danger-button" onClick={() => void backend.cancelIndex(scan.scanId)}><Square />Abbrechen</button> : <button className="primary-button" onClick={() => void start(volume)}>{volume.indexStatus === 'Ready' ? <RefreshCw /> : <Database />}{volume.indexStatus === 'Ready' ? 'Neu aufbauen' : 'Indexieren'}</button>}<button disabled={volume.indexStatus !== 'Ready'} onClick={() => void verify(volume)}><ShieldCheck />Index überprüfen</button>{volume.indexStatus === 'Ready' && <span className="ready-note"><CircleCheck />Live-Index bereit</span>}</div>
      </article>
    })}</div>
  </section>
}
