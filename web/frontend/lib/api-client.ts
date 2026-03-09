/**
 * API Client for EcoCode Dashboard
 * 
 * This service layer abstracts data fetching and makes it easy to swap
 * between mock data and real backend calls.
 * 
 * To integrate with a real backend:
 * 1. Replace the mock data imports with actual API calls
 * 2. Update the fetch URLs from `/api/*` to your backend endpoint
 * 3. Add error handling and retry logic as needed
 */

import { Run, MetricRecord } from './mock-data'

const API_BASE_URL = process.env.NEXT_PUBLIC_API_URL || 'http://localhost:3001'

// --- Internal Mappings ---

const CARBON_FACTOR = 0.475 // kgCO2/kWh
const WATER_FACTOR = 1.8 // L/kWh

interface BackendProject {
  id: number
  name: string
}

interface BackendRun {
  id: number
  name: string
  project_id: number
  collaborator_name?: string | null
  status?: 'running' | 'finished' | 'failed' | null
  started_at?: string | null
  ended_at?: string | null
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
}

interface BackendRunSummary {
  run_id: number
  total_cpu_energy: number
  total_gpu_energy: number
  total_mem_energy: number
  total_igpu_energy: number
}

interface BackendAdminRun {
  run_id: number
  run_name: string
  collaborator_name: string | null
  status: 'running' | 'finished' | 'failed'
  started_at: string | null
  ended_at: string | null
  last_record_at: string | null
}

interface BackendAdminProject {
  id: number
  name: string
  default_collaborator: string | null
  collaborators?: string[]
  active_runs?: BackendAdminRun[]
  latest_run: BackendAdminRun | null
}

function parseTimestamp(value?: string | null): number | null {
  if (!value) return null
  const timestamp = new Date(value).getTime()
  return Number.isNaN(timestamp) ? null : timestamp
}

function normalizeRunStatus(status?: BackendRun['status']): Run['status'] {
  return status === 'running' || status === 'failed' ? status : 'finished'
}

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
  }
}

interface RunMetricSnapshot {
  records: MetricRecord[]
  totalCpuEnergy: number
  totalGpuEnergy: number
  totalMemEnergy: number
  totalIgpuEnergy: number
  totalEnergy: number
  avgCpuUsage: number
  avgGpuUsage: number
  avgMemUsage: number
  duration: number
  timestamp: number
}

interface RunTotalsSnapshot {
  totalCpuEnergy: number
  totalGpuEnergy: number
  totalMemEnergy: number
  totalIgpuEnergy: number
  totalEnergy: number
  timestamp: number
}

function calculateAverages(records: MetricRecord[]) {
  if (records.length === 0) {
    return {
      avgCpuUsage: 0,
      avgGpuUsage: 0,
      avgMemUsage: 0,
    }
  }

  return {
    avgCpuUsage: records.reduce((sum, record) => sum + record.cpu_usage, 0) / records.length,
    avgGpuUsage: records.reduce((sum, record) => sum + record.gpu_usage, 0) / records.length,
    avgMemUsage: records.reduce((sum, record) => sum + record.mem_usage, 0) / records.length,
  }
}

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

function calculateEnergyTotals(summary: BackendRunSummary | null) {
  const totalCpuEnergy = summary?.total_cpu_energy ?? 0
  const totalGpuEnergy = summary?.total_gpu_energy ?? 0
  const totalMemEnergy = summary?.total_mem_energy ?? 0
  const totalIgpuEnergy = summary?.total_igpu_energy ?? 0
  const totalEnergy = totalCpuEnergy + totalGpuEnergy + totalMemEnergy + totalIgpuEnergy

  return {
    totalCpuEnergy,
    totalGpuEnergy,
    totalMemEnergy,
    totalIgpuEnergy,
    totalEnergy,
  }
}

