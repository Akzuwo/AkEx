import { Folder, Plus, X } from 'lucide-react'

interface TabBarProps {
  tabs: { id: string; path: string }[]
  activeTabId: string
  onActivate: (id: string) => void
  onClose: (id: string) => void
  onNew: () => void
}

function tabTitle(path: string) {
  if (!path) return 'Neuer Tab'
  const trimmed = path.replace(/[\\/]+$/, '')
  return trimmed.split(/[\\/]/).pop() || path
}

export function TabBar({ tabs, activeTabId, onActivate, onClose, onNew }: TabBarProps) {
  return <div className="tab-bar">
    <div className="tab-strip" role="tablist" aria-label="Geöffnete Ordner">
      {tabs.map(tab => <div
        className={`tab-item ${tab.id === activeTabId ? 'active' : ''}`}
        key={tab.id}
        role="tab"
        aria-selected={tab.id === activeTabId}
        tabIndex={tab.id === activeTabId ? 0 : -1}
        title={tab.path || 'Neuer Tab'}
        onClick={() => onActivate(tab.id)}
        onKeyDown={event => {
          if (event.key === 'Enter' || event.key === ' ') onActivate(tab.id)
        }}
        onAuxClick={event => { if (event.button === 1) onClose(tab.id) }}
      >
        <Folder size={15} />
        <span>{tabTitle(tab.path)}</span>
        <button aria-label={`${tabTitle(tab.path)} schließen`} onClick={event => { event.stopPropagation(); onClose(tab.id) }}>
          <X size={14} />
        </button>
      </div>)}
    </div>
    <button className="new-tab-button" aria-label="Neuer Tab" title="Neuer Tab (Strg+T)" onClick={onNew}>
      <Plus size={17} />
    </button>
  </div>
}
