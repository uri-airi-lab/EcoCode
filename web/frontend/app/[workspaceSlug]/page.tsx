'use client'

import { useEffect, useState, useMemo } from 'react'
import {
  Zap,
  Clock,
  Activity,
  Coins,
  Leaf,
} from 'lucide-react'
import { DashboardLayout } from '@/components/layout/DashboardLayout'
import { MetricCard } from '@/components/MetricCard'
import { EnergyChart } from '@/components/charts/EnergyChart'
import { UtilizationChart } from '@/components/charts/UtilizationChart'
import { ProjectsTable } from '@/components/tables/ProjectsTable'
import { PowerDistributionChart } from '@/components/charts/PowerDistributionChart'
import { CumulativePowerChart } from '@/components/charts/CumulativePowerChart'
import { EnvironmentalImpact } from '@/components/run/EnvironmentalImpact'
import { CarbonSettings } from '@/components/run/CarbonSettings'
import { StartRunModal } from '@/components/StartRunModal'
import { Run, MetricRecord } from '@/lib/mock-data'
import { fetchAllRuns, fetchRunDetail, fetchAllProjects, ProjectSummary } from '@/lib/api'
import { useSSE } from '@/hooks/use-sse'
import { useWorkspace } from '@/components/layout/WorkspaceProvider'

