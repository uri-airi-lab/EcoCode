/**
 * Run-level API — fetch individual runs and their record points.
 */

import { Run, MetricRecord } from '../mock-data'
import {
  apiFetch,
  buildApiUrl,
  BackendRun,
  BackendRecordPoint,
  BackendRunSummary,
  WATER_FACTOR,
  WORLD_AVERAGE_INTENSITY_G_PER_KWH,
  parseTimestamp,
  normalizeRunStatus,
  mapRecord,
  calculateAverages,
  calculateDurationSeconds,
  calculateEnergyTotals,
  fetchRunMetricSnapshot,
  fetchRunTotalsSnapshot,
} from './client'

export interface RunSummary extends Omit<Run, 'records'> {}
export interface RunDetail extends Run {}

export interface Improvement {
  id: string
  pid: number
  timestamp: number
  cpu_usage: number
  cpu_energy: number
  gpu_usage: number
  gpu_energy: number
  mem_usage: number
  mem_energy: number
  igpu_usage: number
  igpu_energy: number
}

// Uses only the summary endpoint — no records fetch, so safe to call for many runs at once.
export async function mapRunSummary(backendRun: BackendRun): Promise<RunSummary> {
  const snapshot = await fetchRunTotalsSnapshot(backendRun)
  const energyKwh = snapshot.totalEnergy / 1000
  const duration = calculateDurationSeconds([], backendRun.started_at, backendRun.ended_at)

  return {
    id: backendRun.id.toString(),
    projectId: backendRun.project_id.toString(),
    name: backendRun.name,
    timestamp: snapshot.timestamp,
    status: normalizeRunStatus(backendRun.status),
    collaboratorName: backendRun.collaborator_name ?? null,
    totalEnergy: snapshot.totalEnergy,
    totalCpuEnergy: snapshot.totalCpuEnergy,
    totalGpuEnergy: snapshot.totalGpuEnergy,
    totalMemEnergy: snapshot.totalMemEnergy,
    totalIgpuEnergy: snapshot.totalIgpuEnergy,
    totalCoreHours: energyKwh * 24,
    avgCpuUsage: 0,
    avgGpuUsage: 0,
    avgMemUsage: 0,
    carbonFootprint: snapshot.totalCo2G,
    waterConsumption: energyKwh * WATER_FACTOR * 1000,
    duration,
    command: backendRun.command ?? null,
    carbonIntensityUsed: snapshot.carbonIntensityUsed,
    pueUsed: snapshot.pueUsed,
    totalCost: snapshot.totalCost,
    currency: snapshot.currency,
  }
}

export async function mapRun(backendRun: BackendRun): Promise<Run> {
  const snapshot = await fetchRunMetricSnapshot(backendRun)
  const energyKwh = snapshot.totalEnergy / 1000

  return {
    id: backendRun.id.toString(),
    projectId: backendRun.project_id.toString(),
    name: backendRun.name,
    timestamp: snapshot.timestamp,
    status: normalizeRunStatus(backendRun.status),
    collaboratorName: backendRun.collaborator_name ?? null,
    totalEnergy: snapshot.totalEnergy,
    totalCpuEnergy: snapshot.totalCpuEnergy,
    totalGpuEnergy: snapshot.totalGpuEnergy,
    totalMemEnergy: snapshot.totalMemEnergy,
    totalIgpuEnergy: snapshot.totalIgpuEnergy,
    totalCoreHours: energyKwh * 24,
    avgCpuUsage: snapshot.avgCpuUsage,
    avgGpuUsage: snapshot.avgGpuUsage,
    avgMemUsage: snapshot.avgMemUsage,
    carbonFootprint: snapshot.totalCo2G,
    waterConsumption: energyKwh * WATER_FACTOR * 1000,
    duration: snapshot.duration,
    command: backendRun.command ?? null,
    carbonIntensityUsed: snapshot.carbonIntensityUsed,
    pueUsed: snapshot.pueUsed,
    totalCost: snapshot.totalCost,
    currency: snapshot.currency,
    systemInfo: backendRun.cpu_name ? {
      cpuName: backendRun.cpu_name,
      cpuCores: backendRun.cpu_cores ?? 0,
      ramGb: backendRun.ram_gb ?? 0,
      gpuCount: backendRun.gpu_count ?? 0,
      gpuNames: backendRun.gpu_names ?? '',
    } : null,
    records: snapshot.records,
  }
}

