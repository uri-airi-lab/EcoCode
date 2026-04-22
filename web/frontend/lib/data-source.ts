export const API_BASE_URL = process.env.NEXT_PUBLIC_API_URL || 'http://localhost:3001'

export type DataSource = 'local' | 'online'

export interface DataSourceAvailability {
  defaultSource: DataSource
  availableSources: DataSource[]
}

const STORAGE_KEY = 'ecocode-data-source'

export function normalizeDataSource(value?: string | null): DataSource | null {
  if (value === 'local' || value === 'online') {
    return value
  }

  return null
}

export function buildApiUrl(path: string, source?: DataSource): string {
  const url = new URL(path, API_BASE_URL)

  if (source) {
    url.searchParams.set('source', source)
  }

  return url.toString()
}

export function readStoredDataSource(): DataSource | null {
  if (typeof window === 'undefined') {
    return null
  }

  return normalizeDataSource(window.localStorage.getItem(STORAGE_KEY))
}

export function storeDataSource(source: DataSource) {
  if (typeof window === 'undefined') {
    return
  }

  window.localStorage.setItem(STORAGE_KEY, source)
}

export async function fetchDataSourceAvailability(): Promise<DataSourceAvailability> {
  const response = await fetch(buildApiUrl('/api/data-sources'))
  if (!response.ok) {
    throw new Error('Failed to fetch data source availability')
  }

  const payload = await response.json()
  const defaultSource = normalizeDataSource(payload.default_source) ?? 'local'
  const availableSources = Array.isArray(payload.available_sources)
    ? payload.available_sources
        .map((value: unknown) => normalizeDataSource(typeof value === 'string' ? value : null))
        .filter((value: DataSource | null): value is DataSource => value !== null)
    : ['local']

  return {
    defaultSource,
    availableSources: availableSources.length > 0 ? availableSources : ['local'],
  }
}
