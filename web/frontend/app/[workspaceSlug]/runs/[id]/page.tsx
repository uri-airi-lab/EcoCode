'use client'

import { useCallback, useEffect, useRef, useState } from 'react'
import { useParams } from 'next/navigation'
import Link from 'next/link'
import { ArrowLeft, Leaf, Coins, Zap } from 'lucide-react'
import { DashboardLayout } from '@/components/layout/DashboardLayout'
import { EnergyChart } from '@/components/charts/EnergyChart'
import { UtilizationChart } from '@/components/charts/UtilizationChart'
import { EnergyBreakdownChart } from '@/components/charts/EnergyBreakdownChart'
import { SocketEnergyChart } from '@/components/charts/SocketEnergyChart'
import { RunSystemInfo } from '@/components/run/RunSystemInfo'
import { PowerDistributionChart } from '@/components/charts/PowerDistributionChart'
import { CumulativePowerChart } from '@/components/charts/CumulativePowerChart'
import { EnvironmentalImpact } from '@/components/run/EnvironmentalImpact'
import { MLMetricsBar } from '@/components/run/MLMetricsBar'
import { PhaseTimeline } from '@/components/charts/PhaseTimeline'
import { TrainingMetricsChart } from '@/components/charts/TrainingMetricsChart'
import { Run, MetricRecord } from '@/lib/mock-data'
import { fetchRunMeta, fetchRunRecords, fetchCarbonConfig, CarbonConfig } from '@/lib/api'
import { BackendRecordPoint, mapRecord } from '@/lib/api/client'
import { format } from 'date-fns'
import { minIntervalMs } from '@/lib/utils'
import { buildDirectSseUrl } from '@/lib/data-source'

// Keep at most this many records in the chart display window.
// The offset ref tracks the true backend count independently, so capping
// here does not affect SSE/polling correctness.
const MAX_DISPLAY_RECORDS = 3000

interface RunDetailPageProps {
  params: Promise<{ id: string }>
}

