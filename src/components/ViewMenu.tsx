import { ArrowDown, ArrowUp, Check, ChevronDown, Eye, Grid2X2, LayoutList, List, Monitor, PanelRight, Rows3 } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import type { EntrySortField, FilePaneMode, FileViewMode, SortDirection } from '../types'

interface Props {
  viewMode: FileViewMode
  paneMode: FilePaneMode
  onViewMode: (mode: FileViewMode) => void
  onPaneMode: (mode: FilePaneMode) => void
  sortField: EntrySortField
  sortDirection: SortDirection
  onSort: (field: EntrySortField, direction: SortDirection) => void
}

const views: { mode: FileViewMode; label: string; icon: typeof Monitor }[] = [
  { mode: 'extraLarge', label: 'Extra große Symbole', icon: Monitor },
  { mode: 'large', label: 'Große Symbole', icon: Monitor },
  { mode: 'medium', label: 'Mittelgroße Symbole', icon: Monitor },
  { mode: 'small', label: 'Kleine Symbole', icon: Grid2X2 },
  { mode: 'list', label: 'Liste', icon: List },
  { mode: 'details', label: 'Details', icon: Rows3 },
  { mode: 'tiles', label: 'Kacheln', icon: LayoutList },
  { mode: 'content', label: 'Inhalt', icon: LayoutList },
]

export function ViewMenu({ viewMode, paneMode, onViewMode, onPaneMode, sortField, sortDirection, onSort }: Props) {
  const [open, setOpen] = useState(false)
  const rootRef = useRef<HTMLDivElement>(null)
  useEffect(() => {
    const close = (event: MouseEvent) => { if (!rootRef.current?.contains(event.target as Node)) setOpen(false) }
    window.addEventListener('mousedown', close)
    return () => window.removeEventListener('mousedown', close)
  }, [])
  return <div className="view-menu-root" ref={rootRef}>
    <button className="view-menu-trigger" title="Ansicht" aria-expanded={open} onClick={() => setOpen(value => !value)}><Rows3 />Ansicht<ChevronDown /></button>
    {open && <div className="view-menu">
      {views.map(item => {
        const Icon = item.icon
        return <button key={item.mode} className={viewMode === item.mode ? 'active' : ''} onClick={() => { onViewMode(item.mode); setOpen(false) }}>
          <span className="menu-check">{viewMode === item.mode && <Check />}</span><Icon /><span>{item.label}</span>
        </button>
      })}
      <hr />
      <span className="view-menu-label">Sortieren nach</span>
      {([['name', 'Name'], ['type', 'Typ'], ['size', 'Grösse'], ['modified', 'Geändert']] as const).map(([field, label]) => {
        const active = sortField === field
        const DirectionIcon = sortDirection === 'asc' ? ArrowUp : ArrowDown
        return <button key={field} className={active ? 'active' : ''} onClick={() => onSort(field, active && sortDirection === 'asc' ? 'desc' : 'asc')}>
          <span className="menu-check">{active && <DirectionIcon />}</span><span>{label}</span>
        </button>
      })}
      <hr />
      <button className={paneMode === 'details' ? 'active' : ''} onClick={() => onPaneMode(paneMode === 'details' ? 'none' : 'details')}>
        <span className="menu-check">{paneMode === 'details' && <Check />}</span><PanelRight /><span>Detailbereich</span>
      </button>
      <button className={paneMode === 'preview' ? 'active' : ''} onClick={() => onPaneMode(paneMode === 'preview' ? 'none' : 'preview')}>
        <span className="menu-check">{paneMode === 'preview' && <Check />}</span><Eye /><span>Vorschaufenster</span>
      </button>
    </div>}
  </div>
}
