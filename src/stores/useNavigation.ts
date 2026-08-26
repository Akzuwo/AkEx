import { useCallback, useState } from 'react'

interface NavigationTab { id: string; history: string[]; index: number }
interface NavigationState { tabs: NavigationTab[]; activeId: string }

let tabSequence = 0
const createTab = (path = ''): NavigationTab => ({
  id: `tab-${++tabSequence}`,
  history: path ? [path] : [],
  index: path ? 0 : -1,
})

export function useNavigation(initialPath = '') {
  const [state, setState] = useState<NavigationState>(() => {
    const tab = createTab(initialPath)
    return { tabs: [tab], activeId: tab.id }
  })
  const activeTab = state.tabs.find(tab => tab.id === state.activeId) ?? state.tabs[0]
  const currentPath = activeTab?.index >= 0 ? activeTab.history[activeTab.index] : ''
  const updateActive = useCallback((update: (tab: NavigationTab) => NavigationTab) => {
    setState(previous => ({
      ...previous,
      tabs: previous.tabs.map(tab => tab.id === previous.activeId ? update(tab) : tab),
    }))
  }, [])
  const navigate = useCallback((path: string) => updateActive(tab => {
    if (!path || path.toLocaleLowerCase() === tab.history[tab.index]?.toLocaleLowerCase()) return tab
    const history = [...tab.history.slice(0, tab.index + 1), path]
    return { ...tab, history, index: history.length - 1 }
  }), [updateActive])
  const back = useCallback(() => updateActive(tab => ({ ...tab, index: Math.max(0, tab.index - 1) })), [updateActive])
  const forward = useCallback(() => updateActive(tab => ({ ...tab, index: Math.min(tab.history.length - 1, tab.index + 1) })), [updateActive])
  const up = useCallback(() => {
    if (!currentPath || /^.:\\$/.test(currentPath)) return
    const parent = currentPath.replace(/\\[^\\]+$/, '') || `${currentPath.slice(0, 2)}\\`
    navigate(parent)
  }, [currentPath, navigate])
  const openTab = useCallback((path = '') => {
    const tab = createTab(path)
    setState(previous => ({ tabs: [...previous.tabs, tab], activeId: tab.id }))
  }, [])
  const closeTab = useCallback((id: string) => setState(previous => {
    const closingIndex = previous.tabs.findIndex(tab => tab.id === id)
    if (closingIndex < 0) return previous
    if (previous.tabs.length === 1) return previous
    const tabs = previous.tabs.filter(tab => tab.id !== id)
    const activeId = previous.activeId === id
      ? tabs[Math.min(closingIndex, tabs.length - 1)].id
      : previous.activeId
    return { tabs, activeId }
  }), [])
  const activateTab = useCallback((id: string) => setState(previous => (
    previous.tabs.some(tab => tab.id === id) ? { ...previous, activeId: id } : previous
  )), [])
  return {
    currentPath, navigate, back, forward, up,
    tabs: state.tabs.map(tab => ({ id: tab.id, path: tab.index >= 0 ? tab.history[tab.index] : '' })),
    activeTabId: state.activeId,
    openTab, closeTab, activateTab,
    canBack: Boolean(activeTab && activeTab.index > 0),
    canForward: Boolean(activeTab && activeTab.index >= 0 && activeTab.index < activeTab.history.length - 1),
    canUp: Boolean(currentPath && !/^.:\\$/.test(currentPath)),
  }
}
