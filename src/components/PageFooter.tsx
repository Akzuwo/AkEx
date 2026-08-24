import { ChevronLeft, ChevronRight } from 'lucide-react'
import { formatCount } from '../utils/format'

export function PageFooter({ offset, limit, total, onPage }: { offset: number; limit: number; total: number; onPage: (offset: number) => void }) {
  const from = total ? offset + 1 : 0
  const to = Math.min(offset + limit, total)
  return <footer className="page-footer"><span>{formatCount(from)}–{formatCount(to)} von {formatCount(total)}</span><div><button disabled={offset === 0} onClick={() => onPage(Math.max(0, offset - limit))}><ChevronLeft /></button><button disabled={offset + limit >= total} onClick={() => onPage(offset + limit)}><ChevronRight /></button></div></footer>
}
