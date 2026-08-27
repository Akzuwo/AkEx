export type IndexStatus = 'NotIndexed' | 'Indexing' | 'Ready' | 'OutOfDate' | 'Error'

export interface Volume {
  id: number
  volumeId: string
  rootPath: string
  label?: string
  filesystemType?: string
  totalBytes?: number
  freeBytes?: number
  lastFullScan?: string
  indexStatus: IndexStatus
  entryCount: number
  lastError?: string
}

export interface Entry {
  id: number
  parentId?: number
  volumeId: number
  name: string
  fullPath: string
  extension?: string
  isDirectory: boolean
  size: number
  recursiveSize: number
  createdAt?: string
  modifiedAt?: string
  hidden: boolean
  readOnly: boolean
  system: boolean
}

export interface Page<T> { items: T[]; total: number; offset: number; limit: number }

export type EntrySortField = 'name' | 'type' | 'size' | 'modified'
export type SortDirection = 'asc' | 'desc'
export type FileViewMode = 'extraLarge' | 'large' | 'medium' | 'small' | 'list' | 'details' | 'tiles' | 'content'
export type FilePaneMode = 'none' | 'details' | 'preview'

export interface FilePreview {
  kind: 'image' | 'text' | 'pdf' | 'audio' | 'video' | 'unavailable'
  mimeType?: string
  data?: string
  text?: string
  message?: string
}

export interface ScanProgress {
  scanId: string
  rootPath: string
  entriesFound: number
  bytesFound: number
  currentPath: string
  percent?: number
  phase: 'Scanning' | 'Complete'
  errors: number
}

export interface ExtensionUsage { extension: string; bytes: number; count: number }
export interface StorageAnalysis {
  totalBytes: number
  fileCount: number
  folderCount: number
  largestFolders: Entry[]
  largestFiles: Entry[]
  extensions: ExtensionUsage[]
}

export interface VerificationResult {
  ok: boolean
  integrityMessage: string
  orphanCount: number
  sizeMismatchCount: number
}

export interface PathProperties {
  path: string
  isDirectory: boolean
  size: number
  createdAt?: string
  modifiedAt?: string
  readOnly: boolean
}

export interface AppCommandError { code?: string; message?: string }
export type View = 'browser' | 'search' | 'analysis'
export type Theme = 'dark' | 'light'
export type ClipboardOperation = { mode: 'copy' | 'move'; paths: string[] } | null
