/**
 * Base fetch client and shared internal types/helpers used across all domain modules.
 *
 * Energy unit conversions applied here:
 *   Database stores energy in Joules.
 *   Frontend displays in Watt-hours:  Wh = J / 3600
 *
 * Water consumption estimate: 1.8 L/kWh (global data-centre average).
 * Carbon fallback: 445 gCO2/kWh (IEA Emissions Factors 2025 world average).
 */

import { Run, MetricRecord } from '../mock-data'
import { buildApiUrl } from '../data-source'

export { buildApiUrl }

export const WATER_FACTOR = 1.8 // L/kWh
export const WORLD_AVERAGE_INTENSITY_G_PER_KWH = 445 // IEA Emissions Factors 2025

// --- Shared backend response shapes ---

export interface BackendProject {
  id: number
  name: string
  workspace_slug: string
}

export interface BackendRun {
  id: number
  name: string
  project_id: number
  collaborator_name?: string | null
  status?: 'running' | 'finished' | 'failed' | 'canceled' | null
  started_at?: string | null
  ended_at?: string | null
  command?: string | null
  exporter_type?: string | null
  cpu_name?: string | null
  cpu_cores?: number | null
  ram_gb?: number | null
  gpu_count?: number | null
  gpu_names?: string | null
  total_cost?: number | null
  currency?: string | null
}

export interface BackendRunSummaryResponse {
  id: number
  name: string
  project_id: number
  project_name?: string | null
  collaborator_name?: string | null
  status?: 'running' | 'finished' | 'failed' | 'canceled' | null
  started_at?: string | null
  ended_at?: string | null
  command?: string | null
  exporter_type?: string | null
  total_cpu_energy?: number | null
  total_gpu_energy?: number | null
  total_mem_energy?: number | null
  total_igpu_energy?: number | null
  total_co2_g?: number | null
  carbon_intensity_used?: number | null
  pue_used?: number | null
  total_cost?: number | null
  currency_used?: string | null
  avg_cpu_usage?: number | null
  avg_gpu_usage?: number | null
  avg_mem_usage?: number | null
}

export interface BackendPaginatedRuns {
  runs: BackendRunSummaryResponse[]
  total: number
}

export interface BackendRecordPoint {
  id: number
  run_id: number
  pid: number
  timestamp: string
  cpu_usage: number
  cpu_energy: number
  gpu_usage: number
  gpu_energy: number
  mem_usage: number
  mem_energy: number
  igpu_usage: number
  igpu_energy: number
  gpu_details?: BackendGpuRecordPoint[]
  socket_details?: BackendSocketRecordPoint[]
  phase?: string | null
  epoch_num?: number | null
  tokens_processed?: number | null
  training_step?: number | null
  loss?: number | null
  learning_rate?: number | null
  sequence_length?: number | null
  accuracy?: number | null
  custom_metrics?: Record<string, number> | null
}

export interface BackendGpuRecordPoint {
  id: number
  record_id: number
  device_index: number
  gpu_usage: number
  gpu_energy: number
}

export interface BackendSocketRecordPoint {
  id: number
  record_id: number
  socket: number
  domain: string
  energy: number
}

export interface BackendRunSummary {
  run_id: number
  total_cpu_energy: number
  total_gpu_energy: number
  total_mem_energy: number
  total_igpu_energy: number
  total_co2_g?: number | null
  carbon_intensity_used?: number | null
  pue_used?: number | null
  total_cost?: number | null
  currency_used?: string | null
  avg_cpu_usage?: number | null
  avg_gpu_usage?: number | null
  avg_mem_usage?: number | null
}

export interface BackendAdminRun {
  run_id: number
  run_name: string
  collaborator_name: string | null
  status: 'running' | 'finished' | 'failed' | 'canceled'
  started_at: string | null
  ended_at: string | null
  last_record_at: string | null
  command: string | null
}

export interface BackendAdminProject {
  id: number
  name: string
  default_collaborator: string | null
  collaborators?: string[]
  active_runs?: BackendAdminRun[]
  latest_run: BackendAdminRun | null
}

// --- Shared snapshot types used across runs/projects ---

