import { getCurrentWebview } from '@tauri-apps/api/webview'
import { Folder, Plus, X } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'

interface TabBarProps {
  tabs: { id: string; path: string }[]
  activeTabId: string
  onActivate: (id: string) => void
  onClose: (id: string) => void
  onNew: () => void
  onDetach: (id: string, path: string) => void
  onDropFiles: (paths: string[], destination: string, copy: boolean) => void
}

function tabTitle(path: string) {
  if (!path) return 'Neuer Tab'
  const trimmed = path.replace(/[\\/]+$/, '')
  return trimmed.split(/[\\/]/).pop() || path
}

export function TabBar({ tabs, activeTabId, onActivate, onClose, onNew, onDetach, onDropFiles }: TabBarProps) {
  const [draggingTabId, setDraggingTabId] = useState<string | null>(null)
  const [hoveredTabId, setHoveredTabId] = useState<string | null>(null)
  const pointerDrag = useRef<{ id: string; path: string; x: number; y: number; dragging: boolean; detached: boolean } | null>(null)
  const activateRef = useRef(onActivate)
  const detachRef = useRef(onDetach)
  const dropFilesRef = useRef(onDropFiles)
  const copyDropRef = useRef(false)

  useEffect(() => { activateRef.current = onActivate }, [onActivate])
  useEffect(() => { detachRef.current = onDetach }, [onDetach])
  useEffect(() => { dropFilesRef.current = onDropFiles }, [onDropFiles])

  useEffect(() => {
    const finish = () => { pointerDrag.current = null; setDraggingTabId(null) }
    const detach = () => {
      const state = pointerDrag.current
      if (!state?.dragging || state.detached) return
      state.detached = true
      detachRef.current(state.id, state.path)
      finish()
    }
    const move = (event: PointerEvent) => {
      const state = pointerDrag.current
      if (!state || state.detached) return
      if (!state.dragging && Math.hypot(event.clientX - state.x, event.clientY - state.y) >= 7) {
        state.dragging = true
        setDraggingTabId(state.id)
      }
      if (!state.dragging) return
      const outsideWindow = event.screenX <= window.screenX || event.screenY <= window.screenY ||
        event.screenX >= window.screenX + window.outerWidth || event.screenY >= window.screenY + window.outerHeight
      if (outsideWindow) detach()
    }
    const leave = (event: PointerEvent) => {
      const state = pointerDrag.current
      if (!state || event.buttons !== 1) return
      if (!state.dragging && Math.hypot(event.clientX - state.x, event.clientY - state.y) >= 7) state.dragging = true
      if (state.dragging) detach()
    }
    window.addEventListener('pointermove', move)
    window.addEventListener('pointerup', finish)
    window.addEventListener('pointercancel', finish)
    document.documentElement.addEventListener('pointerleave', leave)
    return () => {
      window.removeEventListener('pointermove', move)
      window.removeEventListener('pointerup', finish)
      window.removeEventListener('pointercancel', finish)
      document.documentElement.removeEventListener('pointerleave', leave)
    }
  }, [])

  useEffect(() => {
    const updateCopyMode = (event: KeyboardEvent) => { copyDropRef.current = event.ctrlKey }
    const resetCopyMode = () => { copyDropRef.current = false }
    window.addEventListener('keydown', updateCopyMode)
    window.addEventListener('keyup', updateCopyMode)
    window.addEventListener('blur', resetCopyMode)
    return () => {
      window.removeEventListener('keydown', updateCopyMode)
      window.removeEventListener('keyup', updateCopyMode)
      window.removeEventListener('blur', resetCopyMode)
    }
  }, [])

  useEffect(() => {
    let disposed = false
    let unlisten: (() => void) | undefined
    let hoverTimer: ReturnType<typeof setTimeout> | undefined
    let pendingId: string | null = null
    const clearHover = () => {
      if (hoverTimer) clearTimeout(hoverTimer)
      hoverTimer = undefined
      pendingId = null
      setHoveredTabId(null)
    }
    const tabAt = (position: { x: number; y: number }) => {
      const scale = window.devicePixelRatio || 1
      const element = document.elementFromPoint(position.x / scale, position.y / scale)?.closest<HTMLElement>('[data-tab-id]')
      return tabs.find(tab => tab.id === element?.dataset.tabId)
    }
    void getCurrentWebview().onDragDropEvent(event => {
      if (event.payload.type === 'enter' || event.payload.type === 'over') {
        const tab = tabAt(event.payload.position)
        if (!tab || tab.id === activeTabId) { clearHover(); return }
        if (pendingId === tab.id) return
        clearHover()
        pendingId = tab.id
        setHoveredTabId(tab.id)
        hoverTimer = setTimeout(() => {
          activateRef.current(tab.id)
          clearHover()
        }, 650)
      } else if (event.payload.type === 'drop') {
        const tab = tabAt(event.payload.position)
        clearHover()
        if (tab?.path) {
          activateRef.current(tab.id)
          dropFilesRef.current(event.payload.paths, tab.path, copyDropRef.current)
        }
      } else {
        clearHover()
      }
    }).then(dispose => { if (disposed) dispose(); else unlisten = dispose })
    return () => {
      disposed = true
      clearHover()
      unlisten?.()
    }
  }, [activeTabId, tabs])

  return <div className="tab-bar">
    <div className="tab-strip" role="tablist" aria-label="Geöffnete Ordner">
      {tabs.map(tab => <div
        className={`tab-item ${tab.id === activeTabId ? 'active' : ''} ${draggingTabId === tab.id ? 'dragging' : ''} ${hoveredTabId === tab.id ? 'drag-hover' : ''}`}
        key={tab.id}
        data-tab-id={tab.id}
        role="tab"
        aria-selected={tab.id === activeTabId}
        tabIndex={tab.id === activeTabId ? 0 : -1}
        title={tab.path || 'Neuer Tab'}
        onPointerDown={event => {
          if (event.button !== 0 || (event.target as HTMLElement).closest('button')) return
          pointerDrag.current = { id: tab.id, path: tab.path, x: event.clientX, y: event.clientY, dragging: false, detached: false }
        }}
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
