import { useCallback, useState } from 'react'

interface NavigationState { history: string[]; index: number }

export function useNavigation(initialPath = '') {
  const [state, setState] = useState<NavigationState>({ history: initialPath ? [initialPath] : [], index: initialPath ? 0 : -1 })
  const currentPath = state.index >= 0 ? state.history[state.index] : ''
  const navigate = useCallback((path: string) => setState(previous => {
    if (!path || path.toLocaleLowerCase() === previous.history[previous.index]?.toLocaleLowerCase()) return previous
    const history = [...previous.history.slice(0, previous.index + 1), path]
    return { history, index: history.length - 1 }
  }), [])
  const back = useCallback(() => setState(previous => ({ ...previous, index: Math.max(0, previous.index - 1) })), [])
  const forward = useCallback(() => setState(previous => ({ ...previous, index: Math.min(previous.history.length - 1, previous.index + 1) })), [])
  const up = useCallback(() => {
    if (!currentPath || /^.:\\$/.test(currentPath)) return
    const parent = currentPath.replace(/\\[^\\]+$/, '') || `${currentPath.slice(0, 2)}\\`
    navigate(parent)
  }, [currentPath, navigate])
  return {
    currentPath, navigate, back, forward, up,
    canBack: state.index > 0,
    canForward: state.index >= 0 && state.index < state.history.length - 1,
    canUp: Boolean(currentPath && !/^.:\\$/.test(currentPath)),
  }
}