export interface RunMetricSnapshot {
  records: MetricRecord[]
  totalCpuEnergy: number
  totalGpuEnergy: number
  totalMemEnergy: number
  totalIgpuEnergy: number
  totalEnergy: number
  totalCo2G: number
  avgCpuUsage: number
  avgGpuUsage: number
  avgMemUsage: number
  duration: number
  timestamp: number
  carbonIntensityUsed: number | null
  pueUsed: number | null
  totalCost: number | null
  currency: string | null
}

export interface RunTotalsSnapshot {
  totalCpuEnergy: number
  totalGpuEnergy: number
  totalMemEnergy: number
  totalIgpuEnergy: number
  totalEnergy: number
  totalCo2G: number
  timestamp: number
  carbonIntensityUsed: number | null
  pueUsed: number | null
  totalCost: number | null
  currency: string | null
  avgCpuUsage: number
  avgGpuUsage: number
  avgMemUsage: number
}

// --- Pure transformation helpers ---

/** Parses an ISO timestamp string into a Unix millisecond value, or null if absent/invalid. */
export function parseTimestamp(value?: string | null): number | null {
  if (!value) return null
  const timestamp = new Date(value).getTime()
  return Number.isNaN(timestamp) ? null : timestamp
}

/** Normalises the backend run status to one of the frontend-known values. */
export function normalizeRunStatus(status?: BackendRun['status']): Run['status'] {
  return status === 'running' || status === 'failed' || status === 'canceled' ? status : 'finished'
}

/** Maps a backend record point (Joules, ISO timestamp) to the frontend MetricRecord type. */
export function mapRecord(r: BackendRecordPoint): MetricRecord {
  return {
    id: r.id.toString(),
    run_id: r.run_id.toString(),
    pid: r.pid,
    timestamp: new Date(r.timestamp).getTime(),
    cpu_usage: r.cpu_usage,
    cpu_energy: r.cpu_energy,
    gpu_usage: r.gpu_usage,
    gpu_energy: r.gpu_energy,
    mem_usage: r.mem_usage,
    mem_energy: r.mem_energy,
    igpu_usage: r.igpu_usage,
    igpu_energy: r.igpu_energy,
    gpu_details: (r.gpu_details ?? []).map(g => ({
      device_index: g.device_index,
      gpu_usage: g.gpu_usage,
      gpu_energy: g.gpu_energy,
    })),
    socket_details: (r.socket_details ?? []).map(s => ({
      socket: s.socket,
      domain: s.domain,
      energy: s.energy,
    })),
    phase: r.phase ?? null,
    epoch_num: r.epoch_num ?? null,
    tokens_processed: r.tokens_processed ?? null,
    training_step: r.training_step ?? null,
    loss: r.loss ?? null,
    learning_rate: r.learning_rate ?? null,
    sequence_length: r.sequence_length ?? null,
    accuracy: r.accuracy ?? null,
    custom_metrics: r.custom_metrics ?? null,
  }
}

/** Computes mean CPU, GPU, and memory utilization across all records in a run. */
export function calculateAverages(records: MetricRecord[]) {
  if (records.length === 0) {
    return { avgCpuUsage: 0, avgGpuUsage: 0, avgMemUsage: 0 }
  }
  return {
    avgCpuUsage: records.reduce((sum, r) => sum + r.cpu_usage, 0) / records.length,
    avgGpuUsage: records.reduce((sum, r) => sum + r.gpu_usage, 0) / records.length,
    avgMemUsage: records.reduce((sum, r) => sum + r.mem_usage, 0) / records.length,
  }
}

/**
 * Derives run duration in seconds.
 * Prefers `started_at` / `ended_at` from run metadata; falls back to record span.
 */
export function calculateDurationSeconds(
  records: MetricRecord[],
  startedAt?: string | null,
  endedAt?: string | null
) {
  const startedTimestamp = parseTimestamp(startedAt)
  const endedTimestamp = parseTimestamp(endedAt)

  if (startedTimestamp !== null && endedTimestamp !== null && endedTimestamp >= startedTimestamp) {
    return Math.round((endedTimestamp - startedTimestamp) / 1000)
  }

  if (records.length > 1) {
    return Math.round((records[records.length - 1].timestamp - records[0].timestamp) / 1000)
  }

  return records.length === 1 ? 60 : 0
}

