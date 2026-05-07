'use client'

import { useEffect, useRef, useState, useMemo } from 'react'
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
import { Run } from '@/lib/mock-data'
import { fetchAllRuns, fetchRunDetail, fetchRunRecords, fetchAllProjects, ProjectSummary } from '@/lib/api'
import { useWorkspace } from '@/components/layout/WorkspaceProvider'

const POLL_INTERVAL_MS = 1000

export default function Dashboard() {
  const { currentWorkspace, loading: workspaceLoading } = useWorkspace()
  const [runs, setRuns] = useState<Omit<Run, 'records'>[]>([])
  const [projects, setProjects] = useState<ProjectSummary[]>([])
  const [currentRun, setCurrentRun] = useState<Run | null>(null)
  const [loading, setLoading] = useState(true)
  const recordOffsetRef = useRef(0)

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
          recordOffsetRef.current = runData.records.length
          setCurrentRun(runData)
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

  // Keep offset ref in sync with currentRun's record count
  useEffect(() => {
    if (currentRun) recordOffsetRef.current = currentRun.records.length
  }, [currentRun?.records.length])

  // Incremental poll: only fetches new records added since the last poll
  useEffect(() => {
    if (!currentRun || currentRun.status !== 'running') return

    const poll = async () => {
      try {
        const offset = recordOffsetRef.current
        const newRecords = await fetchRunRecords(currentRun.id, 200, offset)
        if (newRecords.length === 0) return

        recordOffsetRef.current += newRecords.length
        setCurrentRun((prev) => {
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
      } catch { /* ignore */ }
    }

    const id = setInterval(poll, POLL_INTERVAL_MS)
    return () => clearInterval(id)
  }, [currentRun?.id, currentRun?.status])

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
              <div className="flex items-center gap-2 mb-1">
                <h1 className="text-2xl md:text-3xl lg:text-4xl font-bold tracking-tight text-foreground">
                  System Overview
                </h1>
                <div className="px-2 py-0.5 rounded text-[10px] font-bold bg-primary/10 text-primary uppercase tracking-tighter flex items-center gap-1 animate-pulse">
                  <div className="w-1 h-1 rounded-full bg-primary" />
                  Live
                </div>
              </div>
              <p className="text-sm md:text-base text-muted-foreground/80 max-w-2xl">
                Real-time monitoring of energy consumption, computational efficiency, and environmental impact metrics.
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
        <div className="grid grid-cols-1 lg:grid-cols-12 gap-0 items-stretch border border-border rounded-xl overflow-hidden mb-12 glass-strong">
          <section className="lg:col-span-7 p-6 border-b lg:border-b-0 lg:border-r border-border relative">
            <div className="absolute top-0 left-0 w-full h-1 bg-primary/20" />
            <h4 className="text-[10px] font-bold text-primary uppercase tracking-[0.2em] mb-6 flex items-center gap-2">
              <Activity className="w-3 h-3" />
              Computational Impact
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
                className="bg-primary/5 border-primary/10"
              />
              <MetricCard
                icon={Coins}
                label="Total Cost"
                value={stats.totalCost.toFixed(4)}
                unit={stats.currency}
                loading={loading}
                className="bg-primary/5 border-primary/10"
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
                className="bg-primary/5 border-primary/10"
              />
            </div>
          </section>

          <section className="lg:col-span-5 p-6 relative">
            <div className="absolute top-0 left-0 w-full h-1 bg-primary/10" />
            <h4 className="text-[10px] font-bold text-muted-foreground uppercase tracking-[0.2em] mb-6 flex items-center gap-2">
              <Zap className="w-3 h-3" />
              Workspace Efficiency
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
        ) : currentRun ? (
          <div className="mb-8 space-y-4 md:space-y-6">
            {(() => {
              const runData = [{ runId: currentRun.id, runName: currentRun.name, records: currentRun.records }]
              return (
                <>
                  <div className="grid grid-cols-1 xl:grid-cols-3 gap-4 md:gap-6">
                    <div className="xl:col-span-2">
                      <EnergyChart runs={runData} />
                    </div>

                  </div>
                  <div className="grid grid-cols-1 lg:grid-cols-2 gap-4 md:gap-6">
                    <CumulativePowerChart runs={runData} />
                    <UtilizationChart runs={runData} />
                  </div>
                </>
              )
            })()}
          </div>
        ) : null}

        {/* Projects Table */}
        <ProjectsTable projects={projects} loading={loading} />
      </div>
    </DashboardLayout>
  )
}
