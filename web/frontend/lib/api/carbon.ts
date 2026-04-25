/**
 * Carbon configuration API — project-level carbon intensity, PUE, and cost settings.
 */

import { apiFetch, buildApiUrl } from './client'

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

export async function fetchCarbonConfig(projectId: number): Promise<CarbonConfig> {
  const res = await apiFetch(buildApiUrl(`/api/project/${projectId}/carbon-config`))
  if (!res.ok) throw new Error(`Failed to fetch carbon config: ${res.status}`)
  return res.json()
}

export async function updateCarbonConfig(
  projectId: number,
  payload: CarbonConfigUpdate
): Promise<CarbonConfig> {
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
