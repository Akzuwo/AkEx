import { useVirtualizer } from '@tanstack/react-virtual'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import { ArrowDown, ArrowUp, Copy, ExternalLink, FolderInput, Info, PanelsTopLeft, Pencil, Scissors, Trash2 } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import type { Entry, EntrySortField, FileViewMode, SortDirection } from '../types'
import { fileKind, formatBytes, formatDate } from '../utils/format'
import { iconFor } from '../utils/fileIcons'

interface Props {
  entries: Entry[]
  emptyText?: string
  onOpen: (entry: Entry) => void
  onReveal: (entry: Entry) => void
  onOpenTab?: (entry: Entry) => void
  onOpenWindow?: (entry: Entry) => void
  onRename: (entry: Entry) => void
  onDelete: (entries: Entry[]) => void
  onProperties: (entry: Entry) => void
  onClipboard: (mode: 'copy' | 'move', entries: Entry[]) => void
  onPaste?: () => void
  onDropEntries?: (paths: string[], destination: Entry | null, copy: boolean) => void
  onDragOut: (entries: Entry[], copy: boolean) => void
  sortField: EntrySortField
  sortDirection: SortDirection
  onSort: (field: EntrySortField, direction: SortDirection) => void
  viewMode: FileViewMode
  onSelectionChange?: (entries: Entry[]) => void
}