export default function Dashboard() {
  const { currentWorkspace, loading: workspaceLoading } = useWorkspace()
  const [runs, setRuns] = useState<Omit<Run, 'records'>[]>([])
  const [projects, setProjects] = useState<ProjectSummary[]>([])
  const [currentRun, setCurrentRun] = useState<Run | null>(null)
  const [allRunsRecords, setAllRunsRecords] = useState<{ runId: string; runName: string; records: MetricRecord[] }[]>([])
  const [loading, setLoading] = useState(true)
  const { latestRecord } = useSSE()

  // Update current run with real-time SSE data
  useEffect(() => {
    if (!latestRecord) return

    setCurrentRun((prevRun) => {
      if (!prevRun) return null

      // Check if the record already exists to avoid duplicates
      if (prevRun.records.some((r) => r.id === latestRecord.id)) return prevRun

      const updatedRecords = [...prevRun.records, latestRecord]
      const count = updatedRecords.length

      // Calculate new total and averages incrementally (O(1) instead of O(N))
      // Conversion: Joules / 3600 = Wh
      const newTotalEnergy =
        prevRun.totalEnergy +
        (latestRecord.cpu_energy +
          latestRecord.gpu_energy +
          latestRecord.mem_energy +
          latestRecord.igpu_energy) / 3600.0;

      const energyKwh = newTotalEnergy / 1000

      // Incremental averages: (old_avg * old_count + new_val) / new_count
      const avgCpuUsage = (prevRun.avgCpuUsage * (count - 1) + latestRecord.cpu_usage) / count
      const avgGpuUsage = (prevRun.avgGpuUsage * (count - 1) + latestRecord.gpu_usage) / count
      const avgMemUsage = (prevRun.avgMemUsage * (count - 1) + latestRecord.mem_usage) / count

      return {
        ...prevRun,
        records: updatedRecords,
        totalEnergy: newTotalEnergy,
        carbonFootprint: energyKwh * (prevRun.carbonIntensityUsed ?? 445) * (prevRun.pueUsed ?? 1.0),
        avgCpuUsage,
        avgGpuUsage,
        avgMemUsage,
      }
    })
  }, [latestRecord])

  useEffect(() => {
    if (workspaceLoading) return
    let cancelled = false

    const fetchData = async () => {
      setLoading(true)
      let latestRunId: string | null = null

      // Phase 1: load summaries — finishes fast, shows metric cards + project table.
      try {
        const projData = await fetchAllProjects()
        if (cancelled) return

        const filteredProjects = currentWorkspace
          ? projData.filter(p => p.workspaceSlug === currentWorkspace.slug)
          : projData
        setProjects(filteredProjects)

        const allProjectIds = projData.map(p => Number(p.id))
        const runsData = await fetchAllRuns(allProjectIds)
        if (cancelled) return

        const filteredRuns = currentWorkspace
          ? runsData.filter(r => filteredProjects.some(p => p.id === r.projectId))
          : runsData
        setRuns(filteredRuns)

        if (filteredRuns.length > 0) latestRunId = filteredRuns[0].id
      } catch (error) {
        if (!cancelled) {
          console.error('Failed to fetch data:', error)
          setRuns([])
          setProjects([])
        }
      } finally {
        // Reveal the page now — metric cards and project table are ready.
        if (!cancelled) setLoading(false)
      }

      // Phase 2: load records for the chart — runs after the page is already visible.
      if (!latestRunId || cancelled) return
      try {
        const runData = await fetchRunDetail(latestRunId)
        if (!cancelled) {
          setCurrentRun(runData)
          setAllRunsRecords([{ runId: runData.id, runName: runData.name, records: runData.records }])
        }
      } catch {
        if (!cancelled) setCurrentRun(null)
      }
    }

    fetchData()

    return () => {
      cancelled = true
    }
  }, [currentWorkspace, workspaceLoading])

  // Calculate aggregate stats (memoized to avoid re-calculating on every real-time update)
  const stats = useMemo(() => {
    const totalEnergy = runs.reduce((sum, r) => sum + r.totalEnergy, 0)
    const totalCoreHours = runs.reduce((sum, r) => sum + r.totalCoreHours, 0)
    const avgCpuUsage =
      runs.length > 0
        ? runs.reduce((sum, r) => sum + r.avgCpuUsage, 0) / runs.length
        : 0
    const totalCarbonFootprint = runs.reduce((sum, r) => sum + r.carbonFootprint, 0)
    const totalCost = runs.reduce((sum, r) => sum + (r.totalCost || 0), 0)
    const currency = runs.find(r => r.currency)?.currency || 'USD'
    const avgMemUsage =
      runs.length > 0
        ? runs.reduce((sum, r) => sum + r.avgMemUsage || 0, 0) / runs.length
        : 0
    const avgGpuUsage =
      runs.length > 0
        ? runs.reduce((sum, r) => sum + r.avgGpuUsage, 0) / runs.length
        : 0

    return {
      totalEnergy,
      totalCoreHours,
      avgCpuUsage,
      totalCarbonFootprint,
      totalCost,
      currency,
      avgMemUsage,
      avgGpuUsage
    }
  }, [runs])

  // Calculate trends (comparing last two runs)
  const trend = useMemo(() => {
    if (runs.length < 2) return null

    const prevRun = runs[runs.length - 2]
    const lastRun = runs[runs.length - 1]

    return {
      energyTrend:
        prevRun.totalEnergy === 0
          ? lastRun.totalEnergy === 0 ? 0 : 100
          : ((prevRun.totalEnergy - lastRun.totalEnergy) / prevRun.totalEnergy) * 100,
      carbonTrend:
        prevRun.carbonFootprint === 0
          ? lastRun.carbonFootprint === 0 ? 0 : 100
          : ((prevRun.carbonFootprint - lastRun.carbonFootprint) / prevRun.carbonFootprint) * 100,
    }
  }, [runs])

  return (
    <DashboardLayout>
      <div className="p-4 md:p-6 lg:p-8 w-full">
        {/* Header */}
        <div className="mb-6 md:mb-8">
          <div className="flex flex-col md:flex-row md:items-center justify-between gap-4 mb-2">
            <div>
              <h1 className="text-2xl md:text-3xl lg:text-4xl font-bold text-foreground mb-2">
                Dashboard
              </h1>
              <p className="text-sm md:text-base text-muted-foreground">
                Monitor your EcoCode runs and environmental impact
              </p>
            </div>
            <StartRunModal 
              projects={projects} 
              onProjectCreated={() => {
                fetchAllProjects().then(setProjects)
              }} 
            />
          </div>
        </div>


        {/* Project Overview */}
        {/* Metrics Row */}
        <div className="grid grid-cols-1 lg:grid-cols-12 gap-0 items-stretch border border-border rounded-xl overflow-hidden mb-12">
          <section className="lg:col-span-7 p-6 bg-emerald-500/[0.02]">
            <h4 className="text-xs font-bold text-emerald-500 uppercase tracking-widest mb-4 flex items-center gap-2">
              <div className="w-1.5 h-1.5 rounded-full bg-emerald-500" />
              Impact Summary
            </h4>
            <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
              <MetricCard
                icon={Leaf}
                label="Carbon Footprint"
                value={stats.totalCarbonFootprint > 1000 ? (stats.totalCarbonFootprint / 1000).toFixed(4) : stats.totalCarbonFootprint.toFixed(4)}
                unit={stats.totalCarbonFootprint > 1000 ? 'kg CO₂' : 'g CO₂'}
                trend={
                  trend
                    ? {
                      value: trend.carbonTrend,
                      direction: trend.carbonTrend > 0 ? 'up' : 'down',
                    }
                    : undefined
                }
                loading={loading}
                className="bg-emerald-500/10 border-emerald-500/20"
              />
              <MetricCard
                icon={Coins}
                label="Total Cost"
                value={stats.totalCost.toFixed(4)}
                unit={stats.currency}
                loading={loading}
                className="bg-amber-500/10 border-amber-500/20"
              />
              <MetricCard
                icon={Zap}
                label="Total Energy"
                value={stats.totalEnergy.toFixed(4)}
                unit="Wh"
                trend={
                  trend
                    ? {
                      value: trend.energyTrend,
                      direction: trend.energyTrend > 0 ? 'up' : 'down',
                    }
                    : undefined
                }
                loading={loading}
              />
            </div>
          </section>

          <section className="lg:col-span-5 p-6 bg-blue-500/[0.02] border-t lg:border-t-0 lg:border-l border-border">
            <h4 className="text-xs font-bold text-blue-400 uppercase tracking-widest mb-4 flex items-center gap-2">
              <div className="w-1.5 h-1.5 rounded-full bg-blue-500" />
              Workspace Stats
            </h4>
            <div className="grid grid-cols-2 sm:grid-cols-2 gap-4">
              <MetricCard
                icon={Clock}
                label="Core Hours"
                value={(stats.totalCoreHours / 24).toFixed(4)}
                unit="hrs"
                loading={loading}
                className="bg-blue-500/5 border-blue-500/10"
              />
              <MetricCard icon={Activity} label="Avg CPU" value={stats.avgCpuUsage.toFixed(4)} unit="%" loading={loading} className="bg-blue-500/5 border-blue-500/10" />
              <MetricCard icon={Activity} label="Avg GPU" value={stats.avgGpuUsage.toFixed(4)} unit="%" loading={loading} className="bg-blue-500/5 border-blue-500/10" />
              <MetricCard icon={Activity} label="Avg Mem" value={stats.avgMemUsage.toFixed(4)} unit="%" loading={loading} className="bg-blue-500/5 border-blue-500/10" />
            </div>
          </section>
        </div>

        {/* Sustainability Impact */}
        <div className="mb-12">
          <h2 className="text-xl font-bold text-foreground mb-6 flex items-center gap-2">
            <Leaf className="w-5 h-5 text-primary" />
            Sustainability Impact
          </h2>
          <EnvironmentalImpact
            totalEnergyWh={stats.totalEnergy}
            totalCo2G={stats.totalCarbonFootprint}
            totalCost={stats.totalCost}
            currency={stats.currency}
          />
        </div>

        {/* Carbon Settings */}
        {/* {projects.length > 0 && (
          <div className="mb-8">
            <h2 className="text-lg md:text-xl font-semibold text-foreground mb-4">
              Carbon & Energy Settings
            </h2>
            <CarbonSettings projectId={Number(projects[0].id)} />
          </div>
        )} */}

        {/* Charts */}
        {loading ? (
          <div className="mb-8">
            <div className="chart-container flex items-center justify-center py-12">
              <div className="flex flex-col items-center gap-3">
                <div className="w-8 h-8 border-4 border-primary border-t-transparent rounded-full animate-spin"></div>
                <p className="text-sm text-muted-foreground">Loading dashboard data...</p>
              </div>
            </div>
          </div>
        ) : (allRunsRecords.length > 0 || currentRun) ? (
          <div className="mb-8 space-y-4 md:space-y-6">
            <div className="grid grid-cols-1 xl:grid-cols-3 gap-4 md:gap-6">
              <div className="xl:col-span-2">
                <EnergyChart runs={allRunsRecords.length > 0 ? allRunsRecords : (currentRun ? [{ runId: currentRun.id, runName: currentRun.name, records: currentRun.records }] : [])} />
              </div>
              {currentRun && currentRun.records.length > 0 && (
                <div className="xl:col-span-1">
                  <PowerDistributionChart 
                    record={currentRun.records[currentRun.records.length - 1]} 
                    prevRecord={currentRun.records.length > 1 ? currentRun.records[currentRun.records.length - 2] : undefined}
                  />
                </div>
              )}
            </div>

            <div className="grid grid-cols-1 lg:grid-cols-2 gap-4 md:gap-6">
              <CumulativePowerChart runs={allRunsRecords.length > 0 ? allRunsRecords : (currentRun ? [{ runId: currentRun.id, runName: currentRun.name, records: currentRun.records }] : [])} />
              <UtilizationChart runs={allRunsRecords.length > 0 ? allRunsRecords : (currentRun ? [{ runId: currentRun.id, runName: currentRun.name, records: currentRun.records }] : [])} />
            </div>
          </div>
        ) : null}

        {/* Projects Table */}
        <ProjectsTable projects={projects} loading={loading} />
      </div>
    </DashboardLayout>
  )
}
