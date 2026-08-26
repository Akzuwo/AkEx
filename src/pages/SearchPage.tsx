import { LoaderCircle, Search } from 'lucide-react'
import { useEffect, useState } from 'react'
import { FileTable } from '../components/FileTable'
import { PageFooter } from '../components/PageFooter'
import { useDebouncedValue } from '../hooks/useDebouncedValue'
import { backend, errorMessage } from '../services/backend'
import type { ClipboardOperation, Entry, EntrySortField, Page, SortDirection } from '../types'
import { formatBytes, formatDate } from '../utils/format'
import { startNativeFileDrag } from '../utils/nativeDrag'

const PAGE_SIZE = 300

function parentPath(path: string): string {
  const parent = path.replace(/\\[^\\]+$/, '')
  return /^[a-z]:$/i.test(parent) ? `${parent}\\` : parent
}

export function SearchPage({ query, onNavigate, onOpenWindow, onClipboard, onError }: { query: string; onNavigate: (path: string) => void; onOpenWindow: (path: string) => void; onClipboard: (value: ClipboardOperation) => void; onError: (message: string) => void }) {
  const debounced = useDebouncedValue(query)
  const [page, setPage] = useState<Page<Entry>>({ items: [], total: 0, offset: 0, limit: PAGE_SIZE })
  const [loading, setLoading] = useState(false)
  const [parseError, setParseError] = useState('')
  const [sortField, setSortField] = useState<EntrySortField>('modified')
  const [sortDirection, setSortDirection] = useState<SortDirection>('desc')
  async function load(offset = 0) {
    if (!debounced.trim()) { setPage({ items: [], total: 0, offset: 0, limit: PAGE_SIZE }); return }
    setLoading(true); setParseError('')
    try { setPage(await backend.search(debounced, offset, PAGE_SIZE, sortField, sortDirection)) }
    catch (error) { setParseError(errorMessage(error)) }
    finally { setLoading(false) }
  }
  useEffect(() => { void load(0) }, [debounced, sortField, sortDirection]) // eslint-disable-line react-hooks/exhaustive-deps
  async function open(entry: Entry) { try { if (entry.isDirectory) onNavigate(entry.fullPath); else await backend.open(entry.fullPath) } catch (error) { onError(errorMessage(error)) } }
  async function reveal(entry: Entry) { try { await backend.reveal(entry.fullPath) } catch (error) { onError(errorMessage(error)) } }
  async function remove(entries: Entry[]) { if (window.confirm(`${entries.length} Eintrag/Einträge in den Papierkorb verschieben?`)) { const remaining = Math.max(0, page.total - entries.length); const nextOffset = remaining ? Math.min(page.offset, Math.floor((remaining - 1) / PAGE_SIZE) * PAGE_SIZE) : 0; try { await backend.remove(entries.map(e => e.fullPath)); await load(nextOffset) } catch (error) { onError(errorMessage(error)); await load(page.offset) } } }
  async function rename(entry: Entry) { const name = window.prompt('Neuer Name', entry.name); if (name && name !== entry.name) { try { await backend.rename(entry.fullPath, name); await load(page.offset) } catch (error) { onError(errorMessage(error)) } } }
  async function properties(entry: Entry) { try { const value = await backend.properties(entry.fullPath); window.alert(`${value.path}\n\nGrösse: ${formatBytes(entry.isDirectory ? entry.recursiveSize : value.size)}\nGeändert: ${formatDate(value.modifiedAt)}`) } catch (error) { onError(errorMessage(error)) } }
  async function dragOut(entries: Entry[], copy: boolean) { try { const paths = entries.map(entry => entry.fullPath); await backend.validateDragPaths(paths); const dropped = await startNativeFileDrag(paths, entries.every(entry => entry.isDirectory), copy); if (dropped && !copy) { const parents = [...new Set(paths.map(parentPath))]; await Promise.all(parents.map(parent => backend.refresh(parent))); await load(page.offset) } } catch (error) { onError(errorMessage(error)) } }
  return <section className="page search-page">
    <div className="page-heading"><div><h1><Search />Index-Suche</h1><p>{debounced ? `${page.total.toLocaleString('de-CH')} Treffer` : 'Dateien und Ordner ohne Dateisystemscan finden'}</p></div></div>
    <div className="syntax-help"><code>ext:blend</code><code>size:&gt;1gb</code><code>type:file</code><code>path:Projekte</code><span>Filter lassen sich kombinieren.</span></div>
    {parseError && <div className="notice error">{parseError}</div>}
    <FileTable entries={page.items} emptyText={debounced ? 'Keine Treffer.' : 'Suchbegriff oben eingeben.'} onOpen={open} onReveal={entry => void reveal(entry)} onOpenWindow={entry => onOpenWindow(entry.fullPath)} onRename={rename} onDelete={remove} onProperties={properties} onClipboard={(mode, entries) => onClipboard({ mode, paths: entries.map(e => e.fullPath) })}
      onDragOut={(entries, copy) => void dragOut(entries, copy)} sortField={sortField} sortDirection={sortDirection} onSort={(field, direction) => { setSortField(field); setSortDirection(direction) }} />
    {loading && <div className="loading-overlay"><LoaderCircle className="spin" />Suche …</div>}
    <PageFooter {...page} onPage={offset => void load(offset)} />
  </section>
}