// Returns run summaries (no records). Accepts an already-fetched project id list
// to avoid a duplicate /api/projects call when the caller already has it.
export async function fetchAllRuns(projectIds?: number[]): Promise<RunSummary[]> {
  try {
    let ids: number[]
    if (projectIds) {
      ids = projectIds
    } else {
      const projectsRes = await apiFetch(buildApiUrl('/api/projects'))
      if (projectsRes.status === 404) return []
      if (!projectsRes.ok) throw new Error(`Failed to fetch projects: ${projectsRes.status}`)
      const projects = await projectsRes.json()
      ids = projects.map((p: { id: number }) => p.id)
    }

    const projectRunsArrays = await Promise.all(
      ids.map(async (id) => {
        const runsRes = await apiFetch(buildApiUrl(`/api/project/${id}/runs`))
        if (!runsRes.ok) return []
        const backendRuns: BackendRun[] = await runsRes.json()
        return Promise.all(backendRuns.map(mapRunSummary))
      })
    )

    return projectRunsArrays.flat().sort((a, b) => b.timestamp - a.timestamp)
  } catch (error) {
    throw error
  }
}

export async function fetchAllRunsWithRecords(
  projectIds?: string[]
): Promise<{ runId: string; runName: string; records: MetricRecord[] }[]> {
  const projectsRes = await apiFetch(buildApiUrl('/api/projects'))
  if (!projectsRes.ok) return []
  const projects = await projectsRes.json()

  const filtered = projectIds
    ? projects.filter((p: { id: number }) => projectIds.includes(p.id.toString()))
    : projects

  const results: { runId: string; runName: string; records: MetricRecord[] }[] = []

  await Promise.all(
    filtered.map(async (project: { id: number }) => {
      const runsRes = await apiFetch(buildApiUrl(`/api/project/${project.id}/runs`))
      if (!runsRes.ok) return
      const backendRuns: BackendRun[] = await runsRes.json()

      await Promise.all(
        backendRuns.map(async (backendRun) => {
          const recordsRes = await apiFetch(
            buildApiUrl(`/api/run/${backendRun.id}/record_points`)
          )
          const backendRecords: BackendRecordPoint[] = recordsRes.ok
            ? await recordsRes.json()
            : []
          const records = backendRecords
            .map(mapRecord)
            .sort((a, b) => a.timestamp - b.timestamp)
          results.push({
            runId: backendRun.id.toString(),
            runName: backendRun.name,
            records,
          })
        })
      )
    })
  )

  results.sort((a, b) => {
    const aTs = a.records[0]?.timestamp ?? 0
    const bTs = b.records[0]?.timestamp ?? 0
    return aTs - bTs
  })

  return results
}

