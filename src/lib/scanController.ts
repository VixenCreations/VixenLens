// src/lib/scanController.ts
import { writable } from 'svelte/store'
import { scanAndRegisterImagesWithProgress } from '$lib/api'

export type ScanProgressState = {
  scanId: number | null
  processed: number
  total: number
  done: boolean
  cancelled: boolean
}

const initialState: ScanProgressState = {
  scanId: null,
  processed: 0,
  total: 0,
  done: true,
  cancelled: false,
}

export const scanProgress = writable<ScanProgressState>({ ...initialState })

let scanCounter = 0
let running = false

/**
 * Starts a scan and tracks its lifecycle.
 * This is the ONLY place that talks to the scan API.
 */
export async function startScan(folders: string[]) {
  if (running) return

  running = true
  const scanId = ++scanCounter

  scanProgress.set({
    scanId,
    processed: 0,
    total: 0,
    done: false,
    cancelled: false,
  })

  try {
		await scanAndRegisterImagesWithProgress(
			(processed, total, message) => {
				scanProgress.update((p) => ({
					...p,
					processed: processed ?? p.processed,
					total: total ?? p.total,
				}))
			},
			folders
		)

    scanProgress.update((p) => ({
      ...p,
      done: true,
    }))
  } catch (err) {
    scanProgress.update((p) => ({
      ...p,
      done: true,
      cancelled: true,
    }))
    throw err
  } finally {
    running = false
  }
}

/**
 * Optional reset helper
 */
export function resetScanProgress() {
  scanProgress.set({ ...initialState })
}
