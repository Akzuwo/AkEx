export function formatBytes(bytes?: number): string {
  if (bytes == null) return '—'
  if (bytes === 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB', 'PB']
  const index = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1)
  const value = bytes / 1024 ** index
  return `${new Intl.NumberFormat('de-CH', { maximumFractionDigits: index > 1 ? 1 : 0 }).format(value)} ${units[index]}`
}

export function formatCount(value?: number): string {
  return new Intl.NumberFormat('de-CH').format(value ?? 0)
}

export function formatDate(value?: string): string {
  if (!value) return '—'
  const date = new Date(value)
  return Number.isNaN(date.getTime()) ? '—' : new Intl.DateTimeFormat('de-CH', { dateStyle: 'short', timeStyle: 'short' }).format(date)
}

export function fileKind(extension?: string, isDirectory?: boolean): string {
  if (isDirectory) return 'Ordner'
  if (!extension) return 'Datei'
  return extension.toLocaleUpperCase()
}