// Fetches run metadata + summary totals without fetching any records.
// Used for the first phase of two-phase loading on the run detail page.
export async function fetchRunMeta(id: string): Promise<RunDetail> {
  const [runRes, summaryRes] = await Promise.all([
    apiFetch(buildApiUrl(`/api/run/${id}`)),
    apiFetch(buildApiUrl(`/api/run/${id}/summary`)),
  ])
  if (!runRes.ok) throw new Error(`Run ${id} not found`)
  const backendRun: BackendRun = await runRes.json()
  const summary: BackendRunSummary | null = summaryRes.ok ? await summaryRes.json() : null

  let totalCpuEnergy: number, totalGpuEnergy: number, totalMemEnergy: number, totalIgpuEnergy: number
  if (summary) {
    totalCpuEnergy = summary.total_cpu_energy / 3600
    totalGpuEnergy = summary.total_gpu_energy / 3600
    totalMemEnergy = summary.total_mem_energy / 3600
    totalIgpuEnergy = summary.total_igpu_energy / 3600
  } else {
    totalCpuEnergy = 0; totalGpuEnergy = 0; totalMemEnergy = 0; totalIgpuEnergy = 0
  }
  const totalEnergy = totalCpuEnergy + totalGpuEnergy + totalMemEnergy + totalIgpuEnergy
  const energyKwh = totalEnergy / 1000
  const carbonFootprint = summary?.total_co2_g != null
    ? summary.total_co2_g
    : energyKwh * WORLD_AVERAGE_INTENSITY_G_PER_KWH
  const timestamp =
    parseTimestamp(backendRun.started_at) ??
    parseTimestamp(backendRun.ended_at) ??
    Date.now()

  return {
    id,
    projectId: backendRun.project_id.toString(),
    name: backendRun.name,
    timestamp,
    status: normalizeRunStatus(backendRun.status),
    collaboratorName: backendRun.collaborator_name ?? null,
    totalEnergy,
    totalCpuEnergy,
    totalGpuEnergy,
    totalMemEnergy,
    totalIgpuEnergy,
    totalCoreHours: energyKwh * 24,
    avgCpuUsage: 0,
    avgGpuUsage: 0,
    avgMemUsage: 0,
    carbonFootprint,
    waterConsumption: energyKwh * WATER_FACTOR * 1000,
    duration: calculateDurationSeconds([], backendRun.started_at, backendRun.ended_at),
    command: backendRun.command ?? null,
    carbonIntensityUsed: summary?.carbon_intensity_used ?? null,
    pueUsed: summary?.pue_used ?? null,
    totalCost: summary?.total_cost ?? null,
    currency: summary?.currency_used ?? null,
    systemInfo: backendRun.cpu_name ? {
      cpuName: backendRun.cpu_name,
      cpuCores: backendRun.cpu_cores ?? 0,
      ramGb: backendRun.ram_gb ?? 0,
      gpuCount: backendRun.gpu_count ?? 0,
      gpuNames: backendRun.gpu_names ?? '',
    } : null,
    records: [],
  }
}

// Fetches up to `limit` record points for a run, sorted by timestamp.
export async function fetchRunRecords(id: string, limit = 1000): Promise<MetricRecord[]> {
  const res = await apiFetch(buildApiUrl(`/api/run/${id}/record_points?limit=${limit}`))
  if (!res.ok) return []
  const backendRecords: BackendRecordPoint[] = await res.json()
  return backendRecords.map(mapRecord).sort((a, b) => a.timestamp - b.timestamp)
}

