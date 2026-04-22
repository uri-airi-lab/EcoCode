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
      } catch (error) {
        if (cancelled) return

        const stored = readStoredDataSource()
        const nextSource = stored ?? 'local'
        setAvailableSources(['local'])
        setSourceState(nextSource === 'online' ? 'local' : nextSource)
        storeDataSource(nextSource === 'online' ? 'local' : nextSource)
      } finally {
        if (!cancelled) {
          setLoadingSources(false)
        }
      }
    }

    loadSources()

    return () => {
      cancelled = true
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
