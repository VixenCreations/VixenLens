import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { Config } from '$lib/config'

/**
 * ─────────────────────────────────────────────────────────────
 * Large-scan readiness: file list caching + safe invalidation
 * ─────────────────────────────────────────────────────────────
 *
 * Problem:
 *   getThumbnailsChunk() was invoking `search_files_in_folders` every time,
 *   which forces huge IPC payloads for large datasets and will time out / stall.
 *
 * Fix:
 *   - Cache the full list once (still expensive, but only once per session)
 *   - Slice locally for pagination
 *   - Provide invalidateFileListCache() for scan/folder changes
 *   - Opportunistically try server-side pagination if available (non-breaking)
 */

export type FileEntry = { path: string; uuid: string }

let _fileListCache: FileEntry[] | null = null
let _fileListPromise: Promise<FileEntry[]> | null = null
let _fileListCacheVersion = 0

/** Call this when folders change or when a scan completes. */
export function invalidateFileListCache(): void {
  _fileListCache = null
  _fileListPromise = null
  _fileListCacheVersion++
}

/**
 * Attempts to fetch a paged slice from backend if you add a command later.
 * Falls back to cached full-list behavior without breaking current builds.
 */
async function tryGetFilesPageFromBackend(
  offset: number,
  limit: number
): Promise<FileEntry[] | null> {
  try {
    // Optional future command (not required today).
    // If it doesn't exist, invoke will throw and we’ll return null.
    return await invoke<FileEntry[]>('get_files_page', { offset, limit })
  } catch {
    return null
  }
}

/** Fetches and caches the full file list exactly once per cache version. */
async function getAllFilesCached(): Promise<FileEntry[]> {
  if (_fileListCache) return _fileListCache
  if (_fileListPromise) return _fileListPromise

  const myVersion = _fileListCacheVersion

  _fileListPromise = (async () => {
    const list = await invoke<FileEntry[]>('search_files_in_folders')
    // If cache was invalidated while we were fetching, discard results.
    if (myVersion !== _fileListCacheVersion) {
      return list
    }
    _fileListCache = list
    return list
  })().finally(() => {
    // Only clear the promise if we’re still on the same version
    // (avoids stomping a newer in-flight request).
    if (myVersion === _fileListCacheVersion) {
      _fileListPromise = null
    }
  })

  return _fileListPromise
}

// 初期設定済みかどうかを確認
export async function getInitialSetupState(): Promise<boolean> {
  const folders = await invoke<FileEntry[]>('get_all_folders')
  console.log(folders)
  return folders.length > 0
}

// フォルダを追加
export async function addFolder(path: string): Promise<void> {
  await invoke('add_folder', { path })
  invalidateFileListCache()
}

export async function deleteFolder(id: number): Promise<void> {
  await invoke('delete_folder', { id })
  invalidateFileListCache()
}

export async function addIgnoreFolder(path: string): Promise<void> {
  await invoke('add_ignore_folder', { path })
  // ignore folders affect scan results, so invalidate
  invalidateFileListCache()
}

export async function deleteIgnoreFolder(id: number): Promise<void> {
  await invoke('delete_ignore_folder', { id })
  invalidateFileListCache()
}

export async function getAllFolders(): Promise<{ id: number; path: string }[]> {
  return await invoke<{ id: number; path: string }[]>('get_all_folders')
}

export async function getAllIgnoreFolders(): Promise<
  { id: number; path: string }[]
> {
  return await invoke<{ id: number; path: string }[]>('get_all_ignore_folders')
}

/**
 * Scan + progress
 * Fixes:
 *  - guarantees unlisten always runs
 *  - uses await listen(...) correctly
 *  - invalidates file list cache when scan completes (so thumbnails refresh)
 */

export async function scanAndRegisterImagesWithProgress(
  eventCallback: (processed: number, total: number, message: string) => void,
  folderList: Array<string>
): Promise<void> {
  const event_id = Date.now().toString()

  invalidateFileListCache()

  const unlisten = await listen('scan_progress', (event) => {
    const payload = event.payload as {
      event_id: string
      processed?: number
      total?: number
      progress?: number // legacy
      message: string
    }

    if (payload.event_id !== event_id) return

    const processed = payload.processed ?? payload.progress ?? 0
    const total = payload.total ?? 0

    eventCallback(processed, total, payload.message)
  })

  try {
    await invoke('scan_and_register_images_with_progress', {
      folderList,
      eventId: event_id,
    })
  } finally {
    unlisten()
    invalidateFileListCache()
  }
}

/**
 * Thumbnails paging (READ-ONLY)
 *
 * Reads pre-generated thumbnails from the unified index DB (imageCache).
 * No UUIDs, no per-folder DBs, no thumbnail regeneration.
 */
export async function getThumbnailsChunk(
  offset: number,
  limit: number
): Promise<[string, string, string][]> {
  return await invoke<[string, string, string][]>('get_thumbnails_chunk', {
    offset,
    limit,
  })
}

export async function getMetadata(
  dbid: string,
  filePath: string
): Promise<any> {
  const response = await invoke<{
    metadata: any
    data_url: string
    file_created_at: string
  }>('get_image_metadata', { uuid: dbid, filePath })

  return {
    metadata: response.metadata,
    data_url: response.data_url,
    file_created_at: response.file_created_at,
  }
}

export async function setConfig(config: Object): Promise<void> {
  await invoke('set_config', { config })
}

export async function getConfig(): Promise<Config> {
  return await invoke<Config>('get_config')
}



export async function searchImage(
  conditions: Array<any>
): Promise<Array<Object>> {
  return await invoke('search_images', { conditions })
}