export async function fetchRunDetail(id: string): Promise<RunDetail> {
  try {
    const [runRes, summaryRes, recordsRes] = await Promise.all([
      apiFetch(buildApiUrl(`/api/run/${id}`)),
      apiFetch(buildApiUrl(`/api/run/${id}/summary`)),
      apiFetch(buildApiUrl(`/api/run/${id}/record_points`)),
    ])

    if (!runRes.ok) throw new Error(`Run ${id} not found`)

    const backendRun: BackendRun = await runRes.json()
    const summary: BackendRunSummary | null = summaryRes.ok ? await summaryRes.json() : null
    const backendRecords: BackendRecordPoint[] = recordsRes.ok ? await recordsRes.json() : []
    const records = backendRecords
      .map(mapRecord)
      .sort((left, right) => left.timestamp - right.timestamp)

    let totalCpuEnergy: number, totalGpuEnergy: number, totalMemEnergy: number, totalIgpuEnergy: number
    if (summary) {
      totalCpuEnergy = summary.total_cpu_energy / 3600
      totalGpuEnergy = summary.total_gpu_energy / 3600
      totalMemEnergy = summary.total_mem_energy / 3600
      totalIgpuEnergy = summary.total_igpu_energy / 3600
    } else {
      totalCpuEnergy = records.reduce((sum, r) => sum + r.cpu_energy, 0) / 3600
      totalGpuEnergy = records.reduce((sum, r) => sum + r.gpu_energy, 0) / 3600
      totalMemEnergy = records.reduce((sum, r) => sum + r.mem_energy, 0) / 3600
      totalIgpuEnergy = records.reduce((sum, r) => sum + r.igpu_energy, 0) / 3600
    }

    const totalEnergy = totalCpuEnergy + totalGpuEnergy + totalMemEnergy + totalIgpuEnergy
    const energyKwh = totalEnergy / 1000
    const carbonFootprint = summary?.total_co2_g != null
      ? summary.total_co2_g
      : energyKwh * WORLD_AVERAGE_INTENSITY_G_PER_KWH
    const avgCpuUsage = records.length > 0 ? records.reduce((sum, r) => sum + r.cpu_usage, 0) / records.length : 0
    const avgGpuUsage = records.length > 0 ? records.reduce((sum, r) => sum + r.gpu_usage, 0) / records.length : 0
    const avgMemUsage = records.length > 0 ? records.reduce((sum, r) => sum + r.mem_usage, 0) / records.length : 0
    const timestamp =
      parseTimestamp(backendRun.started_at) ??
      parseTimestamp(backendRun.ended_at) ??
      records[0]?.timestamp ??
      Date.now()

    return {
      id,
      projectId: backendRun.project_id.toString(),
      name: backendRun.name,
      timestamp,
      status: normalizeRunStatus(backendRun.status),
      collaboratorName: backendRun.collaborator_name ?? null,
      totalEnergy,
      totalCpuEnergy,
      totalGpuEnergy,
      totalMemEnergy,
      totalIgpuEnergy,
      totalCoreHours: energyKwh * 24,
      avgCpuUsage,
      avgGpuUsage,
      avgMemUsage,
      carbonFootprint,
      waterConsumption: energyKwh * WATER_FACTOR * 1000,
      duration: calculateDurationSeconds(records, backendRun.started_at, backendRun.ended_at),
      command: backendRun.command ?? null,
      carbonIntensityUsed: summary?.carbon_intensity_used ?? null,
      pueUsed: summary?.pue_used ?? null,
      totalCost: summary?.total_cost ?? null,
      currency: summary?.currency_used ?? null,
      systemInfo: backendRun.cpu_name ? {
        cpuName: backendRun.cpu_name,
        cpuCores: backendRun.cpu_cores ?? 0,
        ramGb: backendRun.ram_gb ?? 0,
        gpuCount: backendRun.gpu_count ?? 0,
        gpuNames: backendRun.gpu_names ?? '',
      } : null,
      records,
    }
  } catch (error) {
    throw error
  }
}

export async function fetchImprovements(): Promise<Improvement[]> {
  return []
}

export async function createRun(data: Partial<Run>): Promise<Run> {
  throw new Error('Not implemented')
}

export async function updateRun(id: string, data: Partial<Run>): Promise<Run> {
  throw new Error('Not implemented')
}

export async function deleteRun(id: string): Promise<void> {
  throw new Error('Not implemented')
}

export async function searchRuns(filters: Record<string, unknown>): Promise<RunSummary[]> {
  const allRuns = await fetchAllRuns()
  return allRuns.filter(run => {
    if (filters.status && run.status !== filters.status) return false
    if (filters.collaborator && run.collaboratorName !== filters.collaborator) return false
    return true
  })
}

export async function batchDeleteRuns(ids: string[]): Promise<void> {
  await Promise.all(ids.map(id => deleteRun(id)))
}

export async function exportRunAsCSV(id: string): Promise<Blob> {
  const run = await fetchRunDetail(id)
  const headers = ['Timestamp', 'CPU Usage (%)', 'CPU Energy (W)', 'GPU Usage (%)', 'GPU Energy (W)', 'MEM Energy (W)', 'iGPU Energy (W)']
  const rows = run.records.map(record => [
    new Date(record.timestamp).toISOString(),
    record.cpu_usage,
    record.cpu_energy,
    record.gpu_usage,
    record.gpu_energy,
    record.mem_energy,
    record.igpu_energy,
  ])
  const csv = [headers, ...rows].map(row => row.join(',')).join('\n')
  return new Blob([csv], { type: 'text/csv' })
}