export function FileTable(props: Props) {
  const visibleEntries = props.entries
  const dropEntries = props.onDropEntries
  const shellRef = useRef<HTMLDivElement>(null)
  const parentRef = useRef<HTMLDivElement>(null)
  const copyDropRef = useRef(false)
  const [selected, setSelected] = useState<Set<number>>(new Set())
  const [menu, setMenu] = useState<{ x: number; y: number; entry: Entry; targets: Entry[] } | null>(null)
  const [dropTarget, setDropTarget] = useState<number | null>(null)
  const virtualizer = useVirtualizer({ count: props.entries.length, getScrollElement: () => parentRef.current, estimateSize: () => 44, overscan: 12 })
  const selectedEntries = props.entries.filter(entry => selected.has(entry.id))

  useEffect(() => { setSelected(new Set()); setMenu(null) }, [props.entries])
  useEffect(() => { props.onSelectionChange?.(selectedEntries) }, [selected, props.entries, props.onSelectionChange]) // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => {
    const close = () => setMenu(null)
    window.addEventListener('click', close)
    return () => window.removeEventListener('click', close)
  }, [])
  useEffect(() => {
    const keyDown = (event: KeyboardEvent) => { copyDropRef.current = event.ctrlKey }
    const keyUp = (event: KeyboardEvent) => { copyDropRef.current = event.ctrlKey }
    const reset = () => { copyDropRef.current = false }
    window.addEventListener('keydown', keyDown)
    window.addEventListener('keyup', keyUp)
    window.addEventListener('blur', reset)
    return () => { window.removeEventListener('keydown', keyDown); window.removeEventListener('keyup', keyUp); window.removeEventListener('blur', reset) }
  }, [])
  useEffect(() => {
    if (!dropEntries) return
    let disposed = false
    let unlisten: (() => void) | undefined
    const entryAt = (position: { x: number; y: number }) => {
      const scale = window.devicePixelRatio || 1
      const element = document.elementFromPoint(position.x / scale, position.y / scale)?.closest<HTMLElement>('[data-entry-id]')
      const id = Number(element?.dataset.entryId)
      return visibleEntries.find(entry => entry.id === id && entry.isDirectory)
    }
    const insideTable = (position: { x: number; y: number }) => {
      const scale = window.devicePixelRatio || 1
      const element = document.elementFromPoint(position.x / scale, position.y / scale)
      return Boolean(element && shellRef.current?.contains(element))
    }
    void getCurrentWebview().onDragDropEvent(event => {
      if (event.payload.type === 'enter' || event.payload.type === 'over') {
        setDropTarget(entryAt(event.payload.position)?.id ?? null)
      } else if (event.payload.type === 'drop') {
        const destination = entryAt(event.payload.position)
        setDropTarget(null)
        if (destination || insideTable(event.payload.position)) dropEntries(event.payload.paths, destination ?? null, copyDropRef.current)
      } else {
        setDropTarget(null)
      }
    }).then(dispose => { if (disposed) dispose(); else unlisten = dispose })
    return () => { disposed = true; unlisten?.() }
  }, [visibleEntries, dropEntries])

  function select(entry: Entry, additive: boolean) {
    shellRef.current?.focus({ preventScroll: true })
    setSelected(previous => additive ? new Set(previous.has(entry.id) ? [...previous].filter(id => id !== entry.id) : [...previous, entry.id]) : new Set([entry.id]))
  }
  function targets(entry: Entry) { return selected.has(entry.id) ? selectedEntries : [entry] }
  function openMenu(event: React.MouseEvent, entry: Entry) {
    event.preventDefault()
    shellRef.current?.focus({ preventScroll: true })
    const actionTargets = selected.has(entry.id) ? selectedEntries : event.ctrlKey ? [...selectedEntries, entry] : [entry]
    setSelected(new Set(actionTargets.map(item => item.id)))
    setMenu({ x: event.clientX, y: event.clientY, entry, targets: actionTargets })
  }
  function dragOut(event: React.DragEvent, entry: Entry) {
    event.preventDefault()
    const actionTargets = targets(entry)
    setSelected(new Set(actionTargets.map(item => item.id)))
    copyDropRef.current = event.ctrlKey
    props.onDragOut(actionTargets, event.ctrlKey)
  }
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

  function renderEntry(entry: Entry, top?: number) {
    const Icon = iconFor(entry.extension, entry.isDirectory)
    const details = props.viewMode === 'details'
    const iconSize = props.viewMode === 'extraLarge' ? 72 : props.viewMode === 'large' ? 56 : props.viewMode === 'medium' ? 38 : props.viewMode === 'tiles' || props.viewMode === 'content' ? 34 : 18
    return <div key={entry.id} data-entry-id={entry.id} title="Ziehen zum Verschieben · Ctrl gedrückt halten zum Kopieren"
      className={`${details ? 'file-row' : `file-item view-${props.viewMode}`} ${selected.has(entry.id) ? 'selected' : ''} ${dropTarget === entry.id ? 'drop-target' : ''}`}
      style={details ? { transform: `translateY(${top ?? 0}px)` } : undefined}
      draggable onDragStart={event => dragOut(event, entry)}
      onDragOver={event => { if (entry.isDirectory) event.preventDefault() }}
      onDrop={event => { if (!entry.isDirectory || !props.onDropEntries) return; event.preventDefault(); const data = event.dataTransfer.getData('application/x-akex-paths'); if (data) props.onDropEntries(JSON.parse(data), entry, event.ctrlKey) }}
      onClick={event => select(entry, event.ctrlKey)} onDoubleClick={() => props.onOpen(entry)}
      onContextMenu={event => openMenu(event, entry)}>
      {details ? <>
        <span className="name-cell"><Icon className={entry.isDirectory ? 'folder-icon' : 'file-icon'} size={19} /><span>{entry.name}</span>{entry.hidden && <small>versteckt</small>}</span>
        <span>{fileKind(entry.extension, entry.isDirectory)}</span>
        <span>{formatBytes(entry.isDirectory ? entry.recursiveSize : entry.size)}</span>
        <span>{formatDate(entry.modifiedAt)}</span>
      </> : <>
        <Icon className={`item-icon ${entry.isDirectory ? 'folder-icon' : 'file-icon'}`} size={iconSize} />
        <div className="item-copy">
          <span className="item-name">{entry.name}</span>
          {(props.viewMode === 'tiles' || props.viewMode === 'content') && <small>{fileKind(entry.extension, entry.isDirectory)} · {formatBytes(entry.isDirectory ? entry.recursiveSize : entry.size)}</small>}
          {props.viewMode === 'content' && <small>Geändert: {formatDate(entry.modifiedAt)} · {entry.fullPath}</small>}
          {entry.hidden && <small>Versteckt</small>}
        </div>
      </>}
    </div>
  }

  return <div className="table-shell" ref={shellRef} tabIndex={0} onKeyDown={keyDown}>
    {props.viewMode === 'details' && <div className="file-header">
      {sortHeader('name', 'Name')}
      {sortHeader('type', 'Typ')}
      {sortHeader('size', 'Grösse')}
      {sortHeader('modified', 'Geändert')}
    </div>}
    <div className="file-scroll" ref={parentRef}>
      {!props.entries.length && <div className="empty-state">{props.emptyText ?? 'Dieser Ordner ist leer.'}</div>}
      {props.viewMode === 'details' ? <div className="virtual-body" style={{ height: virtualizer.getTotalSize() }}>
        {virtualizer.getVirtualItems().map(item => renderEntry(props.entries[item.index], item.start))}
      </div> : <div className={`file-layout layout-${props.viewMode}`}>{props.entries.map(entry => renderEntry(entry))}</div>}
    </div>
    {menu && <div className="context-menu" style={{ left: menu.x, top: menu.y }} onClick={event => event.stopPropagation()}>
      <button onClick={() => { props.onOpen(menu.entry); setMenu(null) }}><ExternalLink />Öffnen</button>
      {menu.entry.isDirectory && props.onOpenTab && <button onClick={() => { props.onOpenTab?.(menu.entry); setMenu(null) }}><PanelsTopLeft />In neuem Tab öffnen</button>}
      {menu.entry.isDirectory && props.onOpenWindow && <button onClick={() => { props.onOpenWindow?.(menu.entry); setMenu(null) }}><PanelsTopLeft />In neuem Fenster öffnen</button>}
      <button onClick={() => { props.onReveal(menu.entry); setMenu(null) }}><FolderInput />Im Ordner anzeigen</button>
      <button onClick={() => { void navigator.clipboard.writeText(menu.entry.fullPath); setMenu(null) }}><Copy />Pfad kopieren</button>
      <hr />
      <button onClick={() => { props.onClipboard('copy', menu.targets); setMenu(null) }}><Copy />Kopieren</button>
      <button onClick={() => { props.onClipboard('move', menu.targets); setMenu(null) }}><Scissors />Ausschneiden</button>
      <button onClick={() => { props.onRename(menu.entry); setMenu(null) }}><Pencil />Umbenennen</button>
      <button className="danger" onClick={() => { props.onDelete(menu.targets); setMenu(null) }}><Trash2 />{menu.targets.length > 1 ? `${menu.targets.length} Einträge in Papierkorb` : 'In Papierkorb'}</button>
      <hr />
      <button onClick={() => { props.onProperties(menu.entry); setMenu(null) }}><Info />Eigenschaften</button>
    </div>}
  </div>
}
