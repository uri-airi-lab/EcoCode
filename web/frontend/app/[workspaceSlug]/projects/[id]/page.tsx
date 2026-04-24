'use client'

import { useEffect, useState } from 'react'
import Link from 'next/link'
import { Activity, ArrowLeft, Folder, Zap, Clock } from 'lucide-react'
import { DashboardLayout } from '@/components/DashboardLayout'
import { RecentRunsTable } from '@/components/RecentRunsTable'
import { ProjectDetail, fetchProjectRuns, fetchAllProjects, ProjectSummary } from '@/lib/api-client'
import { useParams } from 'next/navigation'
import { MetricCard } from '@/components/MetricCard'
import { CarbonSettings } from '@/components/CarbonSettings'
import { StartRunModal } from '@/components/StartRunModal'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Leaf } from 'lucide-react'

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
                        <div className="space-y-8">
                            <div className="grid grid-cols-1 sm:grid-cols-2 xl:grid-cols-5 gap-3 md:gap-4">
                                <MetricCard
                                    icon={Zap}
                                    label="Total Energy"
                                    value={Number(project.totalEnergy.toFixed(2)).toLocaleString()}
                                    unit="Wh"
                                    loading={loading}
                                />
                                <MetricCard
                                    icon={Clock}
                                    label="Total Core Hours"
                                    value={(totalCoreHours / 24).toFixed(2)}
                                    unit="hrs"
                                    loading={loading}
                                />
                                <MetricCard
                                    icon={Zap}
                                    label="CPU Energy"
                                    value={Number(project.totalCpuEnergy.toFixed(2)).toLocaleString()}
                                    unit="Wh"
                                    loading={loading}
                                />
                                <MetricCard
                                    icon={Zap}
                                    label="GPU Energy"
                                    value={Number(project.totalGpuEnergy.toFixed(2)).toLocaleString()}
                                    unit="Wh"
                                    loading={loading}
                                />
                                <MetricCard
                                    icon={Zap}
                                    label="Memory Energy"
                                    value={Number(project.totalMemEnergy.toFixed(2)).toLocaleString()}
                                    unit="Wh"
                                    loading={loading}
                                />
                                <MetricCard
                                    icon={Zap}
                                    label="iGPU Energy"
                                    value={Number(project.totalIgpuEnergy.toFixed(2)).toLocaleString()}
                                    unit="Wh"
                                    loading={loading}
                                />
                                <MetricCard
                                    icon={Folder}
                                    label="Total Runs"
                                    value={project.totalRuns.toString()}
                                    unit=""
                                    loading={loading}
                                />
                                <MetricCard
                                    icon={Activity}
                                    label="Avg CPU Usage"
                                    value={project.avgCpuUsage.toFixed(1)}
                                    unit="%"
                                    loading={loading}
                                />
                                <MetricCard
                                    icon={Activity}
                                    label="Avg GPU Usage"
                                    value={project.avgGpuUsage.toFixed(1)}
                                    unit="%"
                                    loading={loading}
                                />
                                <MetricCard
                                    icon={Activity}
                                    label="Avg Memory Usage"
                                    value={project.avgMemUsage.toFixed(1)}
                                    unit="%"
                                    loading={loading}
                                />
                            </div>

                            <RecentRunsTable runs={project.runs} />
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
