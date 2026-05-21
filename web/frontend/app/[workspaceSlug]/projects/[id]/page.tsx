'use client'

import { useEffect, useState } from 'react'
import Link from 'next/link'
import { Activity, ArrowLeft, Folder, Zap, Clock } from 'lucide-react'
import { DashboardLayout } from '@/components/layout/DashboardLayout'
import { RecentRunsTable } from '@/components/tables/RecentRunsTable'
import { ProjectDetail, fetchProjectRuns, fetchAllProjects, ProjectSummary, fetchProjectRunsPaginated, RunSummary } from '@/lib/api'
import { useParams } from 'next/navigation'
import { MetricCard } from '@/components/MetricCard'
import { CarbonSettings } from '@/components/run/CarbonSettings'
import { StartRunModal } from '@/components/StartRunModal'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Leaf, Coins } from 'lucide-react'
import { EnvironmentalImpact } from '@/components/run/EnvironmentalImpact'

interface ProjectPageProps {
    params: Promise<{ id: string }>
}

export default function ProjectPage({ params }: ProjectPageProps) {
    const routeParams = useParams()
    const workspaceSlug = routeParams.workspaceSlug as string
    const [project, setProject] = useState<ProjectDetail | null>(null)
    const [allProjects, setAllProjects] = useState<ProjectSummary[]>([])
    const [loading, setLoading] = useState(true)
    const [paramId, setParamId] = useState<string | null>(null)
    
    // Pagination state
    const [page, setPage] = useState(1)
    const [paginatedRuns, setPaginatedRuns] = useState<RunSummary[]>([])
    const [totalRunsCount, setTotalRunsCount] = useState(0)
    const [runsLoading, setRunsLoading] = useState(false)
    const pageSize = 10

    useEffect(() => {
        const initParams = async () => {
            const { id } = await params
            setParamId(id)
            fetchAllProjects().then(setAllProjects)
        }
        initParams()
    }, [params])

    useEffect(() => {
        if (!paramId) return

        let cancelled = false

        const loadProject = async () => {
            setLoading(true)
            try {
                const data = await fetchProjectRuns(paramId)
                if (!cancelled) {
                    setProject(data)
                }
            } catch (error) {
                if (!cancelled) {
                    console.error('Failed to fetch project:', error)
                    setProject(null)
                }
            } finally {
                if (!cancelled) {
                    setLoading(false)
                }
            }
        }
        loadProject()

        return () => {
            cancelled = true
        }
    }, [paramId])

    useEffect(() => {
        if (!paramId) return

        let cancelled = false

        const loadRuns = async () => {
            setRunsLoading(true)
            try {
                const { runs, total } = await fetchProjectRunsPaginated(paramId, page, pageSize)
                if (!cancelled) {
                    setPaginatedRuns(runs)
                    setTotalRunsCount(total)
                }
            } catch (error) {
                console.error('Failed to fetch paginated runs:', error)
            } finally {
                if (!cancelled) {
                    setRunsLoading(false)
                }
            }
        }
        loadRuns()

        return () => {
            cancelled = true
        }
    }, [paramId, page])

    if (loading) {
        return (
            <DashboardLayout>
                <div className="p-4 md:p-6 lg:p-8 w-full">
                    <div className="h-96 bg-card border border-border rounded-lg animate-pulse" />
                </div>
            </DashboardLayout>
        )
    }

    if (!project) {
        return (
            <DashboardLayout>
                <div className="p-4 md:p-6 lg:p-8 w-full">
                    <div className="text-center py-12">
                        <p className="text-muted-foreground mb-4">Project not found</p>
                        <Link
                            href={`/${workspaceSlug}`}
                            className="inline-flex items-center gap-2 text-primary hover:text-primary/80 transition-colors"
                        >
                            <ArrowLeft className="w-4 h-4" />
                            Back to Dashboard
                        </Link>
                    </div>
                </div>
            </DashboardLayout>
        )
    }

    const totalCoreHours = project.runs.reduce((sum, r) => sum + r.totalCoreHours, 0)

    return (
        <DashboardLayout>
            <div className="p-4 md:p-6 lg:p-8 w-full">
                <div className="mb-6 md:mb-8">
                    <Link
                        href={`/${workspaceSlug}`}
                        className="inline-flex items-center gap-2 text-primary hover:text-primary/80 transition-colors mb-4"
                    >
                        <ArrowLeft className="w-4 h-4" />
                        Back to Dashboard
                    </Link>

                    <div className="flex flex-col md:flex-row md:items-start md:justify-between gap-3 md:gap-4">
                        <div className="flex-1">
                            <h1 className="text-2xl md:text-3xl lg:text-4xl font-bold text-foreground mb-2 flex items-center gap-3">
                                <Folder className="w-8 h-8 text-primary" />
                                {project.name}
                            </h1>
                            <p className="text-sm md:text-base text-muted-foreground">
                                Project Dashboard
                            </p>
                        </div>
                        <StartRunModal
                            projects={allProjects}
                            defaultProjectId={paramId || undefined}
                            existingOnly
                            onProjectCreated={() => {
                                fetchAllProjects().then(setAllProjects)
                            }}
                        />
                    </div>
                </div>

                <Tabs defaultValue="overview" className="space-y-6">
                    <TabsList className="bg-white/5 border border-white/10 p-1">
                        <TabsTrigger value="overview" className="data-[state=active]:bg-primary data-[state=active]:text-primary-foreground gap-2">
                            <Activity className="w-4 h-4" />
                            Overview
                        </TabsTrigger>
                        <TabsTrigger value="environmental" className="data-[state=active]:bg-primary data-[state=active]:text-primary-foreground gap-2">
                            <Leaf className="w-4 h-4" />
                            Environmental Settings
                        </TabsTrigger>
                    </TabsList>

                    <TabsContent value="overview" className="space-y-8 animate-in fade-in slide-in-from-bottom-2 duration-500">
                        <div className="space-y-6">
                            {/* Key Metrics — flat 5-column grid */}
                            <div className="border border-border rounded-xl overflow-hidden">
                                <div className="flex items-center gap-6 px-6 pt-5 pb-3 border-b border-border">
                                    <span className="text-xs font-bold text-emerald-500 uppercase tracking-widest flex items-center gap-1.5">
                                        <span className="w-1.5 h-1.5 rounded-full bg-emerald-500 inline-block" />
                                        Impact Overview
                                    </span>
                                    <span className="text-xs font-bold text-green-400 uppercase tracking-widest flex items-center gap-1.5">
                                        <span className="w-1.5 h-1.5 rounded-full bg-green-500 inline-block" />
                                        Execution Stats
                                    </span>
                                </div>
                                <div className="grid grid-cols-2 md:grid-cols-5 gap-px bg-border">
                                    <MetricCard
                                        icon={Leaf}
                                        label="Total Carbon"
                                        value={project.totalCo2G > 1000 ? (project.totalCo2G / 1000).toFixed(4) : project.totalCo2G.toFixed(4)}
                                        unit={project.totalCo2G > 1000 ? 'kg CO₂' : 'g CO₂'}
                                        loading={loading}
                                        className="rounded-none bg-card border-none bg-emerald-500/5"
                                    />
                                    <MetricCard
                                        icon={Coins}
                                        label="Total Cost"
                                        value={project.totalCost.toFixed(4)}
                                        unit={project.currency}
                                        loading={loading}
                                        className="rounded-none bg-card border-none bg-amber-500/5"
                                    />
                                    <MetricCard icon={Zap} label="Total Energy" value={project.totalEnergy.toFixed(4)} unit="Wh" loading={loading} className="rounded-none bg-card border-none" />
                                    <MetricCard icon={Folder} label="Total Runs" value={project.totalRuns.toString()} loading={loading} className="rounded-none bg-card border-none bg-green-500/5" />
                                    <MetricCard icon={Clock} label="Core Hours" value={(totalCoreHours / 24).toFixed(4)} unit="hrs" loading={loading} className="rounded-none bg-card border-none bg-green-500/5" />
                                </div>
                            </div>

                            {/* Energy Breakdown — flat 4-column grid */}
                            <div className="border border-border rounded-xl overflow-hidden">
                                <div className="px-6 pt-5 pb-3 border-b border-border">
                                    <span className="text-xs font-bold text-amber-500 uppercase tracking-widest flex items-center gap-1.5">
                                        <span className="w-1.5 h-1.5 rounded-full bg-amber-500 inline-block" />
                                        Energy Breakdown
                                    </span>
                                </div>
                                <div className="grid grid-cols-2 md:grid-cols-4 gap-px bg-border">
                                    <MetricCard icon={Zap} label="CPU" value={project.totalCpuEnergy.toFixed(4)} unit="Wh" loading={loading} className="rounded-none bg-card border-none bg-amber-500/5" />
                                    <MetricCard icon={Zap} label="GPU" value={project.totalGpuEnergy.toFixed(4)} unit="Wh" loading={loading} className="rounded-none bg-card border-none bg-amber-500/5" />
                                    <MetricCard icon={Zap} label="Memory" value={project.totalMemEnergy.toFixed(4)} unit="Wh" loading={loading} className="rounded-none bg-card border-none bg-amber-500/5" />
                                    <MetricCard icon={Zap} label="iGPU" value={project.totalIgpuEnergy.toFixed(4)} unit="Wh" loading={loading} className="rounded-none bg-card border-none bg-amber-500/5" />
                                </div>
                            </div>

                            {/* Average Performance — flat 3-column grid */}
                            <div className="border border-border rounded-xl overflow-hidden">
                                <div className="px-6 pt-5 pb-3 border-b border-border">
                                    <span className="text-xs font-bold text-purple-400 uppercase tracking-widest flex items-center gap-1.5">
                                        <span className="w-1.5 h-1.5 rounded-full bg-purple-500 inline-block" />
                                        Average Performance
                                    </span>
                                </div>
                                <div className="grid grid-cols-1 sm:grid-cols-3 gap-px bg-border">
                                    <MetricCard icon={Activity} label="Avg CPU" value={project.avgCpuUsage.toFixed(4)} unit="%" loading={loading} className="rounded-none bg-card border-none bg-purple-500/5" />
                                    <MetricCard icon={Activity} label="Avg GPU" value={project.avgGpuUsage.toFixed(4)} unit="%" loading={loading} className="rounded-none bg-card border-none bg-purple-500/5" />
                                    <MetricCard icon={Activity} label="Avg Mem" value={project.avgMemUsage.toFixed(4)} unit="%" loading={loading} className="rounded-none bg-card border-none bg-purple-500/5" />
                                </div>
                            </div>
                        </div>
                            <div className="space-y-12">
                                <section>
                                    <h3 className="text-xl font-bold text-foreground mb-6 flex items-center gap-2">
                                        <Leaf className="w-5 h-5 text-primary" />
                                        Sustainability Impact
                                    </h3>
                                    <EnvironmentalImpact 
                                        totalEnergyWh={project.totalEnergy}
                                        totalCo2G={project.totalCo2G}
                                        totalCost={project.totalCost}
                                        currency={project.currency}
                                    />
                                </section>

                                <section>
                                    <h3 className="text-xl font-bold text-foreground mb-6 flex items-center gap-2">
                                        <Activity className="w-5 h-5 text-primary" />
                                        Project Activity
                                    </h3>
                                    <div className={runsLoading ? 'opacity-50 pointer-events-none transition-opacity' : 'transition-opacity'}>
                                        <RecentRunsTable 
                                            runs={paginatedRuns} 
                                            totalCount={totalRunsCount}
                                            currentPage={page}
                                            pageSize={pageSize}
                                            onPageChange={setPage}
                                        />
                                    </div>
                                </section>
                            </div>
                    </TabsContent>

                    <TabsContent value="environmental" className="animate-in fade-in slide-in-from-bottom-2 duration-500">
                        <CarbonSettings projectId={Number(paramId)} />
                    </TabsContent>
                </Tabs>
            </div>
        </DashboardLayout>
    )
}
