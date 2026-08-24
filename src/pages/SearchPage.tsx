import { LoaderCircle, Search } from 'lucide-react'
import { useEffect, useState } from 'react'
import { FileTable } from '../components/FileTable'
import { PageFooter } from '../components/PageFooter'
import { useDebouncedValue } from '../hooks/useDebouncedValue'
import { backend, errorMessage } from '../services/backend'
import type { ClipboardOperation, Entry, Page } from '../types'
import { formatBytes, formatDate } from '../utils/format'

const PAGE_SIZE = 300

export function SearchPage({ query, onNavigate, onClipboard, onError }: { query: string; onNavigate: (path: string) => void; onClipboard: (value: ClipboardOperation) => void; onError: (message: string) => void }) {
  const debounced = useDebouncedValue(query)
  const [page, setPage] = useState<Page<Entry>>({ items: [], total: 0, offset: 0, limit: PAGE_SIZE })
  const [loading, setLoading] = useState(false)
  const [parseError, setParseError] = useState('')
  async function load(offset = 0) {
    if (!debounced.trim()) { setPage({ items: [], total: 0, offset: 0, limit: PAGE_SIZE }); return }
    setLoading(true); setParseError('')
    try { setPage(await backend.search(debounced, offset, PAGE_SIZE)) }
    catch (error) { setParseError(errorMessage(error)) }
    finally { setLoading(false) }
  }
  useEffect(() => { void load(0) }, [debounced]) // eslint-disable-line react-hooks/exhaustive-deps
  async function open(entry: Entry) { try { if (entry.isDirectory) onNavigate(entry.fullPath); else await backend.open(entry.fullPath) } catch (error) { onError(errorMessage(error)) } }
  async function remove(entries: Entry[]) { if (window.confirm(`${entries.length} Eintrag/Einträge in den Papierkorb verschieben?`)) { try { await backend.remove(entries.map(e => e.fullPath)); await load(page.offset) } catch (error) { onError(errorMessage(error)) } } }
  async function rename(entry: Entry) { const name = window.prompt('Neuer Name', entry.name); if (name && name !== entry.name) { try { await backend.rename(entry.fullPath, name); await load(page.offset) } catch (error) { onError(errorMessage(error)) } } }
  async function properties(entry: Entry) { try { const value = await backend.properties(entry.fullPath); window.alert(`${value.path}\n\nGrösse: ${formatBytes(entry.isDirectory ? entry.recursiveSize : value.size)}\nGeändert: ${formatDate(value.modifiedAt)}`) } catch (error) { onError(errorMessage(error)) } }
  return <section className="page search-page">
    <div className="page-heading"><div><h1><Search />Index-Suche</h1><p>{debounced ? `${page.total.toLocaleString('de-CH')} Treffer` : 'Dateien und Ordner ohne Dateisystemscan finden'}</p></div></div>
    <div className="syntax-help"><code>ext:blend</code><code>size:&gt;1gb</code><code>type:file</code><code>path:Projekte</code><span>Filter lassen sich kombinieren.</span></div>
    {parseError && <div className="notice error">{parseError}</div>}
    <FileTable entries={page.items} emptyText={debounced ? 'Keine Treffer.' : 'Suchbegriff oben eingeben.'} onOpen={open} onReveal={entry => void backend.reveal(entry.fullPath)} onRename={rename} onDelete={remove} onProperties={properties} onClipboard={(mode, entries) => onClipboard({ mode, paths: entries.map(e => e.fullPath) })} />
    {loading && <div className="loading-overlay"><LoaderCircle className="spin" />Suche …</div>}
    <PageFooter {...page} onPage={offset => void load(offset)} />
  </section>
}