export default function RunDetailPage({ params }: RunDetailPageProps) {
  const routeParams = useParams()
  const workspaceSlug = routeParams.workspaceSlug as string
  const [run, setRun] = useState<Run | null>(null)
  const [carbonConfig, setCarbonConfig] = useState<CarbonConfig | null>(null)
  const [loading, setLoading] = useState(true)
  const [chartsLoading, setChartsLoading] = useState(true)
  const [paramId, setParamId] = useState<string | null>(null)
  // Tracks how many records we already have so incremental polls only fetch the diff
  const recordOffsetRef = useRef(0)
  // Deduplication: tracks record IDs already merged so SSE + polling don't double-count
  const seenIdsRef = useRef<Set<string>>(new Set())

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

        fetchCarbonConfig(Number(meta.projectId))
          .then((config) => {
            if (!cancelled) setCarbonConfig(config)
          })
          .catch((err) => {
            console.error('Failed to fetch carbon config:', err)
          })
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
          // Set offset + seed dedup set so SSE/polling don't re-count these records
          recordOffsetRef.current = records.length
          seenIdsRef.current = new Set(records.map(r => r.id))
          setRun((prev) => {
            if (!prev) return null
            const count = records.length
            // Recompute energy (J → Wh) and carbon from records so the panel
            // shows real values even before the run summary is written.
            const totalCpuEnergy  = records.reduce((s, r) => s + r.cpu_energy,  0) / 3600
            const totalGpuEnergy  = records.reduce((s, r) => s + r.gpu_energy,  0) / 3600
            const totalMemEnergy  = records.reduce((s, r) => s + r.mem_energy,  0) / 3600
            const totalIgpuEnergy = records.reduce((s, r) => s + r.igpu_energy, 0) / 3600
            const totalEnergy = totalCpuEnergy + totalGpuEnergy + totalMemEnergy + totalIgpuEnergy
            const carbonFootprint = (totalEnergy / 1000) * (prev.carbonIntensityUsed ?? 445)
            return {
              ...prev,
              records,
              totalEnergy,
              totalCpuEnergy,
              totalGpuEnergy,
              totalMemEnergy,
              totalIgpuEnergy,
              carbonFootprint,
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

  // Helper: merge newly-fetched records into the existing run state.
  // Deduplicates against seenIdsRef so SSE + polling never double-count.
  const mergeRecords = useCallback((incoming: MetricRecord[]) => {
    const fresh = incoming.filter(r => !seenIdsRef.current.has(r.id))
    if (fresh.length === 0) return
    for (const r of fresh) seenIdsRef.current.add(r.id)
    const prevOffset = recordOffsetRef.current
    recordOffsetRef.current += fresh.length
    setRun((prev) => {
      if (!prev) return null
      const addCpu  = fresh.reduce((s, r) => s + r.cpu_energy,  0) / 3600
      const addGpu  = fresh.reduce((s, r) => s + r.gpu_energy,  0) / 3600
      const addMem  = fresh.reduce((s, r) => s + r.mem_energy,  0) / 3600
      const addIgpu = fresh.reduce((s, r) => s + r.igpu_energy, 0) / 3600
      const totalCpuEnergy  = prev.totalCpuEnergy  + addCpu
      const totalGpuEnergy  = prev.totalGpuEnergy  + addGpu
      const totalMemEnergy  = prev.totalMemEnergy  + addMem
      const totalIgpuEnergy = prev.totalIgpuEnergy + addIgpu
      const totalEnergy = totalCpuEnergy + totalGpuEnergy + totalMemEnergy + totalIgpuEnergy
      const carbonFootprint = (totalEnergy / 1000) * (prev.carbonIntensityUsed ?? 445)
      const newCount = prevOffset + fresh.length
      const avgCpuUsage = newCount > 0
        ? (prev.avgCpuUsage * prevOffset + fresh.reduce((s, r) => s + r.cpu_usage, 0)) / newCount
        : prev.avgCpuUsage
      const avgGpuUsage = newCount > 0
        ? (prev.avgGpuUsage * prevOffset + fresh.reduce((s, r) => s + r.gpu_usage, 0)) / newCount
        : prev.avgGpuUsage
      const avgMemUsage = newCount > 0
        ? (prev.avgMemUsage * prevOffset + fresh.reduce((s, r) => s + r.mem_usage, 0)) / newCount
        : prev.avgMemUsage
      const allRecords = [...prev.records, ...fresh]
      const records = allRecords.length > MAX_DISPLAY_RECORDS
        ? allRecords.slice(-MAX_DISPLAY_RECORDS)
        : allRecords
      return { ...prev, records, totalEnergy, totalCpuEnergy, totalGpuEnergy, totalMemEnergy, totalIgpuEnergy, carbonFootprint, avgCpuUsage, avgGpuUsage, avgMemUsage }
    })
  }, [])

  // Live updates: SSE (real-time push) + polling fallback (catches missed records).
  useEffect(() => {
    if (!paramId || !run || run.status !== 'running') return

    // --- SSE: real-time push from backend ---
    let sseOpen = false
    const es = new EventSource(buildDirectSseUrl('/api/sse'))
    let hasConnected = false

    es.onopen = () => {
      sseOpen = true
      if (hasConnected && recordOffsetRef.current > 0) {
        fetchRunRecords(paramId, 200, recordOffsetRef.current)
          .then(mergeRecords)
          .catch(() => {})
      }
      hasConnected = true
    }

    es.onmessage = (e) => {
      let raw: BackendRecordPoint
      try { raw = JSON.parse(e.data) as BackendRecordPoint } catch { return }
      if (raw.run_id !== Number(paramId)) return

      const record = mapRecord(raw)
      const rid = record.id
      if (seenIdsRef.current.has(rid)) return
      seenIdsRef.current.add(rid)
      recordOffsetRef.current += 1
      const newOffset = recordOffsetRef.current

      setRun((prev) => {
        if (!prev) return null
        const totalCpuEnergy  = prev.totalCpuEnergy  + record.cpu_energy  / 3600
        const totalGpuEnergy  = prev.totalGpuEnergy  + record.gpu_energy  / 3600
        const totalMemEnergy  = prev.totalMemEnergy  + record.mem_energy  / 3600
        const totalIgpuEnergy = prev.totalIgpuEnergy + record.igpu_energy / 3600
        const totalEnergy = totalCpuEnergy + totalGpuEnergy + totalMemEnergy + totalIgpuEnergy
        const carbonFootprint = (totalEnergy / 1000) * (prev.carbonIntensityUsed ?? 445)
        const prevCount = newOffset - 1
        const avgCpuUsage = newOffset > 0
          ? (prev.avgCpuUsage * prevCount + record.cpu_usage) / newOffset
          : prev.avgCpuUsage
        const avgGpuUsage = newOffset > 0
          ? (prev.avgGpuUsage * prevCount + record.gpu_usage) / newOffset
          : prev.avgGpuUsage
        const avgMemUsage = newOffset > 0
          ? (prev.avgMemUsage * prevCount + record.mem_usage) / newOffset
          : prev.avgMemUsage
        const allRecords = [...prev.records, record]
        const records = allRecords.length > MAX_DISPLAY_RECORDS
          ? allRecords.slice(-MAX_DISPLAY_RECORDS)
          : allRecords
        return { ...prev, records, totalEnergy, totalCpuEnergy, totalGpuEnergy, totalMemEnergy, totalIgpuEnergy, carbonFootprint, avgCpuUsage, avgGpuUsage, avgMemUsage }
      })
    }

    es.onerror = () => { sseOpen = false }

    // --- Polling fallback: only runs when SSE is disconnected ---
    const poll = async () => {
      if (sseOpen) return
      try {
        const newRecords = await fetchRunRecords(paramId, 200, recordOffsetRef.current)
        mergeRecords(newRecords)
      } catch { /* ignore */ }
    }
    const pollInterval = setInterval(poll, 1000)

    // --- Status poll: detect run completion ---
    const statusCheck = async () => {
      try {
        const meta = await fetchRunMeta(paramId)
        if (meta.status !== 'running') {
          setRun((prev) => prev ? { ...prev, status: meta.status, totalEnergy: meta.totalEnergy, carbonFootprint: meta.carbonFootprint } : null)
        }
      } catch { /* ignore */ }
    }
    const statusInterval = setInterval(statusCheck, 5000)

    return () => {
      es.close()
      clearInterval(pollInterval)
      clearInterval(statusInterval)
    }
  }, [paramId, run?.status, mergeRecords])

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

  const derivedPrice = run.totalCost !== null && run.totalCost !== undefined && run.totalEnergy > 0
    ? run.totalCost / ((run.totalEnergy * (run.pueUsed ?? 1.0)) / 1000.0)
    : null

  const statusColor =
    run.status === 'finished'
      ? 'bg-emerald-500/20 text-emerald-300'
      : run.status === 'running'
        ? 'bg-green-500/20 text-green-300'
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
              {run.records.length > 1 && (
                <span className="inline-block px-3 py-1 rounded-lg text-xs font-medium font-mono bg-secondary text-secondary-foreground">
                  Interval: {(minIntervalMs(run.records.map(r => r.timestamp)) / 1000).toFixed(2)}s
                </span>
              )}
            </div>
          </div>
        </div>

        {/* ML Training Overview — shown only when ML fields are present */}
        {run.records.length > 0 && (
          <div className="mb-6">
            <MLMetricsBar records={run.records} />
          </div>
        )}

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

            <div className="chart-container">
              <h2 className="text-lg font-semibold text-foreground mb-4">
                Calculation Settings Used
              </h2>
              <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
                <div className="p-4 rounded-lg border border-border/40 bg-primary/[0.02] flex flex-col items-center justify-center text-center group hover:bg-primary/[0.05] transition-all duration-300">
                  <Leaf className="w-4 h-4 text-muted-foreground/60 mb-2 group-hover:text-primary transition-colors" />
                  <p className="text-[9px] font-bold text-muted-foreground/40 uppercase tracking-tighter mb-1">Carbon Intensity</p>
                  <p className="text-sm font-mono font-bold text-foreground">
                    {run.carbonIntensityUsed !== null && run.carbonIntensityUsed !== undefined
                      ? `${run.carbonIntensityUsed.toFixed(2)} g/kWh`
                      : carbonConfig
                      ? `${carbonConfig.resolved_intensity_g_per_kwh.toFixed(2)} g/kWh`
                      : '445.00 g/kWh (Default)'}
                  </p>
                </div>
                
                <div className="p-4 rounded-lg border border-border/40 bg-primary/[0.02] flex flex-col items-center justify-center text-center group hover:bg-primary/[0.05] transition-all duration-300">
                  <Coins className="w-4 h-4 text-muted-foreground/60 mb-2 group-hover:text-primary transition-colors" />
                  <p className="text-[9px] font-bold text-muted-foreground/40 uppercase tracking-tighter mb-1">Energy Price Rate</p>
                  <p className="text-sm font-mono font-bold text-foreground">
                    {derivedPrice !== null
                      ? `${derivedPrice.toFixed(4)} ${run.currency || 'USD'}`
                      : carbonConfig?.manual_cost_per_kwh !== null && carbonConfig?.manual_cost_per_kwh !== undefined
                      ? `${carbonConfig.manual_cost_per_kwh.toFixed(4)} ${carbonConfig.manual_currency || 'USD'}`
                      : 'Not Configured'}
                  </p>
                </div>

                <div className="p-4 rounded-lg border border-border/40 bg-primary/[0.02] flex flex-col items-center justify-center text-center group hover:bg-primary/[0.05] transition-all duration-300">
                  <Zap className="w-4 h-4 text-muted-foreground/60 mb-2 group-hover:text-primary transition-colors" />
                  <p className="text-[9px] font-bold text-muted-foreground/40 uppercase tracking-tighter mb-1">PUE Factor</p>
                  <p className="text-sm font-mono font-bold text-foreground">
                    {run.pueUsed !== null && run.pueUsed !== undefined
                      ? run.pueUsed.toFixed(2)
                      : carbonConfig
                      ? carbonConfig.pue_factor.toFixed(2)
                      : '1.00'}
                  </p>
                </div>
              </div>
            </div>
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
            <PhaseTimeline records={run.records} />
            <TrainingMetricsChart records={run.records} />
            <EnergyChart data={run.records} runId={run.id} />
            <div className="grid grid-cols-1 lg:grid-cols-2 gap-4 md:gap-6">
              <CumulativePowerChart data={run.records} runId={run.id} />
              <UtilizationChart data={run.records} runId={run.id} />
            </div>
          </div>
        ) : null}

        {/* Details Grid */}
        {!chartsLoading && run.records.length > 0 && (
          <div className="grid grid-cols-1 xl:grid-cols-2 gap-8">
            <EnergyBreakdownChart run={run} />
            <SocketEnergyChart data={run.records} runId={paramId ?? undefined} />
          </div>
        )}
      </div>
    </DashboardLayout>
  )
}
