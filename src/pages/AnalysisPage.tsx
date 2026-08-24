import { BarChart3, File, Folder, HardDrive, LoaderCircle } from 'lucide-react'
import { useEffect, useState } from 'react'
import { backend, errorMessage } from '../services/backend'
import type { StorageAnalysis, Volume } from '../types'
import { formatBytes, formatCount } from '../utils/format'

export function AnalysisPage({ volumes, initialPath, onNavigate, onError }: { volumes: Volume[]; initialPath: string; onNavigate: (path: string) => void; onError: (message: string) => void }) {
  const ready = volumes.filter(volume => volume.indexStatus === 'Ready')
  const [path, setPath] = useState(initialPath || ready[0]?.rootPath || '')
  const [analysis, setAnalysis] = useState<StorageAnalysis | null>(null)
  const [loading, setLoading] = useState(false)
  useEffect(() => { if (!path && ready[0]) setPath(ready[0].rootPath) }, [path, ready])
  useEffect(() => {
    if (!path) return
    setLoading(true); backend.analyze(path).then(setAnalysis).catch(error => onError(errorMessage(error))).finally(() => setLoading(false))
  }, [path]) // eslint-disable-line react-hooks/exhaustive-deps
  return <section className="page analysis-page">
    <div className="page-heading"><div><h1><BarChart3 />Speicheranalyse</h1><p>Voraggregierte Werte direkt aus dem Index</p></div><select value={path} onChange={event => setPath(event.target.value)}>{ready.map(volume => <option key={volume.id}>{volume.rootPath}</option>)}</select></div>
    {!ready.length ? <div className="notice warning">Indexiere zuerst ein Laufwerk.</div> : analysis && <>
      <div className="metric-grid"><Metric icon={HardDrive} label="Gesamtgrösse" value={formatBytes(analysis.totalBytes)} /><Metric icon={File} label="Dateien" value={formatCount(analysis.fileCount)} /><Metric icon={Folder} label="Ordner" value={formatCount(analysis.folderCount)} /></div>
      <div className="analysis-grid"><section className="panel"><h2>Grösste Ordner</h2>{analysis.largestFolders.map(entry => <button className="rank-row" key={entry.id} onClick={() => onNavigate(entry.fullPath)}><span><Folder />{entry.name}<small>{entry.fullPath}</small></span><strong>{formatBytes(entry.recursiveSize)}</strong></button>)}</section>
      <section className="panel"><h2>Grösste Dateien</h2>{analysis.largestFiles.map(entry => <button className="rank-row" key={entry.id} onClick={() => void backend.open(entry.fullPath)}><span><File />{entry.name}<small>{entry.fullPath}</small></span><strong>{formatBytes(entry.size)}</strong></button>)}</section></div>
      <section className="panel extension-panel"><h2>Dateitypen nach Speicherverbrauch</h2>{analysis.extensions.map(item => <div className="extension-row" key={item.extension}><strong>.{item.extension}</strong><div><i style={{ width: `${Math.max(2, (item.bytes / (analysis.extensions[0]?.bytes || 1)) * 100)}%` }} /></div><span>{formatBytes(item.bytes)} · {formatCount(item.count)}</span></div>)}</section>
    </>}
    {loading && <div className="loading-overlay"><LoaderCircle className="spin" />Analysiere Index …</div>}
  </section>
}

function Metric({ icon: Icon, label, value }: { icon: typeof HardDrive; label: string; value: string }) { return <div className="metric"><Icon /><div><span>{label}</span><strong>{value}</strong></div></div> }
