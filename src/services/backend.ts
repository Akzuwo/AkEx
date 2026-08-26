import { invoke } from '@tauri-apps/api/core'
import type { Entry, EntrySortField, FilePreview, Page, PathProperties, SortDirection, StorageAnalysis, VerificationResult, Volume } from '../types'

export const backend = {
  volumes: () => invoke<Volume[]>('list_volumes'),
  directory: (path: string, offset = 0, limit = 300, sortField: EntrySortField = 'name', sortDirection: SortDirection = 'asc') =>
    invoke<Page<Entry>>('list_directory', { path, offset, limit, sortField, sortDirection }),
  entry: (path: string) => invoke<Entry | null>('get_entry', { path }),
  search: (query: string, offset = 0, limit = 300, sortField: EntrySortField = 'modified', sortDirection: SortDirection = 'desc') =>
    invoke<Page<Entry>>('search_entries', { query, offset, limit, sortField, sortDirection }),
  analyze: (path: string, limit = 20) => invoke<StorageAnalysis>('analyze_storage', { path, limit }),
  startIndex: (rootPath: string) => invoke<string>('start_index', { rootPath }),
  cancelIndex: (scanId: string) => invoke<boolean>('cancel_index', { scanId }),
  verifyIndex: (volumeId: number) => invoke<VerificationResult>('verify_index', { volumeId }),
  startWatchers: () => invoke<void>('start_watchers'),
  open: (path: string) => invoke<void>('open_path', { path }),
  reveal: (path: string) => invoke<void>('reveal_path', { path }),
  openWindow: (path: string) => invoke<void>('open_window', { path }),
  validateDragPaths: (paths: string[]) => invoke<void>('validate_drag_paths', { paths }),
  properties: (path: string) => invoke<PathProperties>('path_properties', { path }),
  preview: (path: string) => invoke<FilePreview>('preview_file', { path }),
  createFolder: (parent: string, name: string) => invoke<Entry>('create_folder', { parent, name }),
  rename: (path: string, newName: string) => invoke<Entry>('rename_entry', { path, newName }),
  remove: (paths: string[]) => invoke<void>('delete_entries', { paths }),
  copy: (sources: string[], destination: string) => invoke<void>('copy_entries', { sources, destination }),
  move: (sources: string[], destination: string) => invoke<void>('move_entries', { sources, destination }),
  refresh: (path: string) => invoke<void>('refresh_directory', { path }),
}

export function errorMessage(error: unknown): string {
  if (typeof error === 'string') return error
  if (error && typeof error === 'object' && 'message' in error) return String(error.message)
  return 'Die Aktion ist fehlgeschlagen.'
}
