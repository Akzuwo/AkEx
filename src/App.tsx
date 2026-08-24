import { AlertTriangle, X } from 'lucide-react'
import { listen } from '@tauri-apps/api/event'
import { useCallback, useEffect, useRef, useState } from 'react'
import './App.css'
import { Sidebar } from './components/Sidebar'
import { Toolbar } from './components/Toolbar'
import { AnalysisPage } from './pages/AnalysisPage'
import { BrowserPage } from './pages/BrowserPage'
import { IndexPage } from './pages/IndexPage'
import { SearchPage } from './pages/SearchPage'
import { backend, errorMessage } from './services/backend'
import { useNavigation } from './stores/useNavigation'
import type { ClipboardOperation, View, Volume } from './types'

export default function App() {
  const navigation = useNavigation()
  const [volumes, setVolumes] = useState<Volume[]>([])
  const [activeView, setActiveView] = useState<View>('browser')
  const [query, setQuery] = useState('')
  const [clipboard, setClipboard] = useState<ClipboardOperation>(null)
  const [refreshToken, setRefreshToken] = useState(0)
  const [error, setError] = useState('')
  const searchRef = useRef<HTMLInputElement>(null)

  const loadVolumes = useCallback(async () => {
    try {
      const result = await backend.volumes()
      setVolumes(result)
      if (!navigation.currentPath && result[0]) navigation.navigate(result[0].rootPath)
    } catch (reason) { setError(errorMessage(reason)) }
  }, [navigation.currentPath, navigation.navigate])

  useEffect(() => { void loadVolumes().then(() => backend.startWatchers().catch(() => undefined)) }, []) // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => {
    const complete = listen('index:complete', () => { void loadVolumes().then(() => backend.startWatchers()) })
    const failed = listen<{ message: string }>('index:error', event => { setError(event.payload.message); void loadVolumes() })
    const cancelled = listen('index:cancelled', () => { void loadVolumes() })
    return () => { void Promise.all([complete, failed, cancelled]).then(disposers => disposers.forEach(dispose => dispose())) }
  }, []) // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      if (event.ctrlKey && event.key.toLocaleLowerCase() === 'f') { event.preventDefault(); setActiveView('search'); searchRef.current?.focus() }
      else if (event.altKey && event.key === 'ArrowLeft') { event.preventDefault(); navigation.back() }
      else if (event.altKey && event.key === 'ArrowRight') { event.preventDefault(); navigation.forward() }
      else if (event.altKey && event.key === 'ArrowUp') { event.preventDefault(); navigation.up() }
      else if (event.key === 'Escape') setError('')
    }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [navigation])

  function browse(path: string) { navigation.navigate(path); setActiveView('browser') }
  return <div className="app-shell">
    <Sidebar volumes={volumes} activeView={activeView} currentPath={navigation.currentPath} onView={setActiveView} onVolume={browse} />
    <div className="workspace">
      <Toolbar path={navigation.currentPath} query={query} searchRef={searchRef} canBack={navigation.canBack} canForward={navigation.canForward} canUp={navigation.canUp}
        onBack={navigation.back} onForward={navigation.forward} onUp={navigation.up} onPath={browse} onQuery={value => { setQuery(value); setActiveView('search') }} onSearchFocus={() => setActiveView('search')} onRefresh={() => setRefreshToken(value => value + 1)} />
      <main>
        {activeView === 'browser' && <BrowserPage path={navigation.currentPath} refreshToken={refreshToken} clipboard={clipboard} onNavigate={navigation.navigate} onClipboard={setClipboard} onError={setError} />}
        {activeView === 'search' && <SearchPage query={query} onNavigate={browse} onClipboard={setClipboard} onError={setError} />}
        {activeView === 'analysis' && <AnalysisPage volumes={volumes} initialPath={navigation.currentPath} onNavigate={browse} onError={setError} />}
        {activeView === 'index' && <IndexPage volumes={volumes} onChanged={loadVolumes} onError={setError} />}
      </main>
      {clipboard && <div className="clipboard-bar">{clipboard.mode === 'copy' ? 'Kopieren' : 'Verschieben'}: {clipboard.paths.length} Eintrag/Einträge <span>Ctrl+V zum Einfügen</span><button onClick={() => setClipboard(null)}><X /></button></div>}
    </div>
    {error && <div className="toast"><AlertTriangle /><span>{error}</span><button onClick={() => setError('')}><X /></button></div>}
  </div>
}
