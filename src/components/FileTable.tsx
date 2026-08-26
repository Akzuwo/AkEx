import { useVirtualizer } from '@tanstack/react-virtual'
import { ArrowDown, ArrowUp, Copy, ExternalLink, FolderInput, Info, PanelsTopLeft, Pencil, Scissors, Trash2 } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import type { Entry, EntrySortField, SortDirection } from '../types'
import { fileKind, formatBytes, formatDate } from '../utils/format'
import { iconFor } from '../utils/fileIcons'

interface Props {
  entries: Entry[]
  emptyText?: string
  onOpen: (entry: Entry) => void
  onReveal: (entry: Entry) => void
  onOpenWindow?: (entry: Entry) => void
  onRename: (entry: Entry) => void
  onDelete: (entries: Entry[]) => void
  onProperties: (entry: Entry) => void
  onClipboard: (mode: 'copy' | 'move', entries: Entry[]) => void
  onPaste?: () => void
  onDropEntries?: (paths: string[], destination: Entry, copy: boolean) => void
  sortField: EntrySortField
  sortDirection: SortDirection
  onSort: (field: EntrySortField, direction: SortDirection) => void
}

export function FileTable(props: Props) {
  const parentRef = useRef<HTMLDivElement>(null)
  const [selected, setSelected] = useState<Set<number>>(new Set())
  const [menu, setMenu] = useState<{ x: number; y: number; entry: Entry } | null>(null)
  const virtualizer = useVirtualizer({ count: props.entries.length, getScrollElement: () => parentRef.current, estimateSize: () => 44, overscan: 12 })
  const selectedEntries = props.entries.filter(entry => selected.has(entry.id))

  useEffect(() => { setSelected(new Set()); setMenu(null) }, [props.entries])
  useEffect(() => {
    const close = () => setMenu(null)
    window.addEventListener('click', close)
    return () => window.removeEventListener('click', close)
  }, [])

  function select(entry: Entry, additive: boolean) {
    setSelected(previous => additive ? new Set(previous.has(entry.id) ? [...previous].filter(id => id !== entry.id) : [...previous, entry.id]) : new Set([entry.id]))
  }
  function targets(fallback?: Entry) { return selectedEntries.length ? selectedEntries : fallback ? [fallback] : [] }
  function sortBy(field: EntrySortField) {
    props.onSort(field, props.sortField === field && props.sortDirection === 'asc' ? 'desc' : 'asc')
  }
  function sortHeader(field: EntrySortField, label: string) {
    const active = props.sortField === field
    const direction = active ? props.sortDirection : undefined
    const Icon = direction === 'desc' ? ArrowDown : ArrowUp
    return <button className={active ? 'active' : ''} type="button" onClick={() => sortBy(field)}
      aria-label={`${label} ${active ? direction === 'asc' ? 'absteigend' : 'aufsteigend' : 'aufsteigend'} sortieren`}
      aria-sort={active ? direction === 'asc' ? 'ascending' : 'descending' : 'none'}>
      <span>{label}</span><Icon aria-hidden="true" />
    </button>
  }
  function keyDown(event: React.KeyboardEvent) {
    const primary = selectedEntries[0]
    if (event.key === 'Delete' && selectedEntries.length) { event.preventDefault(); props.onDelete(selectedEntries) }
    else if (event.key === 'F2' && primary) { event.preventDefault(); props.onRename(primary) }
    else if (event.key === 'Enter' && primary) { event.preventDefault(); props.onOpen(primary) }
    else if (event.ctrlKey && event.key.toLocaleLowerCase() === 'c' && selectedEntries.length) { event.preventDefault(); props.onClipboard('copy', selectedEntries) }
    else if (event.ctrlKey && event.key.toLocaleLowerCase() === 'x' && selectedEntries.length) { event.preventDefault(); props.onClipboard('move', selectedEntries) }
    else if (event.ctrlKey && event.key.toLocaleLowerCase() === 'v' && props.onPaste) { event.preventDefault(); props.onPaste() }
    else if (event.ctrlKey && event.key.toLocaleLowerCase() === 'a') { event.preventDefault(); setSelected(new Set(props.entries.map(entry => entry.id))) }
    else if (event.key === 'Escape') { setSelected(new Set()); setMenu(null) }
  }

  return <div className="table-shell" tabIndex={0} onKeyDown={keyDown}>
    <div className="file-header">
      {sortHeader('name', 'Name')}
      {sortHeader('type', 'Typ')}
      {sortHeader('size', 'Grösse')}
      {sortHeader('modified', 'Geändert')}
    </div>
    <div className="file-scroll" ref={parentRef}>
      {!props.entries.length && <div className="empty-state">{props.emptyText ?? 'Dieser Ordner ist leer.'}</div>}
      <div className="virtual-body" style={{ height: virtualizer.getTotalSize() }}>
        {virtualizer.getVirtualItems().map(item => {
          const entry = props.entries[item.index]
          const Icon = iconFor(entry.extension, entry.isDirectory)
          return <div key={entry.id} className={`file-row ${selected.has(entry.id) ? 'selected' : ''}`} style={{ transform: `translateY(${item.start}px)` }}
            draggable onDragStart={event => event.dataTransfer.setData('application/x-akex-paths', JSON.stringify(targets(entry).map(item => item.fullPath)))}
            onDragOver={event => { if (entry.isDirectory) event.preventDefault() }}
            onDrop={event => { if (!entry.isDirectory || !props.onDropEntries) return; event.preventDefault(); const data = event.dataTransfer.getData('application/x-akex-paths'); if (data) props.onDropEntries(JSON.parse(data), entry, event.ctrlKey) }}
            onClick={event => select(entry, event.ctrlKey)} onDoubleClick={() => props.onOpen(entry)}
            onContextMenu={event => { event.preventDefault(); select(entry, event.ctrlKey); setMenu({ x: event.clientX, y: event.clientY, entry }) }}>
            <span className="name-cell"><Icon className={entry.isDirectory ? 'folder-icon' : 'file-icon'} size={19} /><span>{entry.name}</span>{entry.hidden && <small>versteckt</small>}</span>
            <span>{fileKind(entry.extension, entry.isDirectory)}</span>
            <span>{formatBytes(entry.isDirectory ? entry.recursiveSize : entry.size)}</span>
            <span>{formatDate(entry.modifiedAt)}</span>
          </div>
        })}
      </div>
    </div>
    {menu && <div className="context-menu" style={{ left: menu.x, top: menu.y }} onClick={event => event.stopPropagation()}>
      <button onClick={() => { props.onOpen(menu.entry); setMenu(null) }}><ExternalLink />Öffnen</button>
      {menu.entry.isDirectory && props.onOpenWindow && <button onClick={() => { props.onOpenWindow?.(menu.entry); setMenu(null) }}><PanelsTopLeft />In neuem Fenster öffnen</button>}
      <button onClick={() => { props.onReveal(menu.entry); setMenu(null) }}><FolderInput />Im Ordner anzeigen</button>
      <button onClick={() => { void navigator.clipboard.writeText(menu.entry.fullPath); setMenu(null) }}><Copy />Pfad kopieren</button>
      <hr />
      <button onClick={() => { props.onClipboard('copy', targets(menu.entry)); setMenu(null) }}><Copy />Kopieren</button>
      <button onClick={() => { props.onClipboard('move', targets(menu.entry)); setMenu(null) }}><Scissors />Ausschneiden</button>
      <button onClick={() => { props.onRename(menu.entry); setMenu(null) }}><Pencil />Umbenennen</button>
      <button className="danger" onClick={() => { props.onDelete(targets(menu.entry)); setMenu(null) }}><Trash2 />In Papierkorb</button>
      <hr />
      <button onClick={() => { props.onProperties(menu.entry); setMenu(null) }}><Info />Eigenschaften</button>
    </div>}
  </div>
}
