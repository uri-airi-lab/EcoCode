/**
 * useMetricsWorker — lifecycle hook for the metrics Web Worker.
 *
 * Instantiates the worker once per component mount and provides a
 * `compute` function that sends a request and returns a Promise resolved
 * by the matching response (matched by auto-incrementing id).
 *
 * SSR-safe: returns null for `compute` during server rendering.
 */

'use client'

import { useEffect, useRef, useCallback } from 'react'
import type { WorkerRequest, WorkerResponse } from '../workers/metrics-worker'

type ComputeFn = (req: Omit<WorkerRequest, 'id'>) => Promise<WorkerResponse>

export function useMetricsWorker(): ComputeFn | null {
  const workerRef = useRef<Worker | null>(null)
  const idRef = useRef(0)
  const pendingRef = useRef<Map<number, (res: WorkerResponse) => void>>(new Map())

  useEffect(() => {
    if (typeof window === 'undefined') return

    const worker = new Worker(
      new URL('../workers/metrics-worker.ts', import.meta.url),
      { type: 'module' }
    )

    worker.onmessage = (e: MessageEvent<WorkerResponse>) => {
      const resolve = pendingRef.current.get(e.data.id)
      if (resolve) {
        pendingRef.current.delete(e.data.id)
        resolve(e.data)
      }
    }

    workerRef.current = worker
    return () => {
      worker.terminate()
      workerRef.current = null
    }
  }, [])

  const compute = useCallback<ComputeFn>((req) => {
    return new Promise((resolve, reject) => {
      const worker = workerRef.current
      if (!worker) {
        reject(new Error('Worker not initialized'))
        return
      }
      const id = ++idRef.current
      pendingRef.current.set(id, resolve)
      worker.postMessage({ ...req, id } as WorkerRequest)
    })
  }, [])

  // Return null during SSR so callers fall back to synchronous computation
  if (typeof window === 'undefined') return null

  return compute
}
