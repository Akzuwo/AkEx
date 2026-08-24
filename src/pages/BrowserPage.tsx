import { FolderPlus, LoaderCircle, RefreshCw } from 'lucide-react'
import { useCallback, useEffect, useRef, useState } from 'react'
import { Breadcrumbs } from '../components/Breadcrumbs'
import { FileTable } from '../components/FileTable'
import { PageFooter } from '../components/PageFooter'
import { backend, errorMessage } from '../services/backend'
import type { ClipboardOperation, Entry, Page } from '../types'
import { formatBytes, formatDate } from '../utils/format'

const PAGE_SIZE = 300

interface Props {
  path: string
  refreshToken: number
  clipboard: ClipboardOperation
  onNavigate: (path: string) => void
  onClipboard: (value: ClipboardOperation) => void
  onError: (message: string) => void
}

export function BrowserPage({ path, refreshToken, clipboard, onNavigate, onClipboard, onError }: Props) {
  const [page, setPage] = useState<Page<Entry>>({ items: [], total: 0, offset: 0, limit: PAGE_SIZE })
  const [loading, setLoading] = useState(false)
  const [loadError, setLoadError] = useState('')
  const handledRefresh = useRef(refreshToken)

  const load = useCallback(async (offset = 0, reconcile = false) => {
    if (!path) return
    setLoading(true); setLoadError('')
    try {
      if (reconcile) await backend.refresh(path)
      setPage(await backend.directory(path, offset, PAGE_SIZE))
    } catch (error) { setLoadError(errorMessage(error)) }
    finally { setLoading(false) }
  }, [path])

  useEffect(() => { void load(0, false) }, [load])
  useEffect(() => {
    if (handledRefresh.current === refreshToken) return
    handledRefresh.current = refreshToken
    void load(page.offset, true)
  }, [refreshToken]) // eslint-disable-line react-hooks/exhaustive-deps

  async function open(entry: Entry) {
    try { if (entry.isDirectory) onNavigate(entry.fullPath); else await backend.open(entry.fullPath) }
    catch (error) { onError(errorMessage(error)) }
  }
  async function rename(entry: Entry) {
    const name = window.prompt('Neuer Name', entry.name)
    if (!name || name === entry.name) return
    try { await backend.rename(entry.fullPath, name); await load(page.offset) } catch (error) { onError(errorMessage(error)) }
  }
  async function remove(entries: Entry[]) {
    if (!window.confirm(`${entries.length} Eintrag/Einträge in den Papierkorb verschieben?`)) return
    try { await backend.remove(entries.map(entry => entry.fullPath)); await load(page.offset) } catch (error) { onError(errorMessage(error)) }
  }
  async function createFolder() {
    const name = window.prompt('Name des neuen Ordners', 'Neuer Ordner')
    if (!name) return
    try { await backend.createFolder(path, name); await load(page.offset) } catch (error) { onError(errorMessage(error)) }
  }
  async function properties(entry: Entry) {
    try {
      const value = await backend.properties(entry.fullPath)
      window.alert(`${value.path}\n\nTyp: ${value.isDirectory ? 'Ordner' : 'Datei'}\nGrösse: ${formatBytes(entry.isDirectory ? entry.recursiveSize : value.size)}\nGeändert: ${formatDate(value.modifiedAt)}\nSchreibgeschützt: ${value.readOnly ? 'Ja' : 'Nein'}`)
    } catch (error) { onError(errorMessage(error)) }
  }
  async function paste() {
    if (!clipboard?.paths.length) return
    try {
      if (clipboard.mode === 'copy') await backend.copy(clipboard.paths, path)
      else { await backend.move(clipboard.paths, path); onClipboard(null) }
      await load(0)
    } catch (error) { onError(errorMessage(error)) }
  }
  async function drop(paths: string[], destination: Entry, copy: boolean) {
    try { if (copy) await backend.copy(paths, destination.fullPath); else await backend.move(paths, destination.fullPath); await load(page.offset) }
    catch (error) { onError(errorMessage(error)) }
  }

  return <section className="page browser-page">
    <div className="page-heading"><div><Breadcrumbs path={path} onNavigate={onNavigate} /><p>{page.total.toLocaleString('de-CH')} Einträge · Ordnergrössen aus dem Index</p></div><div className="heading-actions"><button onClick={createFolder}><FolderPlus />Neuer Ordner</button><button className="icon-button" title="Aktualisieren" onClick={() => void load(page.offset, true)}><RefreshCw /></button></div></div>
    {loadError ? <div className="notice warning"><strong>Ordner nicht verfügbar</strong><span>{loadError}</span><span>Indexiere das Laufwerk unter „Index-Verwaltung“.</span></div> :
      <FileTable entries={page.items} onOpen={open} onReveal={entry => void backend.reveal(entry.fullPath)} onRename={rename} onDelete={remove} onProperties={properties}
        onClipboard={(mode, entries) => onClipboard({ mode, paths: entries.map(entry => entry.fullPath) })} onPaste={paste} onDropEntries={drop} />}
    {loading && <div className="loading-overlay"><LoaderCircle className="spin" />Lade Index …</div>}
    {!loadError && <PageFooter {...page} onPage={offset => void load(offset)} />}
  </section>
}
