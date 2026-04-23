'use client'

import {
  createContext,
  useContext,
  useEffect,
  useMemo,
  useState,
} from 'react'

import {
  DataSource,
  fetchDataSourceAvailability,
  readStoredDataSource,
  storeDataSource,
} from '@/lib/data-source'

interface DataSourceContextValue {
  source: DataSource
  availableSources: DataSource[]
  loadingSources: boolean
  setSource: (source: DataSource) => void
}

const DataSourceContext = createContext<DataSourceContextValue | null>(null)

export function DataSourceProvider({
  children,
}: {
  children: React.ReactNode
}) {
  const [source, setSourceState] = useState<DataSource>('local')
  const [availableSources, setAvailableSources] = useState<DataSource[]>(['local'])
  const [loadingSources, setLoadingSources] = useState(true)

  useEffect(() => {
    let cancelled = false
    let retryTimer: ReturnType<typeof setTimeout> | null = null

    const loadSources = async () => {
      try {
        const availability = await fetchDataSourceAvailability()
        if (cancelled) return

        const stored = readStoredDataSource()
        const nextSource =
          stored && availability.availableSources.includes(stored)
            ? stored
            : availability.availableSources.includes(availability.defaultSource)
              ? availability.defaultSource
              : availability.availableSources[0]

        setAvailableSources(availability.availableSources)
        setSourceState(nextSource)
        storeDataSource(nextSource)
        setLoadingSources(false)
      } catch {
        if (cancelled) return

        // Backend not reachable yet — fall back to local and retry in 5s
        const stored = readStoredDataSource()
        setAvailableSources(['local'])
        setSourceState('local')
        storeDataSource('local')
        setLoadingSources(false)
        retryTimer = setTimeout(loadSources, 5000)
      }
    }

    loadSources()

    return () => {
      cancelled = true
      if (retryTimer) clearTimeout(retryTimer)
    }
  }, [])

  const setSource = (nextSource: DataSource) => {
    if (!availableSources.includes(nextSource)) {
      return
    }

    setSourceState(nextSource)
    storeDataSource(nextSource)
  }

  const value = useMemo(
    () => ({
      source,
      availableSources,
      loadingSources,
      setSource,
    }),
    [availableSources, loadingSources, source]
  )

  return <DataSourceContext.Provider value={value}>{children}</DataSourceContext.Provider>
}

export function useDataSource() {
  const context = useContext(DataSourceContext)
  if (!context) {
    throw new Error('useDataSource must be used within a DataSourceProvider')
  }

  return context
}
