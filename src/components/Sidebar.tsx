import { BarChart3, HardDrive, Monitor, Search } from 'lucide-react'
import type { View, Volume } from '../types'
import { formatBytes } from '../utils/format'

interface Props {
  volumes: Volume[]
  activeView: View
  currentPath: string
  onView: (view: View) => void
  onVolume: (path: string) => void
}

export function Sidebar({ volumes, activeView, currentPath, onView, onVolume }: Props) {
  return <aside className="sidebar">
    <nav>
      <div className="nav-label">Schnellzugriff</div>
      <button className={activeView === 'browser' ? 'active' : ''} onClick={() => onView('browser')}><Monitor size={17} />Dieser PC</button>
      <div className="nav-label nav-label-spaced">Laufwerke</div>
      {volumes.map(volume => <button key={volume.id} className={currentPath === volume.rootPath && activeView === 'browser' ? 'active' : ''} onClick={() => onVolume(volume.rootPath)}>
        <HardDrive size={17} />
        <span className="drive-label"><span>{volume.label || volume.rootPath}</span><small>{volume.totalBytes ? formatBytes(volume.totalBytes - (volume.freeBytes ?? 0)) : volume.indexStatus}</small></span>
        <i className={`status-dot ${volume.indexStatus.toLocaleLowerCase()}`} />
      </button>)}
      <div className="nav-label nav-label-spaced">Werkzeuge</div>
      <button className={activeView === 'search' ? 'active' : ''} onClick={() => onView('search')}><Search size={17} />Suche</button>
      <button className={activeView === 'analysis' ? 'active' : ''} onClick={() => onView('analysis')}><BarChart3 size={17} />Speicheranalyse</button>
    </nav>
    <div className="sidebar-footer"><span className="status-dot ready" />Index-First Engine</div>
  </aside>
}