/**
 * Converts Joule-based energy totals from the backend summary into Wh and
 * derives CO2 and cost figures.
 */
export function calculateEnergyTotals(summary: BackendRunSummary | null) {
  const totalCpuEnergy = (summary?.total_cpu_energy ?? 0) / 3600
  const totalGpuEnergy = (summary?.total_gpu_energy ?? 0) / 3600
  const totalMemEnergy = (summary?.total_mem_energy ?? 0) / 3600
  const totalIgpuEnergy = (summary?.total_igpu_energy ?? 0) / 3600
  const totalEnergy = totalCpuEnergy + totalGpuEnergy + totalMemEnergy + totalIgpuEnergy

  const totalCo2G = summary?.total_co2_g != null
    ? summary.total_co2_g
    : (totalEnergy / 1000) * WORLD_AVERAGE_INTENSITY_G_PER_KWH

  return {
    totalCpuEnergy,
    totalGpuEnergy,
    totalMemEnergy,
    totalIgpuEnergy,
    totalEnergy,
    totalCo2G,
    carbonIntensityUsed: summary?.carbon_intensity_used ?? null,
    pueUsed: summary?.pue_used ?? null,
    totalCost: summary?.total_cost ?? null,
    currency: summary?.currency_used ?? null,
    avgCpuUsage: summary?.avg_cpu_usage ?? 0,
    avgGpuUsage: summary?.avg_gpu_usage ?? 0,
    avgMemUsage: summary?.avg_mem_usage ?? 0,
  }
}

/** Thin wrapper around `fetch` that always sends session cookies and includes timeout handling. */
export async function apiFetch(url: string, init?: RequestInit) {
  const controller = new AbortController()
  const timeoutId = setTimeout(() => controller.abort(), 30000) // 30 second timeout

  try {
    const response = await fetch(url, {
      ...init,
      credentials: 'include',
      signal: controller.signal,
    })
    clearTimeout(timeoutId)
    return response
  } catch (error) {
    clearTimeout(timeoutId)
    if (error instanceof Error && error.name === 'AbortError') {
      throw new Error('Request timeout - please try again')
    }
    throw error
  }
}

// --- Internal snapshot fetchers (used by runs and projects) ---

export async function fetchRunMetricSnapshot(backendRun: BackendRun): Promise<RunMetricSnapshot> {
  const [summaryRes, recordsRes] = await Promise.all([
    apiFetch(buildApiUrl(`/api/run/${backendRun.id}/summary`)),
    apiFetch(buildApiUrl(`/api/run/${backendRun.id}/record_points`)),
  ])

  const summary: BackendRunSummary | null = summaryRes.ok ? await summaryRes.json() : null
  const backendRecords: BackendRecordPoint[] = recordsRes.ok ? await recordsRes.json() : []
  const records = backendRecords
    .map(mapRecord)
    .sort((left, right) => left.timestamp - right.timestamp)
  const energyTotals = calculateEnergyTotals(summary)
  const averages = calculateAverages(records)
  const timestamp =
    parseTimestamp(backendRun.started_at) ??
    parseTimestamp(backendRun.ended_at) ??
    records[0]?.timestamp ??
    Date.now()

  return {
    records,
    ...energyTotals,
    ...averages,
    duration: calculateDurationSeconds(records, backendRun.started_at, backendRun.ended_at),
    timestamp,
  }
}

export async function fetchRunTotalsSnapshot(backendRun: BackendRun): Promise<RunTotalsSnapshot> {
  const summaryRes = await apiFetch(buildApiUrl(`/api/run/${backendRun.id}/summary`))
  const summary: BackendRunSummary | null = summaryRes.ok ? await summaryRes.json() : null
  const energyTotals = calculateEnergyTotals(summary)
  const timestamp =
    parseTimestamp(backendRun.started_at) ??
    parseTimestamp(backendRun.ended_at) ??
    Date.now()

  return { ...energyTotals, timestamp }
}
