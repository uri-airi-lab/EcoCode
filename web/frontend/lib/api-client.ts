/**
 * API client for the EcoCode web backend.
 *
 * Provides typed fetch wrappers that:
 * - Include credentials (session cookies) on every request.
 * - Convert backend JSON (energy in Joules, timestamps as ISO strings) to the
 *   frontend's domain types (energy in Wh, timestamps as Unix ms).
 * - Aggregate per-run summaries and averages for charts and tables.
 *
 * Energy unit conversions applied here:
 *   Database stores energy in Joules.
 *   Frontend displays in Watt-hours:  Wh = J / 3600
 *
 * Water consumption estimate: 1.8 L/kWh (global data-centre average).
 * Carbon fallback: 445 gCO2/kWh (IEA Emissions Factors 2025 world average) when no project config is set.
 */

import { Run, MetricRecord } from './mock-data'
import { buildApiUrl } from './data-source'

// --- Internal Mappings ---

/** Water consumption factor used to estimate mL consumed per Wh of energy. */
const WATER_FACTOR = 1.8 // L/kWh

interface BackendProject {
  id: number
  name: string
  workspace_slug: string
}

interface BackendRun {
  id: number
  name: string
  project_id: number
  collaborator_name?: string | null
  status?: 'running' | 'finished' | 'failed' | null
  started_at?: string | null
  ended_at?: string | null
  command?: string | null
  cpu_name?: string | null
  cpu_cores?: number | null
  ram_gb?: number | null
  gpu_count?: number | null
  gpu_names?: string | null
  total_cost?: number | null
  currency?: string | null
}

interface BackendRecordPoint {
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
}

interface BackendGpuRecordPoint {
  id: number
  record_id: number
  device_index: number
  gpu_usage: number
  gpu_energy: number
}

interface BackendSocketRecordPoint {
  id: number
  record_id: number
  socket: number
  domain: string
  energy: number
}

interface BackendRunSummary {
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
}

export interface CarbonConfig {
  country_code: string
  address: string | null
  latitude: number | null
  longitude: number | null
  has_api_key: boolean
  resolved_intensity_g_per_kwh: number
  intensity_source: string
  intensity_fetched_at: string | null
  pue_factor: number
  manual_cost_per_kwh: number | null
  manual_currency: string | null
  use_api_for_cost: boolean
  updated_at: string
}

export interface CarbonRegion {
  code: string
  name: string
  intensity_g_per_kwh: number
}

export interface CarbonConfigUpdate {
  address?: string
  latitude?: number
  longitude?: number
  country_code?: string
  electricity_maps_api_key?: string
  clear_api_key?: boolean
  pue_factor?: number
  manual_cost_per_kwh?: number
  manual_currency?: string
  use_api_for_cost?: boolean
}

interface BackendAdminRun {
  run_id: number
  run_name: string
  collaborator_name: string | null
  status: 'running' | 'finished' | 'failed'
  started_at: string | null
  ended_at: string | null
  last_record_at: string | null
  command: string | null
}

interface BackendAdminProject {
  id: number
  name: string
  default_collaborator: string | null
  collaborators?: string[]
  active_runs?: BackendAdminRun[]
  latest_run: BackendAdminRun | null
}

/** Parses an ISO timestamp string into a Unix millisecond value, or null if absent/invalid. */
function parseTimestamp(value?: string | null): number | null {
  if (!value) return null
  const timestamp = new Date(value).getTime()
  return Number.isNaN(timestamp) ? null : timestamp
}

/** Normalises the backend run status to one of the three frontend-known values. */
function normalizeRunStatus(status?: BackendRun['status']): Run['status'] {
  return status === 'running' || status === 'failed' ? status : 'finished'
}

/** Maps a backend record point (Joules, ISO timestamp) to the frontend MetricRecord type. */
function mapRecord(r: BackendRecordPoint): MetricRecord {
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
  }
}

