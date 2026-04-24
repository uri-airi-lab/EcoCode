'use client'

import { useEffect, useState, useMemo } from 'react'
import {
  Zap,
  Clock,
  Activity,
  Coins,
} from 'lucide-react'
import { DashboardLayout } from '@/components/DashboardLayout'
import { MetricCard } from '@/components/MetricCard'
import { EnergyChart } from '@/components/EnergyChart'
import { UtilizationChart } from '@/components/UtilizationChart'
import { ProjectsTable } from '@/components/ProjectsTable'
import { PowerDistributionChart } from '@/components/PowerDistributionChart'
import { CumulativePowerChart } from '@/components/CumulativePowerChart'
import { EnvironmentalImpact } from '@/components/EnvironmentalImpact'
import { CarbonSettings } from '@/components/CarbonSettings'
import { StartRunModal } from '@/components/StartRunModal'
import { Run } from '@/lib/mock-data'
import { fetchAllRuns, fetchRunDetail, fetchAllProjects, ProjectSummary } from '@/lib/api-client'
import { useSSE } from '@/hooks/use-sse'
import { useWorkspace } from '@/components/WorkspaceProvider'

export default function Dashboard() {
  const { currentWorkspace, loading: workspaceLoading } = useWorkspace()
  const [runs, setRuns] = useState<Omit<Run, 'records'>[]>([])
  const [projects, setProjects] = useState<ProjectSummary[]>([])
  const [currentRun, setCurrentRun] = useState<Run | null>(null)
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
        carbonFootprint: energyKwh * (prevRun.carbonIntensityUsed ?? 494) * (prevRun.pueUsed ?? 1.0),
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
      try {
        const projData = await fetchAllProjects()
        if (cancelled) return
        
        const filteredProjects = currentWorkspace 
            ? projData.filter(p => p.workspaceSlug === currentWorkspace.slug)
            : projData
        setProjects(filteredProjects)

        const runsData = await fetchAllRuns()
        if (cancelled) return
        
        const filteredRuns = currentWorkspace
            ? runsData.filter(r => filteredProjects.some(p => p.id === r.project_id.toString()))
            : runsData
        setRuns(filteredRuns)

        if (filteredRuns.length > 0) {
          const latestRunId = filteredRuns[0].id
          const runData = await fetchRunDetail(latestRunId)
          if (cancelled) return
          setCurrentRun(runData)
        } else {
          setCurrentRun(null)
        }
      } catch (error) {
        if (!cancelled) {
          console.error('Failed to fetch data:', error)
          setRuns([])
          setProjects([])
          setCurrentRun(null)
        }
      } finally {
        if (!cancelled) {
          setLoading(false)
        }
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
        <div className="mb-8">
          <h2 className="text-lg md:text-xl font-semibold text-foreground mb-4">
            Project Overview
          </h2>
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-3 md:gap-4">
            <MetricCard
              icon={Zap}
              label="Total Energy"
              value={Number(stats.totalEnergy.toFixed(2)).toLocaleString()}
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
            <MetricCard
              icon={Clock}
              label="Total Core Hours"
              value={(stats.totalCoreHours / 24).toFixed(2)}
              unit="hrs"
              loading={loading}
            />
            {/* <MetricCard
              icon={Leaf}
              label="Carbon Footprint"
              value={Number(stats.totalCarbonFootprint.toFixed(2)).toLocaleString()}
              unit="g CO₂"
              trend={
                trend
                  ? {
                    value: trend.carbonTrend,
                    direction: trend.carbonTrend > 0 ? 'up' : 'down',
                  }
                  : undefined
              }
              loading={loading}
            /> */}
            <MetricCard
              icon={Activity}
              label="Avg CPU Usage"
              value={stats.avgCpuUsage.toFixed(1)}
              unit="%"
              loading={loading}
            />
            <MetricCard
              icon={Activity}
              label="Avg GPU Usage"
              value={stats.avgGpuUsage.toFixed(1)}
              unit="%"
              loading={loading}
            />
            <MetricCard
              icon={Activity}
              label="Avg Memory Usage"
              value={stats.avgMemUsage.toFixed(1)}
              unit="%"
              loading={loading}
            />
            <MetricCard
              icon={Coins}
              label="Total Cost"
              value={stats.totalCost.toFixed(2)}
              unit={stats.currency}
              loading={loading}
              className="bg-amber-500/10 border-amber-500/20"
            />
          </div>
        </div>

        {/* Environmental Impact Translation
        <div className="mb-8">
          <h2 className="text-lg md:text-xl font-semibold text-foreground mb-4">
            Environmental Impact Equivalents
          </h2>
          <EnvironmentalImpact
            totalEnergyWh={stats.totalEnergy}
            totalCo2G={stats.totalCarbonFootprint}
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
        {currentRun && (
          <div className="mb-8 space-y-4 md:space-y-6">
            <div className="grid grid-cols-1 xl:grid-cols-3 gap-4 md:gap-6">
              <div className="xl:col-span-2">
                <EnergyChart data={currentRun.records} />
              </div>
              {currentRun.records.length > 0 && (
                <div className="xl:col-span-1">
                  <PowerDistributionChart 
                    record={currentRun.records[currentRun.records.length - 1]} 
                    prevRecord={currentRun.records.length > 1 ? currentRun.records[currentRun.records.length - 2] : undefined}
                  />
                </div>
              )}
            </div>

            <div className="grid grid-cols-1 lg:grid-cols-2 gap-4 md:gap-6">
              <CumulativePowerChart data={currentRun.records} />
              <UtilizationChart data={currentRun.records} />
            </div>
          </div>
        )}

        {/* Projects Table */}
        <ProjectsTable projects={projects} />
      </div>
    </DashboardLayout>
  )
}
