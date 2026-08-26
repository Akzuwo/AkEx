import { startDrag } from '@crabnebula/tauri-plugin-drag'

const iconCache = new Map<string, string>()

function previewIcon(count: number, folder: boolean): string {
  const key = `${folder ? 'folder' : 'file'}-${count > 1 ? 'multi' : 'single'}`
  const cached = iconCache.get(key)
  if (cached) return cached

  const canvas = document.createElement('canvas')
  canvas.width = 96
  canvas.height = 96
  const context = canvas.getContext('2d')
  if (!context) throw new Error('Die Drag-Vorschau konnte nicht erstellt werden.')

  context.shadowColor = '#0008'
  context.shadowBlur = 12
  context.shadowOffsetY = 5
  context.fillStyle = '#18202c'
  context.beginPath()
  context.roundRect(8, 8, 72, 72, 14)
  context.fill()
  context.shadowColor = 'transparent'

  if (folder) {
    context.fillStyle = '#e8b762'
    context.beginPath()
    context.roundRect(19, 31, 50, 34, 6)
    context.fill()
    context.beginPath()
    context.roundRect(22, 25, 24, 13, 5)
    context.fill()
  } else {
    context.fillStyle = '#85a9d8'
    context.beginPath()
    context.roundRect(25, 19, 40, 50, 6)
    context.fill()
    context.fillStyle = '#dce9f8'
    context.fillRect(33, 34, 24, 3)
    context.fillRect(33, 43, 24, 3)
    context.fillRect(33, 52, 17, 3)
  }

  if (count > 1) {
    context.fillStyle = '#367ee8'
    context.beginPath()
    context.arc(73, 72, 17, 0, Math.PI * 2)
    context.fill()
    context.fillStyle = '#fff'
    context.font = '700 17px system-ui'
    context.textAlign = 'center'
    context.textBaseline = 'middle'
    context.fillText(count > 99 ? '99+' : String(count), 73, 72)
  }

  const value = canvas.toDataURL('image/png')
  iconCache.set(key, value)
  return value
}

export async function startNativeFileDrag(paths: string[], folder: boolean, copy: boolean): Promise<boolean> {
  let dropped = false
  await startDrag({ item: paths, icon: previewIcon(paths.length, folder), mode: copy ? 'copy' : 'move' }, event => {
    dropped = event.result === 'Dropped'
  })
  return dropped
}
