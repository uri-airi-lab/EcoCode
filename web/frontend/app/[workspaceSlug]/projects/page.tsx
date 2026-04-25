'use client'

import { useEffect, useState } from 'react'
import { DashboardLayout } from '@/components/layout/DashboardLayout'
import { ProjectsTable } from '@/components/tables/ProjectsTable'
import { ProjectSummary, fetchAllProjects } from '@/lib/api'
import { Folder } from 'lucide-react'
import { StartRunModal } from '@/components/StartRunModal'
import { useWorkspace } from '@/components/layout/WorkspaceProvider'

export default function ProjectsPage() {
    const { currentWorkspace, loading: workspaceLoading } = useWorkspace()
    const [projects, setProjects] = useState<ProjectSummary[]>([])
    const [loading, setLoading] = useState(true)

    useEffect(() => {
        if (workspaceLoading) return
        let cancelled = false

        const loadProjects = async () => {
            setLoading(true)
            try {
                const data = await fetchAllProjects()
                if (!cancelled) {
                    const filtered = currentWorkspace
                        ? data.filter(p => p.workspaceSlug === currentWorkspace.slug)
                        : data
                    setProjects(filtered)
                }
            } catch (error) {
                if (!cancelled) {
                    console.error('Failed to load projects:', error)
                    setProjects([])
                }
            } finally {
                if (!cancelled) {
                    setLoading(false)
                }
            }
        }
        loadProjects()

        return () => {
            cancelled = true
        }
    }, [currentWorkspace, workspaceLoading])

    return (
        <DashboardLayout>
            <div className="p-4 md:p-6 lg:p-8 w-full">
                <div className="mb-6 md:mb-8">
                    <div className="flex flex-col md:flex-row md:items-center justify-between gap-4">
                        <div>
                            <h1 className="text-2xl md:text-3xl lg:text-4xl font-bold text-foreground mb-2 flex items-center gap-3">
                                <Folder className="w-8 h-8 text-primary" />
                                Projects
                            </h1>
                            <p className="text-sm md:text-base text-muted-foreground">
                                All registered EcoCode projects
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

                <ProjectsTable 
                    projects={projects} 
                    loading={loading}
                    onProjectDeleted={() => {
                        fetchAllProjects().then(data => {
                            const filtered = currentWorkspace
                                ? data.filter(p => p.workspaceSlug === currentWorkspace.slug)
                                : data
                            setProjects(filtered)
                        })
                    }}
                />
            </div>
        </DashboardLayout>
    )
}
