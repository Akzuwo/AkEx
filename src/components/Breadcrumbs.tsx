import { ChevronRight, HardDrive } from 'lucide-react'

export function Breadcrumbs({ path, onNavigate }: { path: string; onNavigate: (path: string) => void }) {
  if (!path) return <div className="breadcrumbs"><span>Kein Pfad ausgewählt</span></div>
  const root = path.slice(0, 3)
  const parts = path.slice(3).split('\\').filter(Boolean)
  let accumulated = root
  return <div className="breadcrumbs">
    <button onClick={() => onNavigate(root)}><HardDrive size={14} />{root.slice(0, 2)}</button>
    {parts.map(part => {
      accumulated = `${accumulated}${accumulated.endsWith('\\') ? '' : '\\'}${part}`
      const target = accumulated
      return <span className="crumb" key={target}><ChevronRight size={14} /><button onClick={() => onNavigate(target)}>{part}</button></span>
    })}
  </div>
}
