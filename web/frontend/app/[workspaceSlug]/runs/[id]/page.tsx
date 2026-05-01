'use client'

import { useEffect, useRef, useState } from 'react'
import { useParams } from 'next/navigation'
import Link from 'next/link'
import { ArrowLeft } from 'lucide-react'
import { DashboardLayout } from '@/components/layout/DashboardLayout'
import { EnergyChart } from '@/components/charts/EnergyChart'
import { UtilizationChart } from '@/components/charts/UtilizationChart'
import { EnergyBreakdownChart } from '@/components/charts/EnergyBreakdownChart'
import { SocketEnergyChart } from '@/components/charts/SocketEnergyChart'
import { RunSystemInfo } from '@/components/run/RunSystemInfo'
import { PowerDistributionChart } from '@/components/charts/PowerDistributionChart'
import { CumulativePowerChart } from '@/components/charts/CumulativePowerChart'
import { EnvironmentalImpact } from '@/components/run/EnvironmentalImpact'
import { Run } from '@/lib/mock-data'
import { fetchRunMeta, fetchRunRecords } from '@/lib/api'
import { format } from 'date-fns'

const POLL_INTERVAL_MS = 1000

interface RunDetailPageProps {
  params: Promise<{ id: string }>
}

export default function RunDetailPage({ params }: RunDetailPageProps) {
  const routeParams = useParams()
  const workspaceSlug = routeParams.workspaceSlug as string
  const [run, setRun] = useState<Run | null>(null)
  const [loading, setLoading] = useState(true)
  const [chartsLoading, setChartsLoading] = useState(true)
  const [paramId, setParamId] = useState<string | null>(null)
  // Tracks how many records we already have so incremental polls only fetch the diff
  const recordOffsetRef = useRef(0)

  useEffect(() => {
    const initParams = async () => {
      const { id } = await params
      setParamId(id)
    }
    initParams()
  }, [params])

  useEffect(() => {
    if (!paramId) return
    let cancelled = false

    const fetchRun = async () => {
      setLoading(true)
      setChartsLoading(true)

      // Phase 1: fetch metadata + summary — no records, shows page immediately
      try {
        const meta = await fetchRunMeta(paramId)
        if (cancelled) return
        setRun(meta)
      } catch (error) {
        if (!cancelled) {
          console.error('Failed to fetch run:', error)
          setRun(null)
        }
      } finally {
        if (!cancelled) setLoading(false)
      }

      if (cancelled) return

      // Phase 2: fetch records for charts in the background with retry logic
      let retryCount = 0
      const maxRetries = 3
      
      while (retryCount < maxRetries && !cancelled) {
        try {
          const records = await fetchRunRecords(paramId)
          if (cancelled) return
          // Set offset immediately so the first poll doesn't re-fetch these records
          recordOffsetRef.current = records.length
          setRun((prev) => {
            if (!prev) return null
            const count = records.length
            return {
              ...prev,
              records,
              avgCpuUsage: count > 0 ? records.reduce((s, r) => s + r.cpu_usage, 0) / count : 0,
              avgGpuUsage: count > 0 ? records.reduce((s, r) => s + r.gpu_usage, 0) / count : 0,
              avgMemUsage: count > 0 ? records.reduce((s, r) => s + r.mem_usage, 0) / count : 0,
            }
          })
          break // Success, exit retry loop
        } catch (error) {
          retryCount++
          console.error(`Failed to fetch records (attempt ${retryCount}/${maxRetries}):`, error)
          
          if (retryCount < maxRetries && !cancelled) {
            // Wait before retrying (exponential backoff)
            await new Promise(resolve => setTimeout(resolve, 1000 * Math.pow(2, retryCount - 1)))
          }
        }
      }
      
      if (!cancelled) setChartsLoading(false)
    }

    fetchRun()
    return () => { cancelled = true }
  }, [paramId])

  // Keep recordOffsetRef in sync with the latest loaded record count
  useEffect(() => {
    if (run) recordOffsetRef.current = run.records.length
  }, [run?.records.length])

  // Incremental poll: only fetches records added since the last poll
  useEffect(() => {
    if (!paramId || !run || run.status !== 'running') return

    const poll = async () => {
      try {
        const offset = recordOffsetRef.current
        const newRecords = await fetchRunRecords(paramId, 200, offset)
        if (newRecords.length === 0) return

        recordOffsetRef.current += newRecords.length
        setRun((prev) => {
          if (!prev) return null
          const merged = [...prev.records, ...newRecords]
          const count = merged.length
          return {
            ...prev,
            records: merged,
            avgCpuUsage: count > 0 ? merged.reduce((s, r) => s + r.cpu_usage, 0) / count : prev.avgCpuUsage,
            avgGpuUsage: count > 0 ? merged.reduce((s, r) => s + r.gpu_usage, 0) / count : prev.avgGpuUsage,
            avgMemUsage: count > 0 ? merged.reduce((s, r) => s + r.mem_usage, 0) / count : prev.avgMemUsage,
          }
        })
      } catch {
        // silently ignore poll errors
      }
    }

    // Also check if the run finished — on a separate slower interval to avoid overhead
    const statusCheck = async () => {
      try {
        const meta = await fetchRunMeta(paramId)
        if (meta.status !== 'running') {
          setRun((prev) => prev ? { ...prev, status: meta.status, totalEnergy: meta.totalEnergy, carbonFootprint: meta.carbonFootprint } : null)
        }
      } catch { /* ignore */ }
    }

    const recordInterval = setInterval(poll, POLL_INTERVAL_MS)
    const statusInterval = setInterval(statusCheck, 5000)
    return () => {
      clearInterval(recordInterval)
      clearInterval(statusInterval)
    }
  }, [paramId, run?.status])

  if (loading) {
    return (
      <DashboardLayout>
        <div className="p-4 md:p-6 lg:p-8 w-full">
          <div className="h-96 bg-card border border-border rounded-lg animate-pulse" />
        </div>
      </DashboardLayout>
    )
  }

  if (!run) {
    return (
      <DashboardLayout>
        <div className="p-4 md:p-6 lg:p-8 w-full">
          <div className="text-center py-12">
            <p className="text-muted-foreground mb-4">Run not found</p>
            <Link
              href={`/${workspaceSlug}/runs`}
              className="inline-flex items-center gap-2 text-primary hover:text-primary/80 transition-colors"
            >
              <ArrowLeft className="w-4 h-4" />
              Back to runs
            </Link>
          </div>
        </div>
      </DashboardLayout>
    )
  }

  const statusColor =
    run.status === 'finished'
      ? 'bg-emerald-500/20 text-emerald-300'
      : run.status === 'running'
        ? 'bg-blue-500/20 text-blue-300'
        : run.status === 'canceled'
          ? 'bg-amber-500/20 text-amber-300'
          : 'bg-red-500/20 text-red-300'

  return (
    <DashboardLayout>
      <div className="p-4 md:p-6 lg:p-8 w-full">
        {/* Header with back button */}
        <div className="mb-6 md:mb-8">
          <Link
            href={`/${workspaceSlug}/runs`}
            className="inline-flex items-center gap-2 text-primary hover:text-primary/80 transition-colors mb-4"
          >
            <ArrowLeft className="w-4 h-4" />
            Back to runs
          </Link>

          <div className="flex flex-col md:flex-row md:items-start md:justify-between gap-3 md:gap-4">
            <div className="flex-1">
              <h1 className="text-2xl md:text-3xl lg:text-4xl font-bold text-foreground mb-2">
                {run.name}
              </h1>
              <p className="text-sm md:text-base text-muted-foreground">
                {format(new Date(run.timestamp), 'EEEE, MMMM d, yyyy HH:mm')}
              </p>
              <p className="text-sm text-muted-foreground mt-1">
                By {run.collaboratorName ?? 'Unassigned'}
              </p>
              {run.command && (
                <div className="mt-4 p-3 bg-white/5 border border-white/10 rounded-lg font-mono text-xs md:text-sm break-all glass">
                  <span className="text-primary mr-2 font-bold">$</span>
                  <span className="text-foreground/90">{run.command}</span>
                </div>
              )}
            </div>
            <div className="flex flex-col items-end gap-2">
              <span className={`inline-block px-3 md:px-4 py-2 rounded-lg font-medium whitespace-nowrap ${statusColor}`}>
                {run.status}
              </span>
              {run.exporterType && (
                <span className="inline-block px-3 py-1 rounded-lg text-xs font-medium font-mono bg-secondary text-secondary-foreground">
                  {run.exporterType}
                </span>
              )}
            </div>
          </div>
        </div>

        {/* Two Column Layout for System Info and Environmental Impact */}
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6 mb-8">
          <div>
            <RunSystemInfo run={run} />
          </div>
          <div className="space-y-6">
            <div className="chart-container">
              <h2 className="text-lg font-semibold text-foreground mb-4">
                Environmental Equivalents
              </h2>
              <EnvironmentalImpact
                totalEnergyWh={run.totalEnergy}
                totalCo2G={run.carbonFootprint}
                carbonIntensity={run.carbonIntensityUsed}
                totalCost={run.totalCost}
                currency={run.currency}
              />
            </div>
            {run.records.length > 0 && (
              <PowerDistributionChart
                record={run.records[run.records.length - 1]}
                prevRecord={run.records.length > 1 ? run.records[run.records.length - 2] : undefined}
              />
            )}
          </div>
        </div>

        {/* Charts Grid */}
        {chartsLoading ? (
          <div className="mb-8 chart-container flex items-center justify-center py-16">
            <div className="flex flex-col items-center gap-3">
              <div className="w-8 h-8 border-4 border-primary border-t-transparent rounded-full animate-spin" />
              <p className="text-sm text-muted-foreground">Loading charts...</p>
            </div>
          </div>
        ) : run.records.length > 0 ? (
          <div className="mb-8 space-y-4 md:space-y-6">
            <EnergyChart data={run.records} />
            <div className="grid grid-cols-1 lg:grid-cols-2 gap-4 md:gap-6">
              <CumulativePowerChart data={run.records} />
              <UtilizationChart data={run.records} />
            </div>
          </div>
        ) : null}

        {/* Details Grid */}
        {!chartsLoading && run.records.length > 0 && (
          <div className="grid grid-cols-1 xl:grid-cols-2 gap-8">
            <EnergyBreakdownChart run={run} />
            <SocketEnergyChart data={run.records} />
          </div>
        )}
      </div>
    </DashboardLayout>
  )
}
