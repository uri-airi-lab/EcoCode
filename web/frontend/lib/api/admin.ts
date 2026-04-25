/**
 * Admin monitoring API — live view of all projects and active runs.
 */

import { Run } from '../mock-data'
import { apiFetch, buildApiUrl, BackendAdminRun, BackendAdminProject, parseTimestamp } from './client'

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
