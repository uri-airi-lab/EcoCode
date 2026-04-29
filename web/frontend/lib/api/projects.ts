/**
 * Project-level API — create, list, and aggregate project metrics.
 */

import { apiFetch, buildApiUrl, BackendProject, BackendRun, fetchRunTotalsSnapshot, BackendPaginatedRuns } from './client'
import { mapRunSummary, RunSummary, mapBackendRunToSummary } from './runs'

export interface ProjectSummary {
  id: string
  name: string
  workspaceSlug: string
  totalRuns: number
  totalEnergy: number
  totalCo2G: number
  totalCost: number
  currency: string
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

export async function fetchAllProjects(): Promise<ProjectSummary[]> {
  try {
    const projectsRes = await apiFetch(buildApiUrl('/api/projects'))
    if (projectsRes.status === 404) return []
    if (!projectsRes.ok) throw new Error(`Failed to fetch projects: ${projectsRes.status}`)
    const projects: BackendProject[] = await projectsRes.json()

    return Promise.all(
      projects.map(async (project) => {
        const runsRes = await apiFetch(buildApiUrl(`/api/project/${project.id}/runs?limit=100`))

        if (!runsRes.ok) {
          return {
            id: project.id.toString(),
            name: project.name,
            workspaceSlug: project.workspace_slug,
            totalRuns: 0,
            totalEnergy: 0,
            totalCo2G: 0,
            totalCost: 0,
            currency: 'USD',
            lastRunTimestamp: 0,
          }
        }

        const data: BackendPaginatedRuns = await runsRes.json()
        const runSummaries = data.runs.map(mapBackendRunToSummary)
        const totalEnergy = runSummaries.reduce((sum, run) => sum + run.totalEnergy, 0)
        const totalCo2G = runSummaries.reduce((sum, run) => sum + (run.carbonFootprint || 0), 0)
        const totalCost = runSummaries.reduce((sum, run) => sum + (run.totalCost || 0), 0)
        const currency = runSummaries.find(r => r.currency)?.currency || 'USD'
        const lastRunTimestamp = runSummaries.reduce((max, run) => Math.max(max, run.timestamp), 0)

        return {
          id: project.id.toString(),
          name: project.name,
          workspaceSlug: project.workspace_slug,
          totalRuns: data.total,
          totalEnergy,
          totalCo2G,
          totalCost,
          currency,
          lastRunTimestamp: lastRunTimestamp || Date.now(),
        }
      })
    )
  } catch (error) {
    throw error
  }
}

export async function fetchProjectRuns(projectId: string): Promise<ProjectDetail> {
  try {
    const projectsRes = await apiFetch(buildApiUrl('/api/projects'))
    if (projectsRes.status === 404) throw new Error(`Project ${projectId} not found`)
    if (!projectsRes.ok) throw new Error(`Failed to fetch projects: ${projectsRes.status}`)
    const projects: BackendProject[] = await projectsRes.json()

    const project = projects.find(p => p.id.toString() === projectId)
    if (!project) throw new Error(`Project ${projectId} not found`)

    const runsRes = await apiFetch(buildApiUrl(`/api/project/${project.id}/runs?limit=1000`))
    let runSummaries: RunSummary[] = []

    if (runsRes.ok) {
      const data: BackendPaginatedRuns = await runsRes.json()
      runSummaries = data.runs.map(mapBackendRunToSummary)
    }

    const totalEnergy = runSummaries.reduce((sum, r) => sum + r.totalEnergy, 0)
    const totalCo2G = runSummaries.reduce((sum, r) => sum + (r.carbonFootprint || 0), 0)
    const totalCost = runSummaries.reduce((sum, r) => sum + (r.totalCost || 0), 0)
    const currency = runSummaries.find(r => r.currency)?.currency || 'USD'
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
      totalCo2G,
      totalCost,
      currency,
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
