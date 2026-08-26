import { FileQuestion, LoaderCircle, X } from 'lucide-react'
import { createElement, useEffect, useState } from 'react'
import { backend, errorMessage } from '../services/backend'
import type { Entry, FilePaneMode, FilePreview } from '../types'
import { fileKind, formatBytes, formatDate } from '../utils/format'
import { iconFor } from '../utils/fileIcons'

interface Props {
  mode: Exclude<FilePaneMode, 'none'>
  entries: Entry[]
  onClose: () => void
}

export function PreviewPane({ mode, entries, onClose }: Props) {
  const entry = entries.length === 1 ? entries[0] : undefined
  const [preview, setPreview] = useState<FilePreview | null>(null)
  const [loading, setLoading] = useState(mode === 'preview' && Boolean(entry && !entry.isDirectory))
  const [failure, setFailure] = useState('')

  useEffect(() => {
    if (mode !== 'preview' || !entry || entry.isDirectory) return
    let cancelled = false
    void backend.preview(entry.fullPath)
      .then(value => { if (!cancelled) setPreview(value) })
      .catch(error => { if (!cancelled) setFailure(errorMessage(error)) })
      .finally(() => { if (!cancelled) setLoading(false) })
    return () => { cancelled = true }
  }, [entry, mode])

  const Icon = entry ? iconFor(entry.extension, entry.isDirectory) : FileQuestion
  const dataUrl = preview?.data && preview.mimeType ? `data:${preview.mimeType};base64,${preview.data}` : ''
  return <aside className="preview-pane">
    <header><strong>{mode === 'preview' ? 'Vorschau' : 'Details'}</strong><button title="Bereich schließen" onClick={onClose}><X /></button></header>
    {!entries.length && <div className="preview-placeholder"><FileQuestion /><span>Datei auswählen</span></div>}
    {entries.length > 1 && <div className="preview-placeholder"><FileQuestion /><span>{entries.length} Einträge ausgewählt</span></div>}
    {entry && <>
      <div className="preview-title">{createElement(Icon, { className: entry.isDirectory ? 'folder-icon' : 'file-icon' })}<strong>{entry.name}</strong></div>
      {mode === 'preview' && <div className="preview-content">
        {entry.isDirectory && <div className="preview-placeholder"><span>Für Ordner ist keine Dateivorschau verfügbar.</span></div>}
        {loading && <div className="preview-placeholder"><LoaderCircle className="spin" /><span>Vorschau wird geladen …</span></div>}
        {failure && <div className="preview-placeholder"><span>{failure}</span></div>}
        {preview?.kind === 'image' && <img src={dataUrl} alt={entry.name} />}
        {preview?.kind === 'text' && <pre>{preview.text}</pre>}
        {preview?.kind === 'pdf' && <iframe src={dataUrl} title={`Vorschau von ${entry.name}`} />}
        {preview?.kind === 'audio' && <audio controls src={dataUrl} />}
        {preview?.kind === 'video' && <video controls src={dataUrl} />}
        {preview?.kind === 'unavailable' && <div className="preview-placeholder"><span>{preview.message}</span></div>}
      </div>}
      <dl className="preview-details">
        <div><dt>Typ</dt><dd>{fileKind(entry.extension, entry.isDirectory)}</dd></div>
        <div><dt>Grösse</dt><dd>{formatBytes(entry.isDirectory ? entry.recursiveSize : entry.size)}</dd></div>
        <div><dt>Geändert</dt><dd>{formatDate(entry.modifiedAt)}</dd></div>
        <div><dt>Erstellt</dt><dd>{formatDate(entry.createdAt)}</dd></div>
        <div><dt>Pfad</dt><dd title={entry.fullPath}>{entry.fullPath}</dd></div>
        {entry.readOnly && <div><dt>Attribut</dt><dd>Schreibgeschützt</dd></div>}
      </dl>
    </>}
  </aside>
}
