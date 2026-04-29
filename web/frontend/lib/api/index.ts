/**
 * Public API surface for the EcoCode frontend.
 * Import from '@/lib/api' to access all typed fetch functions.
 */

export type { CarbonConfig, CarbonRegion, CarbonConfigUpdate } from './carbon'
export {
  fetchCarbonConfig,
  updateCarbonConfig,
  refreshCarbonIntensity,
  fetchCarbonRegions,
} from './carbon'

export type { RunSummary, RunDetail, Improvement } from './runs'
export {
  fetchAllRuns,
  fetchAllRunsWithRecords,
  fetchRunDetail,
  fetchRunMeta,
  fetchRunRecords,
  mapRunSummary,
  fetchImprovements,
  createRun,
  updateRun,
  deleteRun,
  searchRuns,
  batchDeleteRuns,
  exportRunAsCSV,
  fetchProjectRunsPaginated,
} from './runs'

export type { ProjectSummary, ProjectDetail } from './projects'
export { createProject, fetchAllProjects, fetchProjectRuns } from './projects'

export type { AdminRunInfo, AdminProjectMonitor } from './admin'
export { fetchAdminProjects } from './admin'
