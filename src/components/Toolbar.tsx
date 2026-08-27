import { ArrowLeft, ArrowRight, ArrowUp, PanelsTopLeft, RefreshCw, Search, Settings } from 'lucide-react'
import { useEffect, useState } from 'react'

interface Props {
  path: string
  query: string
  canBack: boolean
  canForward: boolean
  canUp: boolean
  searchRef: React.RefObject<HTMLInputElement | null>
  onBack: () => void
  onForward: () => void
  onUp: () => void
  onPath: (path: string) => void
  onQuery: (query: string) => void
  onSearchFocus: () => void
  onRefresh: () => void
  onNewWindow: () => void
  onSettings: () => void
}

export function Toolbar(props: Props) {
  const [editingPath, setEditingPath] = useState(props.path)
  useEffect(() => setEditingPath(props.path), [props.path])
  return <header className="toolbar">
    <div className="nav-controls">
      <button title="Zurück (Alt+Links)" disabled={!props.canBack} onClick={props.onBack}><ArrowLeft /></button>
      <button title="Vorwärts (Alt+Rechts)" disabled={!props.canForward} onClick={props.onForward}><ArrowRight /></button>
      <button title="Übergeordnet (Alt+Hoch)" disabled={!props.canUp} onClick={props.onUp}><ArrowUp /></button>
      <button title="Aktualisieren" disabled={!props.path} onClick={props.onRefresh}><RefreshCw /></button>
      <button title="Neues Fenster (Ctrl+N)" disabled={!props.path} onClick={props.onNewWindow}><PanelsTopLeft /></button>
    </div>
    <form className="path-field" onSubmit={event => { event.preventDefault(); props.onPath(editingPath) }}>
      <input aria-label="Pfad" value={editingPath} onChange={event => setEditingPath(event.target.value)} />
    </form>
    <div className="toolbar-end"><div className="search-field"><Search size={17} /><input ref={props.searchRef} aria-label="Index durchsuchen" placeholder="Index durchsuchen …" value={props.query} onFocus={props.onSearchFocus} onChange={event => props.onQuery(event.target.value)} /></div><button className="settings-button" title="Einstellungen" aria-label="Einstellungen öffnen" onClick={props.onSettings}><Settings /></button></div>
  </header>
}
