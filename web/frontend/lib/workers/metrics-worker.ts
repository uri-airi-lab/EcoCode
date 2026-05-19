/**
 * Web Worker: LTTB downsampling + watts computation, off the main thread.
 *
 * Runs `computeWatts` + `lttbSample` for energy/utilization charts so the
 * UI never freezes during large data transformations.
 *
 * Message protocol:
 *   Request:  { id, type: 'LTTB_WATTS' | 'LTTB_UTIL', records, threshold }
 *   Response: { id, type, ...columnar arrays }
 *
 * The response is columnar (separate typed arrays per series) so it can be
 * passed directly to uPlot as AlignedData with zero allocation overhead.
 */

import { lttbSample } from '../lttb'
import type { MetricRecord } from '../mock-data'

// ── Types ────────────────────────────────────────────────────────────────────

type RecordW = MetricRecord & {
  cpuW: number; gpuW: number; memW: number; igpuW: number
}

export type WorkerRequest =
  | { id: number; type: 'LTTB_WATTS'; records: MetricRecord[]; threshold: number }
  | { id: number; type: 'LTTB_UTIL';  records: MetricRecord[]; threshold: number }

export type WorkerResponse =
  | {
      id: number; type: 'LTTB_WATTS'
      timestamps: number[]; cpuW: number[]; gpuW: number[]; memW: number[]; igpuW: number[]
    }
  | {
      id: number; type: 'LTTB_UTIL'
      timestamps: number[]; cpu: number[]; gpu: number[]; mem: number[]
    }

// ── Helpers ───────────────────────────────────────────────────────────────────

function computeWatts(records: MetricRecord[]): RecordW[] {
  return records.map((r, i) => {
    // Prefer backend-provided elapsed_secs (monotonic, matches energy deltas exactly).
    // Fall back to timestamp diff for older records that lack the field.
    const prev = i > 0 ? records[i - 1] : null
    const deltaS = r.elapsed_secs
      ? Math.max(r.elapsed_secs, 0.001)
      : prev ? Math.max((r.timestamp - prev.timestamp) / 1000, 0.001) : 1
    return {
      ...r,
      cpuW:  (r.cpu_energy  ?? 0) / deltaS,
      gpuW:  (r.gpu_energy  ?? 0) / deltaS,
      memW:  (r.mem_energy  ?? 0) / deltaS,
      igpuW: (r.igpu_energy ?? 0) / deltaS,
    }
  })
}

// ── Message handler ───────────────────────────────────────────────────────────

self.onmessage = (e: MessageEvent<WorkerRequest>) => {
  const req = e.data

  if (req.type === 'LTTB_WATTS') {
    const withW = computeWatts(req.records)
    const sampled = lttbSample(withW, req.threshold, r => r.cpuW + r.gpuW + r.memW + r.igpuW)
    const resp: WorkerResponse = {
      id: req.id,
      type: 'LTTB_WATTS',
      timestamps: sampled.map(r => r.timestamp),
      cpuW:  sampled.map(r => Number(r.cpuW.toFixed(2))),
      gpuW:  sampled.map(r => Number(r.gpuW.toFixed(2))),
      memW:  sampled.map(r => Number(r.memW.toFixed(2))),
      igpuW: sampled.map(r => Number(r.igpuW.toFixed(2))),
    }
    self.postMessage(resp)
    return
  }

  if (req.type === 'LTTB_UTIL') {
    const sampled = lttbSample(req.records, req.threshold, r => r.cpu_usage + r.gpu_usage)
    const resp: WorkerResponse = {
      id: req.id,
      type: 'LTTB_UTIL',
      timestamps: sampled.map(r => r.timestamp),
      cpu: sampled.map(r => Number((r.cpu_usage ?? 0).toFixed(2))),
      gpu: sampled.map(r => Number((r.gpu_usage ?? 0).toFixed(2))),
      mem: sampled.map(r => Number((r.mem_usage ?? 0).toFixed(2))),
    }
    self.postMessage(resp)
  }
}
