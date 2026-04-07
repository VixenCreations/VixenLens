// src/stores/statusStore.ts
import { writable } from 'svelte/store'

export type StatusType = 'info' | 'success' | 'error'

export interface StatusState {
  message: string
  progress: number | null
  type: StatusType
  isVisible: boolean
}

/**
 * Central UI status store.
 * Presentation-only. No side effects.
 */
export const statusStore = writable<StatusState>({
  message: '',
  progress: null,
  type: 'info',
  isVisible: true,
})

/* ─────────────────────────────────────────────
   Helper setters
   ───────────────────────────────────────────── */

export function setInfo(message: string, progress: number | null = null) {
  statusStore.set({
    message,
    progress,
    type: 'info',
    isVisible: true,
  })
}

export function setSuccess(message: string) {
  statusStore.set({
    message,
    progress: null,
    type: 'success',
    isVisible: true,
  })
}

export function setError(message: string) {
  statusStore.set({
    message,
    progress: null,
    type: 'error',
    isVisible: true,
  })
}

export function clearStatus() {
  statusStore.set({
    message: '',
    progress: null,
    type: 'info',
    isVisible: false,
  })
}

/* ─────────────────────────────────────────────
   Scan progress adapter
   ───────────────────────────────────────────── */

export interface ScanProgressEvent {
  scanId: number | null
  processed: number
  total: number
  done: boolean
  cancelled: boolean
}

/**
 * Maps scan progress → UI state.
 * Messages must be provided by the caller (i18n-safe).
 */
export function updateFromScanProgress(
  p: ScanProgressEvent | null,
  messages: {
    scanning: (processed: number, total: number) => string
    completed: string
    cancelled: string
  }
) {
  if (!p || p.scanId === null) return

  if (p.cancelled) {
    setError(messages.cancelled)
    return
  }

  if (p.done) {
    setSuccess(messages.completed)
    return
  }

  const percent =
    p.total > 0 ? Math.round((p.processed / p.total) * 100) : null

  setInfo(messages.scanning(p.processed, p.total), percent)
}
