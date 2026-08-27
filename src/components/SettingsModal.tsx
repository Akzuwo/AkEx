import { Moon, Settings, Sun, X } from 'lucide-react'
import { useEffect } from 'react'
import { IndexPage } from '../pages/IndexPage'
import type { Theme, Volume } from '../types'

interface Props {
  open: boolean
  theme: Theme
  volumes: Volume[]
  onTheme: (theme: Theme) => void
  onChanged: () => void
  onError: (message: string) => void
  onClose: () => void
}

export function SettingsModal({ open, theme, volumes, onTheme, onChanged, onError, onClose }: Props) {
  useEffect(() => {
    if (!open) return
    const close = (event: KeyboardEvent) => { if (event.key === 'Escape') onClose() }
    window.addEventListener('keydown', close)
    return () => window.removeEventListener('keydown', close)
  }, [open, onClose])
  if (!open) return null

  return <div className="modal-backdrop" role="presentation" onMouseDown={event => { if (event.target === event.currentTarget) onClose() }}>
    <section className="modal settings-modal" role="dialog" aria-modal="true" aria-labelledby="settings-title">
      <header className="modal-header"><div><Settings /><div><h1 id="settings-title">Einstellungen</h1><p>Darstellung und Index verwalten</p></div></div><button className="modal-close" aria-label="Einstellungen schliessen" onClick={onClose}><X /></button></header>
      <div className="modal-scroll">
        <section className="settings-section appearance-setting">
          <div className="settings-section-copy"><span className="settings-section-icon">{theme === 'light' ? <Sun /> : <Moon />}</span><div><h2>Darstellung</h2><p>Zwischen dem standardmässigen Dark Mode und dem hellen Erscheinungsbild wechseln.</p></div></div>
          <label className="theme-switch"><span>{theme === 'light' ? 'Light Mode' : 'Dark Mode'}</span><input type="checkbox" checked={theme === 'light'} onChange={event => onTheme(event.target.checked ? 'light' : 'dark')} /><i aria-hidden="true"><Sun /><Moon /></i></label>
        </section>
        <IndexPage embedded volumes={volumes} onChanged={onChanged} onError={onError} />
      </div>
    </section>
  </div>
}