interface RunMetricSnapshot {
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

interface RunTotalsSnapshot {
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
}

/** Computes mean CPU, GPU, and memory utilization across all records in a run. */
function calculateAverages(records: MetricRecord[]) {
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
 * Prefers the `started_at` / `ended_at` timestamps from the run metadata.
 * Falls back to the span of record timestamps if run timestamps are absent.
 */
function calculateDurationSeconds(
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

/** Fallback carbon intensity (gCO2/kWh) when no project configuration is available. */
const WORLD_AVERAGE_INTENSITY_G_PER_KWH = 445 // IEA Emissions Factors 2025

/**
 * Converts Joule-based energy totals from the backend summary into Wh and
 * derives CO2 and cost figures. Falls back to the world average intensity if
 * the project has not been configured with a specific value.
 */
function calculateEnergyTotals(summary: BackendRunSummary | null) {
  // DB stores energy in Joules; convert to Wh for display (1 Wh = 3600 J)
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
  }
}

/** Thin wrapper around `fetch` that always sends session cookies. */
async function apiFetch(url: string, init?: RequestInit) {
  return fetch(url, {
    ...init,
    credentials: 'include',
  })
}

async function fetchRunMetricSnapshot(backendRun: BackendRun): Promise<RunMetricSnapshot> {
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

async function fetchRunTotalsSnapshot(backendRun: BackendRun): Promise<RunTotalsSnapshot> {
  const summaryRes = await apiFetch(buildApiUrl(`/api/run/${backendRun.id}/summary`))
  const summary: BackendRunSummary | null = summaryRes.ok ? await summaryRes.json() : null
  const energyTotals = calculateEnergyTotals(summary)
  const timestamp =
    parseTimestamp(backendRun.started_at) ??
    parseTimestamp(backendRun.ended_at) ??
    Date.now()

  return { ...energyTotals, timestamp }
}

// --- Carbon API ---

export async function fetchCarbonConfig(projectId: number): Promise<CarbonConfig> {
  const res = await apiFetch(buildApiUrl(`/api/project/${projectId}/carbon-config`))
  if (!res.ok) throw new Error(`Failed to fetch carbon config: ${res.status}`)
  return res.json()
}

export async function updateCarbonConfig(projectId: number, payload: CarbonConfigUpdate): Promise<CarbonConfig> {
  const res = await apiFetch(buildApiUrl(`/api/project/${projectId}/carbon-config`), {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  })
  if (!res.ok) throw new Error(`Failed to update carbon config: ${res.status}`)
  return res.json()
}

export async function refreshCarbonIntensity(projectId: number): Promise<CarbonConfig> {
  const res = await apiFetch(buildApiUrl(`/api/project/${projectId}/carbon-config/refresh`), {
    method: 'POST',
  })
  if (!res.ok) throw new Error(`Failed to refresh carbon intensity: ${res.status}`)
  return res.json()
}

export async function fetchCarbonRegions(): Promise<CarbonRegion[]> {
  const res = await apiFetch(buildApiUrl('/api/carbon-regions'))
  if (!res.ok) throw new Error(`Failed to fetch carbon regions: ${res.status}`)
  return res.json()
}

async function mapRunSummary(backendRun: BackendRun): Promise<RunSummary> {
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
  }
}

async function mapRun(backendRun: BackendRun): Promise<Run> {
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

// --- Public API ---

export interface RunSummary extends Omit<Run, 'records'> { }
export interface RunDetail extends Run { }
export interface ProjectSummary {
  id: string
  name: string
  workspaceSlug: string
  totalRuns: number
  totalEnergy: number
  lastRunTimestamp: number
}
export interface ProjectDetail extends ProjectSummary {
  avgCpuUsage: number
  avgGpuUsage: number
  avgMemUsage: number
  totalCpuEnergy: number
  totalGpuEnergy: number
  totalMemEnergy: number
  totalIgpuEnergy: number
  runs: RunSummary[]
}
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

export interface AdminRunInfo {
  runId: string
  runName: string
  collaboratorName: string | null
  status: Run['status']
  startedAt: number | null
  endedAt: number | null
  lastRecordAt: number | null
  command: string | null
}

export interface AdminProjectMonitor {
  id: string
  name: string
  defaultCollaborator: string | null
  collaborators: string[]
  activeRuns: AdminRunInfo[]
  latestRun: AdminRunInfo | null
}

/**
 * Create a new project with initial configuration
 */
export async function createProject(
  name: string,
  workspaceId: number,
  config: { intensity?: number; cost?: number; currency?: string; address?: string; use_api_for_cost?: boolean }
): Promise<BackendProject> {
  const res = await apiFetch(buildApiUrl('/api/projects'), {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ name, workspace_id: workspaceId, ...config }),
  })
  if (!res.ok) {
    const errorText = await res.text()
    throw new Error(errorText || 'Failed to create project')
  }
  return res.json()
}

/**
 * Fetch all projects and aggregate their metrics
 */
export async function fetchAllProjects(): Promise<ProjectSummary[]> {
  try {
    const projectsRes = await apiFetch(buildApiUrl('/api/projects'))
    if (projectsRes.status === 404) return []
    if (!projectsRes.ok) throw new Error(`Failed to fetch projects: ${projectsRes.status}`)
    const projects: BackendProject[] = await projectsRes.json()

    const projectSummaries = await Promise.all(
      projects.map(async (project) => {
        const runsRes = await apiFetch(buildApiUrl(`/api/project/${project.id}/runs`))

        if (!runsRes.ok) {
          return {
            id: project.id.toString(),
            name: project.name,
            workspaceSlug: project.workspace_slug,
            totalRuns: 0,
            totalEnergy: 0,
            lastRunTimestamp: 0,
          }
        }

        const backendRuns: BackendRun[] = await runsRes.json()
        const runSummaries = await Promise.all(backendRuns.map(fetchRunTotalsSnapshot))
        const totalEnergy = runSummaries.reduce((sum, run) => sum + run.totalEnergy, 0)
        const lastRunTimestamp = runSummaries.reduce((max, run) => Math.max(max, run.timestamp), 0)

        return {
          id: project.id.toString(),
          name: project.name,
          workspaceSlug: project.workspace_slug,
          totalRuns: backendRuns.length,
          totalEnergy,
          lastRunTimestamp: lastRunTimestamp || Date.now(),
        }
      })
    )

    return projectSummaries
  } catch (error) {
    throw error
  }
}

/**
 * Fetch a specific project and its runs
 */
export async function fetchProjectRuns(projectId: string): Promise<ProjectDetail> {
  try {
    const projectsRes = await apiFetch(buildApiUrl('/api/projects'))
    if (projectsRes.status === 404) throw new Error(`Project ${projectId} not found`)
    if (!projectsRes.ok) throw new Error(`Failed to fetch projects: ${projectsRes.status}`)
    const projects: BackendProject[] = await projectsRes.json()

    const project = projects.find(p => p.id.toString() === projectId)
    if (!project) throw new Error(`Project ${projectId} not found`)

    const runsRes = await apiFetch(buildApiUrl(`/api/project/${project.id}/runs`))
    let runSummaries: RunSummary[] = []

    if (runsRes.ok) {
      const backendRuns: BackendRun[] = await runsRes.json()
      runSummaries = await Promise.all(backendRuns.map(mapRunSummary))
    }

    const totalEnergy = runSummaries.reduce((sum, r) => sum + r.totalEnergy, 0)
    const lastRunTimestamp = runSummaries.reduce((max, r) => Math.max(max, r.timestamp), 0)
    const totalCpuEnergy = runSummaries.reduce((sum, r) => sum + r.totalCpuEnergy, 0)
    const totalGpuEnergy = runSummaries.reduce((sum, r) => sum + r.totalGpuEnergy, 0)
    const totalMemEnergy = runSummaries.reduce((sum, r) => sum + r.totalMemEnergy, 0)
    const totalIgpuEnergy = runSummaries.reduce((sum, r) => sum + r.totalIgpuEnergy, 0)
    const avgCpuUsage = runSummaries.length > 0
      ? runSummaries.reduce((sum, r) => sum + r.avgCpuUsage, 0) / runSummaries.length : 0
    const avgGpuUsage = runSummaries.length > 0
      ? runSummaries.reduce((sum, r) => sum + r.avgGpuUsage, 0) / runSummaries.length : 0
    const avgMemUsage = runSummaries.length > 0
      ? runSummaries.reduce((sum, r) => sum + r.avgMemUsage, 0) / runSummaries.length : 0

    return {
      id: project.id.toString(),
      name: project.name,
      workspaceSlug: project.workspace_slug,
      totalRuns: runSummaries.length,
      totalEnergy,
      lastRunTimestamp: lastRunTimestamp || Date.now(),
      avgCpuUsage,
      avgGpuUsage,
      avgMemUsage,
      totalCpuEnergy,
      totalGpuEnergy,
      totalMemEnergy,
      totalIgpuEnergy,
      runs: runSummaries.sort((a, b) => b.timestamp - a.timestamp),
    }
  } catch (error) {
    throw error
  }
}

/**
 * Fetch all runs for all projects
 */
export async function fetchAllRuns(): Promise<RunSummary[]> {
  try {
    const projectsRes = await apiFetch(buildApiUrl('/api/projects'))
    if (projectsRes.status === 404) return []
    if (!projectsRes.ok) throw new Error(`Failed to fetch projects: ${projectsRes.status}`)
    const projects: BackendProject[] = await projectsRes.json()

    const projectRunsArrays = await Promise.all(
      projects.map(async (project) => {
        const runsRes = await apiFetch(buildApiUrl(`/api/project/${project.id}/runs`))
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

/**
 * Fetch all runs with their record points, sorted oldest-first.
 * Returns an array of { runId, runName, records } so callers can stitch them.
 */
export async function fetchAllRunsWithRecords(
  projectIds?: string[]
): Promise<{ runId: string; runName: string; records: MetricRecord[] }[]> {
  const projectsRes = await apiFetch(buildApiUrl('/api/projects'))
  if (!projectsRes.ok) return []
  const projects: BackendProject[] = await projectsRes.json()

  const filtered = projectIds
    ? projects.filter((p) => projectIds.includes(p.id.toString()))
    : projects

  const results: { runId: string; runName: string; records: MetricRecord[] }[] = []

  await Promise.all(
    filtered.map(async (project) => {
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

  // Sort runs oldest-first by their first record timestamp
  results.sort((a, b) => {
    const aTs = a.records[0]?.timestamp ?? 0
    const bTs = b.records[0]?.timestamp ?? 0
    return aTs - bTs
  })

  return results
}

/**
 * Fetch a specific run with record points
 */
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
    const totalCost = summary?.total_cost ?? null
    const currency = summary?.currency_used ?? null
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
      totalCost,
      currency,
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

function mapAdminRun(run: BackendAdminRun): AdminRunInfo {
  return {
    runId: run.run_id.toString(),
    runName: run.run_name,
    collaboratorName: run.collaborator_name,
    status: run.status,
    startedAt: parseTimestamp(run.started_at),
    endedAt: parseTimestamp(run.ended_at),
    lastRecordAt: parseTimestamp(run.last_record_at),
    command: run.command,
  }
}

export async function fetchAdminProjects(): Promise<AdminProjectMonitor[]> {
  const response = await apiFetch(buildApiUrl('/api/admin/projects'))
  if (!response.ok) throw new Error(`Failed to fetch admin project monitor data: ${response.status}`)

  const projects: BackendAdminProject[] = await response.json()
  return projects.map((project) => ({
    id: project.id.toString(),
    name: project.name,
    defaultCollaborator: project.default_collaborator,
    collaborators: project.collaborators ?? [],
    activeRuns: (project.active_runs ?? []).map(mapAdminRun),
    latestRun: project.latest_run ? mapAdminRun(project.latest_run) : null,
  }))
}
