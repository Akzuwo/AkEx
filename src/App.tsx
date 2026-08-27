import { AlertTriangle, X } from 'lucide-react'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react'
import './App.css'
import { FirstRunModal } from './components/FirstRunModal'
import { SettingsModal } from './components/SettingsModal'
import { Sidebar } from './components/Sidebar'
import { TabBar } from './components/TabBar'
import { Toolbar } from './components/Toolbar'
import { AnalysisPage } from './pages/AnalysisPage'
import { BrowserPage } from './pages/BrowserPage'
import { SearchPage } from './pages/SearchPage'
import { backend, errorMessage } from './services/backend'
import { useNavigation } from './stores/useNavigation'
import type { ClipboardOperation, FilePaneMode, FileViewMode, Theme, View, Volume } from './types'

const viewModes: FileViewMode[] = ['extraLarge', 'large', 'medium', 'small', 'list', 'details', 'tiles', 'content']
const paneModes: FilePaneMode[] = ['none', 'details', 'preview']

export default function App() {
  const navigation = useNavigation(new URLSearchParams(window.location.search).get('path') ?? '')
  const [volumes, setVolumes] = useState<Volume[]>([])
  const [activeView, setActiveView] = useState<View>('browser')
  const [query, setQuery] = useState('')
  const [clipboard, setClipboard] = useState<ClipboardOperation>(null)
  const [refreshToken, setRefreshToken] = useState(0)
  const [error, setError] = useState('')
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [onboardingOpen, setOnboardingOpen] = useState(false)
  const onboardingChecked = useRef(false)
  const [theme, setTheme] = useState<Theme>(() => localStorage.getItem('akex:theme') === 'light' ? 'light' : 'dark')
  const [fileViewMode, setFileViewMode] = useState<FileViewMode>(() => {
    const saved = localStorage.getItem('akex:file-view') as FileViewMode
    return viewModes.includes(saved) ? saved : 'details'
  })
  const [filePaneMode, setFilePaneMode] = useState<FilePaneMode>(() => {
    const saved = localStorage.getItem('akex:file-pane') as FilePaneMode
    return paneModes.includes(saved) ? saved : 'none'
  })
  const searchRef = useRef<HTMLInputElement>(null)

  useLayoutEffect(() => {
    document.documentElement.dataset.theme = theme
    document.documentElement.style.colorScheme = theme
    localStorage.setItem('akex:theme', theme)
  }, [theme])

  const closeTab = useCallback(async (id: string) => {
    if (navigation.tabs.length === 1) await getCurrentWindow().close()
    else navigation.closeTab(id)
  }, [navigation])

  const loadVolumes = useCallback(async () => {
    try {
      const result = await backend.volumes()
      setVolumes(result)
      if (!navigation.currentPath && result[0]) navigation.navigate(result[0].rootPath)
    } catch (reason) { setError(errorMessage(reason)) }
  }, [navigation.currentPath, navigation.navigate])

  useEffect(() => { void loadVolumes().then(() => backend.startWatchers().catch(() => undefined)) }, []) // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => {
    if (!volumes.length || onboardingChecked.current) return
    onboardingChecked.current = true
    const completed = localStorage.getItem('akex:onboarding-complete') === 'true'
    if (!completed && volumes.every(volume => !volume.lastFullScan)) setOnboardingOpen(true)
    else if (!completed) localStorage.setItem('akex:onboarding-complete', 'true')
  }, [volumes])
  useEffect(() => { localStorage.setItem('akex:file-view', fileViewMode) }, [fileViewMode])
  useEffect(() => { localStorage.setItem('akex:file-pane', filePaneMode) }, [filePaneMode])
  useEffect(() => {
    const complete = listen('index:complete', () => { void loadVolumes().then(() => backend.startWatchers()) })
    const failed = listen<{ message: string }>('index:error', event => { setError(event.payload.message); void loadVolumes() })
    const cancelled = listen('index:cancelled', () => { void loadVolumes() })
    return () => { void Promise.all([complete, failed, cancelled]).then(disposers => disposers.forEach(dispose => dispose())) }
  }, []) // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | undefined
    const changed = listen('index:changed', () => {
      if (timer) clearTimeout(timer)
      timer = setTimeout(() => setRefreshToken(value => value + 1), 250)
    })
    return () => {
      if (timer) clearTimeout(timer)
      void changed.then(dispose => dispose())
    }
  }, [])
  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      if (event.ctrlKey && event.key.toLocaleLowerCase() === 'f') { event.preventDefault(); setActiveView('search'); searchRef.current?.focus() }
      else if (event.ctrlKey && event.key.toLocaleLowerCase() === 'n') { event.preventDefault(); if (navigation.currentPath) void openWindow(navigation.currentPath) }
      else if (event.ctrlKey && event.key.toLocaleLowerCase() === 't') { event.preventDefault(); navigation.openTab(volumes[0]?.rootPath ?? ''); setActiveView('browser') }
      else if (event.ctrlKey && event.key.toLocaleLowerCase() === 'w') { event.preventDefault(); void closeTab(navigation.activeTabId) }
      else if (event.altKey && event.key === 'ArrowLeft') { event.preventDefault(); navigation.back() }
      else if (event.altKey && event.key === 'ArrowRight') { event.preventDefault(); navigation.forward() }
      else if (event.altKey && event.key === 'ArrowUp') { event.preventDefault(); navigation.up() }
      else if (event.key === 'Escape') setError('')
    }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [closeTab, navigation, volumes])

  function browse(path: string) { navigation.navigate(path); setActiveView('browser') }
  function openTab(path: string) { navigation.openTab(path); setActiveView('browser') }
  async function openWindow(path: string) {
    try { await backend.openWindow(path) } catch (reason) { setError(errorMessage(reason)) }
  }
  async function detachTab(id: string, path: string) {
    const destination = path || volumes[0]?.rootPath
    if (!destination) return
    try {
      await backend.openWindow(destination)
      await closeTab(id)
    } catch (reason) { setError(errorMessage(reason)) }
  }
  async function dropOnTab(paths: string[], destination: string, copy: boolean) {
    try {
      if (copy) await backend.copy(paths, destination)
      else await backend.move(paths, destination)
      setRefreshToken(value => value + 1)
    } catch (reason) { setError(errorMessage(reason)) }
  }
  function completeOnboarding() {
    localStorage.setItem('akex:onboarding-complete', 'true')
    setOnboardingOpen(false)
  }
  async function startInitialIndex(rootPaths: string[]) {
    try {
      await Promise.all(rootPaths.map(rootPath => backend.startIndex(rootPath)))
      completeOnboarding()
      await loadVolumes()
    } catch (reason) { setError(errorMessage(reason)); throw reason }
  }
  return <div className="app-shell">
    <Sidebar volumes={volumes} activeView={activeView} currentPath={navigation.currentPath} onView={setActiveView} onVolume={browse} />
    <div className="workspace">
      <Toolbar path={navigation.currentPath} query={query} searchRef={searchRef} canBack={navigation.canBack} canForward={navigation.canForward} canUp={navigation.canUp}
        onBack={navigation.back} onForward={navigation.forward} onUp={navigation.up} onPath={browse} onQuery={value => { setQuery(value); setActiveView('search') }} onSearchFocus={() => setActiveView('search')} onRefresh={() => setRefreshToken(value => value + 1)} onNewWindow={() => void openWindow(navigation.currentPath)} onSettings={() => setSettingsOpen(true)} />
      <TabBar tabs={navigation.tabs} activeTabId={navigation.activeTabId} onActivate={id => { navigation.activateTab(id); setActiveView('browser') }} onClose={id => void closeTab(id)} onNew={() => { navigation.openTab(volumes[0]?.rootPath ?? ''); setActiveView('browser') }} onDetach={(id, path) => void detachTab(id, path)} onDropFiles={(paths, destination, copy) => void dropOnTab(paths, destination, copy)} />
      <main>
        {activeView === 'browser' && <BrowserPage path={navigation.currentPath} refreshToken={refreshToken} clipboard={clipboard} viewMode={fileViewMode} paneMode={filePaneMode} onViewMode={setFileViewMode} onPaneMode={setFilePaneMode} onNavigate={navigation.navigate} onOpenTab={openTab} onOpenWindow={openWindow} onClipboard={setClipboard} onError={setError} />}
        {activeView === 'search' && <SearchPage query={query} refreshToken={refreshToken} viewMode={fileViewMode} paneMode={filePaneMode} onViewMode={setFileViewMode} onPaneMode={setFilePaneMode} onNavigate={browse} onOpenTab={openTab} onOpenWindow={openWindow} onClipboard={setClipboard} onError={setError} />}
        {activeView === 'analysis' && <AnalysisPage volumes={volumes} initialPath={navigation.currentPath} refreshToken={refreshToken} onNavigate={browse} onError={setError} />}
      </main>
      {clipboard && <div className="clipboard-bar">{clipboard.mode === 'copy' ? 'Kopieren' : 'Verschieben'}: {clipboard.paths.length} Eintrag/Einträge <span>Ctrl+V zum Einfügen</span><button onClick={() => setClipboard(null)}><X /></button></div>}
    </div>
    <SettingsModal open={settingsOpen} theme={theme} volumes={volumes} onTheme={setTheme} onChanged={loadVolumes} onError={setError} onClose={() => setSettingsOpen(false)} />
    <FirstRunModal open={onboardingOpen} volumes={volumes} onStart={startInitialIndex} onSkip={completeOnboarding} />
    {error && <div className="toast"><AlertTriangle /><span>{error}</span><button onClick={() => setError('')}><X /></button></div>}
  </div>
}