async function fetchRunMetricSnapshot(backendRun: BackendRun): Promise<RunMetricSnapshot> {
  const [summaryRes, recordsRes] = await Promise.all([
    fetch(`${API_BASE_URL}/api/run/${backendRun.id}/summary`),
    fetch(`${API_BASE_URL}/api/run/${backendRun.id}/record_points`),
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
  const summaryRes = await fetch(`${API_BASE_URL}/api/run/${backendRun.id}/summary`)
  const summary: BackendRunSummary | null = summaryRes.ok ? await summaryRes.json() : null
  const energyTotals = calculateEnergyTotals(summary)
  const timestamp =
    parseTimestamp(backendRun.started_at) ??
    parseTimestamp(backendRun.ended_at) ??
    Date.now()

  return {
    ...energyTotals,
    timestamp,
  }
}

async function mapRunSummary(backendRun: BackendRun): Promise<RunSummary> {
  const snapshot = await fetchRunMetricSnapshot(backendRun)
  const energyKwh = snapshot.totalEnergy / 1000

  return {
    id: backendRun.id.toString(),
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
    carbonFootprint: energyKwh * CARBON_FACTOR * 1000,
    waterConsumption: energyKwh * WATER_FACTOR * 1000,
    duration: snapshot.duration,
  }
}

async function mapRun(backendRun: BackendRun): Promise<Run> {
  const snapshot = await fetchRunMetricSnapshot(backendRun)
  const energyKwh = snapshot.totalEnergy / 1000

  return {
    id: backendRun.id.toString(),
    name: backendRun.name,
    timestamp: snapshot.timestamp,
    status: normalizeRunStatus(backendRun.status),
    collaboratorName: backendRun.collaborator_name ?? null,
    totalEnergy: snapshot.totalEnergy,
    totalCpuEnergy: snapshot.totalCpuEnergy,
    totalGpuEnergy: snapshot.totalGpuEnergy,
    totalMemEnergy: snapshot.totalMemEnergy,
    totalIgpuEnergy: snapshot.totalIgpuEnergy,
    totalCoreHours: energyKwh * 24, // Mock calculation
    avgCpuUsage: snapshot.avgCpuUsage,
    avgGpuUsage: snapshot.avgGpuUsage,
    avgMemUsage: snapshot.avgMemUsage,
    carbonFootprint: energyKwh * CARBON_FACTOR * 1000,
    waterConsumption: energyKwh * WATER_FACTOR * 1000,
    duration: snapshot.duration,
    records: snapshot.records,
  }
}

// --- Public API ---

export interface RunSummary extends Omit<Run, 'records'> { }
export interface RunDetail extends Run { }
export interface ProjectSummary {
  id: string
  name: string
  totalRuns: number
  totalEnergy: number // Wh
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
 * Fetch all projects and aggregate their metrics
 */
export async function fetchAllProjects(): Promise<ProjectSummary[]> {
  try {
    const projectsRes = await fetch(`${API_BASE_URL}/api/projects`)
    if (!projectsRes.ok) throw new Error('Failed to fetch projects')
    const projects: BackendProject[] = await projectsRes.json()

    // Process projects concurrently
    const projectSummaries = await Promise.all(
      projects.map(async (project) => {
        const runsRes = await fetch(`${API_BASE_URL}/api/project/${project.id}/runs`)

        if (!runsRes.ok) {
          return {
            id: project.id.toString(),
            name: project.name,
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
          totalRuns: backendRuns.length,
          totalEnergy,
          lastRunTimestamp: lastRunTimestamp || Date.now(),
        }
      })
    )

    return projectSummaries
  } catch (error) {
    console.error('Error fetching projects:', error)
    throw error
  }
}

/**
 * Fetch a specific project and its runs
 */
export async function fetchProjectRuns(projectId: string): Promise<ProjectDetail> {
  try {
    const projectsRes = await fetch(`${API_BASE_URL}/api/projects`)
    if (!projectsRes.ok) throw new Error('Failed to fetch projects')
    const projects: BackendProject[] = await projectsRes.json()

    const project = projects.find(p => p.id.toString() === projectId)
    if (!project) throw new Error(`Project ${projectId} not found`)

    const runsRes = await fetch(`${API_BASE_URL}/api/project/${project.id}/runs`)
    let runSummaries: RunSummary[] = []

    if (runsRes.ok) {
      const backendRuns: BackendRun[] = await runsRes.json()
      // Map properties currently in parallel
      runSummaries = await Promise.all(backendRuns.map(mapRunSummary))
    }

    const totalEnergy = runSummaries.reduce((sum, r) => sum + r.totalEnergy, 0)
    const lastRunTimestamp = runSummaries.reduce((max, r) => Math.max(max, r.timestamp), 0)
    const totalCpuEnergy = runSummaries.reduce((sum, run) => sum + run.totalCpuEnergy, 0)
    const totalGpuEnergy = runSummaries.reduce((sum, run) => sum + run.totalGpuEnergy, 0)
    const totalMemEnergy = runSummaries.reduce((sum, run) => sum + run.totalMemEnergy, 0)
    const totalIgpuEnergy = runSummaries.reduce((sum, run) => sum + run.totalIgpuEnergy, 0)
    const avgCpuUsage =
      runSummaries.length > 0
        ? runSummaries.reduce((sum, run) => sum + run.avgCpuUsage, 0) / runSummaries.length
        : 0
    const avgGpuUsage =
      runSummaries.length > 0
        ? runSummaries.reduce((sum, run) => sum + run.avgGpuUsage, 0) / runSummaries.length
        : 0
    const avgMemUsage =
      runSummaries.length > 0
        ? runSummaries.reduce((sum, run) => sum + run.avgMemUsage, 0) / runSummaries.length
        : 0

    return {
      id: project.id.toString(),
      name: project.name,
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
      runs: runSummaries.sort((a, b) => b.timestamp - a.timestamp)
    }
  } catch (error) {
    console.error(`Error fetching project ${projectId}:`, error)
    throw error
  }
}

/**
 * Fetch all runs for all projects
 */
export async function fetchAllRuns(): Promise<RunSummary[]> {
  try {
    const projectsRes = await fetch(`${API_BASE_URL}/api/projects`)
    if (!projectsRes.ok) throw new Error('Failed to fetch projects')
    const projects: BackendProject[] = await projectsRes.json()

    // Fetch runs for ALL projects concurrently
    const projectRunsArrays = await Promise.all(
      projects.map(async (project) => {
        const runsRes = await fetch(`${API_BASE_URL}/api/project/${project.id}/runs`)
        if (!runsRes.ok) return []

        const backendRuns: BackendRun[] = await runsRes.json()
        return Promise.all(backendRuns.map(mapRunSummary))
      })
    )

    // Flatten array of arrays
    const allRuns = projectRunsArrays.flat()

    return allRuns.sort((a, b) => b.timestamp - a.timestamp)
  } catch (error) {
    console.error('Error fetching runs:', error)
    throw error
  }
}

/**
 * Fetch a specific run with record points
 */
export async function fetchRunDetail(id: string): Promise<RunDetail> {
  try {
    const [runRes, summaryRes, recordsRes] = await Promise.all([
      fetch(`${API_BASE_URL}/api/run/${id}`),
      fetch(`${API_BASE_URL}/api/run/${id}/summary`),
      fetch(`${API_BASE_URL}/api/run/${id}/record_points`),
    ])

    if (!runRes.ok || !summaryRes.ok) throw new Error(`Run ${id} not found`)

    const backendRun: BackendRun = await runRes.json()
    const summary: BackendRunSummary = await summaryRes.json()
    const backendRecords: BackendRecordPoint[] = recordsRes.ok ? await recordsRes.json() : []
    const records = backendRecords
      .map(mapRecord)
      .sort((left, right) => left.timestamp - right.timestamp)
    const totalCpuEnergy = summary.total_cpu_energy
    const totalGpuEnergy = summary.total_gpu_energy
    const totalMemEnergy = summary.total_mem_energy
    const totalIgpuEnergy = summary.total_igpu_energy
    const totalEnergy = totalCpuEnergy + totalGpuEnergy + totalMemEnergy + totalIgpuEnergy
    const energyKwh = totalEnergy / 1000
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
      carbonFootprint: energyKwh * CARBON_FACTOR * 1000,
      waterConsumption: energyKwh * WATER_FACTOR * 1000,
      duration: calculateDurationSeconds(records, backendRun.started_at, backendRun.ended_at),
      records,
    }
  } catch (error) {
    console.error(`Error fetching run ${id}:`, error)
    throw error
  }
}

/**
 * Fetch improvements (placeholder as backend doesn't have this yet)
 */
export async function fetchImprovements(): Promise<Improvement[]> {
  return [] // Return empty for now
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

export async function searchRuns(filters: Record<string, any>): Promise<RunSummary[]> {
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
  }
}

export async function fetchAdminProjects(): Promise<AdminProjectMonitor[]> {
  const response = await fetch(`${API_BASE_URL}/api/admin/projects`)
  if (!response.ok) {
    throw new Error('Failed to fetch admin project monitor data')
  }

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
