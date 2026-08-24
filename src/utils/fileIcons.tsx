import { Archive, File, FileAudio, FileCode2, FileImage, FileText, FileVideo, Folder, PackageOpen, type LucideIcon } from 'lucide-react'

const groups: Array<[Set<string>, LucideIcon]> = [
  [new Set(['jpg', 'jpeg', 'png', 'gif', 'webp', 'svg', 'bmp', 'tiff', 'raw']), FileImage],
  [new Set(['mp4', 'mkv', 'mov', 'avi', 'webm', 'wmv']), FileVideo],
  [new Set(['mp3', 'wav', 'flac', 'aac', 'm4a', 'ogg']), FileAudio],
  [new Set(['zip', '7z', 'rar', 'tar', 'gz', 'bz2']), Archive],
  [new Set(['exe', 'msi', 'appx', 'bat', 'cmd']), PackageOpen],
  [new Set(['pdf', 'doc', 'docx', 'txt', 'md', 'rtf', 'odt']), FileText],
  [new Set(['rs', 'ts', 'tsx', 'js', 'jsx', 'py', 'cs', 'cpp', 'h', 'json', 'toml', 'yaml', 'html', 'css']), FileCode2],
]

export function iconFor(extension?: string, isDirectory?: boolean): LucideIcon {
  if (isDirectory) return Folder
  const normalized = extension?.toLocaleLowerCase() ?? ''
  return groups.find(([extensions]) => extensions.has(normalized))?.[1] ?? File
}
