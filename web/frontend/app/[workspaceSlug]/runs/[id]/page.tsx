'use client'

import { useEffect, useState } from 'react'
import { useParams } from 'next/navigation'
import Link from 'next/link'
import { ArrowLeft, Zap } from 'lucide-react'
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
import { fetchRunDetail } from '@/lib/api'
import { useSSE } from '@/hooks/use-sse'
import { format } from 'date-fns'

interface RunDetailPageProps {
  params: Promise<{ id: string }>
}

export default function RunDetailPage({ params }: RunDetailPageProps) {
  const routeParams = useParams()
  const workspaceSlug = routeParams.workspaceSlug as string
  const [run, setRun] = useState<Run | null>(null)
  const [loading, setLoading] = useState(true)
  const [paramId, setParamId] = useState<string | null>(null)
  const { latestRecord } = useSSE()

  useEffect(() => {
    const initParams = async () => {
      const { id } = await params
      setParamId(id)
    }
    initParams()
  }, [params])

  // Update current run with real-time SSE data
  useEffect(() => {
    if (!latestRecord || !run || latestRecord.run_id !== paramId) {
      if (latestRecord) {
        console.log('[RunDetail] SSE record received but filtered:', {
          hasRun: !!run,
          recordRunId: latestRecord.run_id,
          paramId,
          match: latestRecord.run_id === paramId
        })
      }
      return
    }

    console.log('[RunDetail] Processing SSE record:', latestRecord.id)

    setRun((prevRun) => {
      if (!prevRun) return null

      // Check if the record already exists to avoid duplicates
      if (prevRun.records.some((r) => r.id === latestRecord.id)) return prevRun

      const updatedRecords = [...prevRun.records, latestRecord]
      const count = updatedRecords.length

      // Calculate new total energy incrementally (O(1))
      // Conversion: Joules / 3600 = Wh
      const newTotalEnergy =
        prevRun.totalEnergy +
        ((latestRecord.cpu_energy ?? 0) +
          (latestRecord.gpu_energy ?? 0) +
          (latestRecord.mem_energy ?? 0) +
          (latestRecord.igpu_energy ?? 0)) / 3600.0;

      const energyKwh = newTotalEnergy / 1000

      // Incremental averages
      const avgCpuUsage = (prevRun.avgCpuUsage * (count - 1) + (latestRecord.cpu_usage ?? 0)) / count
      const avgGpuUsage = (prevRun.avgGpuUsage * (count - 1) + (latestRecord.gpu_usage ?? 0)) / count
      const avgMemUsage = (prevRun.avgMemUsage * (count - 1) + (latestRecord.mem_usage ?? 0)) / count

      return {
        ...prevRun,
        records: updatedRecords,
        totalEnergy: newTotalEnergy,
        carbonFootprint: energyKwh * (prevRun.carbonIntensityUsed ?? 494) * (prevRun.pueUsed ?? 1.0),
        avgCpuUsage,
        avgGpuUsage,
        avgMemUsage,
        status: 'running', // Definitely running if we're getting live data
      }
    })
  }, [latestRecord, paramId])

  useEffect(() => {
    if (!paramId) return

    let cancelled = false

    const fetchRun = async () => {
      setLoading(true)
      try {
        const data = await fetchRunDetail(paramId)
        if (!cancelled) {
          setRun(data)
        }
      } catch (error) {
        if (!cancelled) {
          console.error('Failed to fetch run:', error)
          setRun(null)
        }
      } finally {
        if (!cancelled) {
          setLoading(false)
        }
      }
    }

    fetchRun()

    return () => {
      cancelled = true
    }
  }, [paramId])

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
            <span className={`inline-block px-3 md:px-4 py-2 rounded-lg font-medium whitespace-nowrap ${statusColor}`}>
              {run.status}
            </span>
          </div>
        </div>

        {/* Two Column Layout for System Info and Environmental Impact */}
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6 mb-8">
          {/* Left Column - System Info */}
          <div>
            <RunSystemInfo run={run} />
          </div>

          {/* Right Column - Environmental Impact + Power Distribution */}
          <div className="space-y-6">
            <div className="chart-container">
              <h2 className="text-lg font-semibold text-foreground mb-4 flex items-center gap-2">
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

            {/* Live Power Distribution */}
            {run.records.length > 0 && (
              <PowerDistributionChart 
                record={run.records[run.records.length - 1]} 
                prevRecord={run.records.length > 1 ? run.records[run.records.length - 2] : undefined}
              />
            )}
          </div>
        </div>

        {/* Charts Grid */}
        <div className="mb-8 space-y-4 md:space-y-6">
          {/* Energy Chart - Full Width */}
          <EnergyChart data={run.records} />
          
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-4 md:gap-6">
            <CumulativePowerChart data={run.records} />
            <UtilizationChart data={run.records} />
          </div>
        </div>

        {/* Details Grid */}
        <div className="grid grid-cols-1 xl:grid-cols-2 gap-8">
          <EnergyBreakdownChart run={run} />
          <SocketEnergyChart data={run.records} />
        </div>
      </div>
    </DashboardLayout>
  )
}
