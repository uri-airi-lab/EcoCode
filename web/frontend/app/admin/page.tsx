'use client'

import Link from 'next/link'
import { useEffect, useMemo, useState } from 'react'
import { Activity, ArrowUpRight, Folder, History, PlayCircle, Users, Zap } from 'lucide-react'

import { DashboardLayout } from '@/components/layout/DashboardLayout'
import { MetricCard } from '@/components/MetricCard'
import { AdminProjectMonitor, ProjectSummary, fetchAdminProjects, fetchAllProjects } from '@/lib/api'

const POLL_INTERVAL_MS = 5000

function formatDate(timestamp: number | null): string {
  return timestamp ? new Date(timestamp).toLocaleString() : 'No activity'
}

function statusTone(status: 'running' | 'finished' | 'failed') {
  switch (status) {
    case 'running':
      return 'bg-blue-500/20 text-blue-300 border-blue-500/30'
    case 'failed':
      return 'bg-red-500/20 text-red-300 border-red-500/30'
    default:
      return 'bg-emerald-500/20 text-emerald-300 border-emerald-500/30'
  }
}

export default function AdminPage() {
  const [projects, setProjects] = useState<AdminProjectMonitor[]>([])
  const [projectSummaries, setProjectSummaries] = useState<ProjectSummary[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    let cancelled = false

    const loadProjects = async () => {
      setLoading(true)
      try {
        const [adminResult, summaryResult] = await Promise.allSettled([
          fetchAdminProjects(),
          fetchAllProjects(),
        ])

        if (!cancelled) {
          if (adminResult.status === 'fulfilled') setProjects(adminResult.value)
          if (summaryResult.status === 'fulfilled') setProjectSummaries(summaryResult.value)
          const failed = adminResult.status === 'rejected' || summaryResult.status === 'rejected'
          setError(failed ? 'Failed to load some data. Is the backend running?' : null)
        }
      } catch (fetchError) {
        if (!cancelled) {
          setError('Failed to load admin monitor data.')
        }
      } finally {
        if (!cancelled) {
          setLoading(false)
        }
      }
    }

    loadProjects()
    const intervalId = window.setInterval(loadProjects, POLL_INTERVAL_MS)

    return () => {
      cancelled = true
      window.clearInterval(intervalId)
    }
  }, [])

  const stats = useMemo(() => {
    const activeProjects = projects.filter((project) => project.activeRuns.length > 0).length
    const activeRuns = projects.reduce((sum, project) => sum + project.activeRuns.length, 0)
    const totalRuns = projectSummaries.reduce((sum, project) => sum + project.totalRuns, 0)
    const totalEnergy = projectSummaries.reduce((sum, project) => sum + project.totalEnergy, 0)

    return {
      totalProjects: projectSummaries.length || projects.length,
      totalRuns,
      totalEnergy,
      activeProjects,
      activeRuns,
    }
  }, [projectSummaries, projects])

  const sortedProjects = useMemo(
    () =>
      [...projects].sort((left, right) => {
        const leftActive = left.activeRuns.length > 0 ? 1 : 0
        const rightActive = right.activeRuns.length > 0 ? 1 : 0

        return (
          rightActive - leftActive ||
          right.activeRuns.length - left.activeRuns.length ||
          left.name.localeCompare(right.name)
        )
      }),
    [projects]
  )

  return (
    <DashboardLayout>
      <div className="p-4 md:p-6 lg:p-8 w-full">
        <div className="mb-6 md:mb-8">
          <h1 className="text-2xl md:text-3xl lg:text-4xl font-bold text-foreground mb-2">
            Admin Monitor
          </h1>
          <p className="text-sm md:text-base text-muted-foreground">
            Watch all projects, saved collaborators, and current activity in one place.
          </p>
        </div>

        <div className="mb-6 flex flex-wrap gap-3">
          <Link
            href="/projects"
            className="inline-flex items-center gap-2 rounded border border-border bg-card px-4 py-2 text-sm font-medium text-foreground transition-colors hover:border-muted-foreground/30"
          >
            <Folder className="w-4 h-4 text-primary" />
            Browse Projects
            <ArrowUpRight className="w-4 h-4 text-muted-foreground" />
          </Link>
          <Link
            href="/runs"
            className="inline-flex items-center gap-2 rounded border border-border bg-card px-4 py-2 text-sm font-medium text-foreground transition-colors hover:border-muted-foreground/30"
          >
            <History className="w-4 h-4 text-primary" />
            Browse Runs
            <ArrowUpRight className="w-4 h-4 text-muted-foreground" />
          </Link>
        </div>

        <div className="mb-8 grid grid-cols-1 sm:grid-cols-2 xl:grid-cols-5 gap-3 md:gap-4">
          <MetricCard
            icon={Zap}
            label="Total Energy"
            value={Number(stats.totalEnergy.toFixed(2)).toLocaleString()}
            unit="Wh"
            loading={loading}
          />
          <MetricCard
            icon={History}
            label="Total Runs"
            value={stats.totalRuns.toString()}
            loading={loading}
          />
          <MetricCard
            icon={Folder}
            label="Total Projects"
            value={stats.totalProjects.toString()}
            loading={loading}
          />
          <MetricCard
            icon={Activity}
            label="Active Projects"
            value={stats.activeProjects.toString()}
            loading={loading}
          />
          <MetricCard
            icon={PlayCircle}
            label="Active Runs"
            value={stats.activeRuns.toString()}
            loading={loading}
          />
        </div>

        {error ? (
          <div className="chart-container border border-red-500/20 text-red-300">
            {error}
          </div>
        ) : null}

        <div className="space-y-4">
          {loading ? (
            <div className="chart-container h-48 bg-card border border-border animate-pulse" />
          ) : sortedProjects.length === 0 ? (
            <div className="chart-container text-muted-foreground">No projects found.</div>
          ) : (
            sortedProjects.map((project) => {
              const collaborators = project.collaborators ?? []
              const collaboratorSelectionLabel =
                collaborators.length === 0
                  ? 'No collaborators saved yet'
                  : collaborators.length === 1
                    ? `Auto-selects: ${project.defaultCollaborator ?? collaborators[0]}`
                    : 'Multiple collaborators: choose one with --collaborator'
              const projectState = project.activeRuns.length > 0
                ? 'Active'
                : project.latestRun
                  ? 'Idle'
                  : 'No runs yet'

              return (
                <section key={project.id} className="chart-container space-y-4">
                  <div className="flex flex-col md:flex-row md:items-start md:justify-between gap-3">
                    <div>
                      <div className="flex items-center gap-3 mb-2">
                        <Link
                          href={`/projects/${project.id}`}
                          className="inline-flex items-center gap-2 text-lg md:text-xl font-semibold text-foreground transition-colors hover:text-primary"
                        >
                          {project.name}
                          <ArrowUpRight className="w-4 h-4 text-muted-foreground" />
                        </Link>
                        <span
                          className={`inline-flex items-center rounded-full border px-3 py-1 text-xs font-medium ${
                            project.activeRuns.length > 0
                              ? 'bg-blue-500/20 text-blue-300 border-blue-500/30'
                              : 'bg-white/5 text-muted-foreground border-white/10 backdrop-blur-sm'
                          }`}
                        >
                          {projectState}
                        </span>
                      </div>
                      <p className="text-sm text-muted-foreground">
                        {collaboratorSelectionLabel}
                      </p>
                    </div>
                    <div className="text-sm text-muted-foreground flex items-center gap-2">
                      <Users className="w-4 h-4" />
                      {project.activeRuns.length} active collaborator
                      {project.activeRuns.length === 1 ? '' : 's'}
                    </div>
                  </div>

                  <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
                    <div className="rounded border border-white/10 bg-white/5 backdrop-blur-sm p-4">
                      <p className="text-xs uppercase tracking-[0.18em] text-muted-foreground mb-3">
                        Active Collaborators
                      </p>
                      {project.activeRuns.length === 0 ? (
                        <p className="text-sm text-muted-foreground">Idle</p>
                      ) : (
                        <div className="flex flex-wrap gap-2">
                          {project.activeRuns.map((run) => (
                            <Link
                              key={run.runId}
                              href={`/runs/${run.runId}`}
                              className={`inline-flex items-center gap-2 rounded-full border px-3 py-1 text-xs font-medium ${statusTone(run.status)}`}
                            >
                              <span>{run.collaboratorName ?? 'Unassigned'}</span>
                              <span className="text-[10px] opacity-80">{run.runName}</span>
                            </Link>
                          ))}
                        </div>
                      )}
                    </div>

                    <div className="rounded border border-white/10 bg-white/5 backdrop-blur-sm p-4">
                      <p className="text-xs uppercase tracking-[0.18em] text-muted-foreground mb-3">
                        Project Collaborators
                      </p>
                      {collaborators.length === 0 ? (
                        <p className="text-sm text-muted-foreground">Unassigned</p>
                      ) : (
                        <div className="flex flex-wrap gap-2">
                          {collaborators.map((collaborator) => (
                            <span
                              key={collaborator}
                              className="inline-flex items-center rounded-full border border-white/10 bg-white/5 px-3 py-1 text-xs font-medium text-foreground backdrop-blur-sm"
                            >
                              {collaborator}
                            </span>
                          ))}
                        </div>
                      )}
                    </div>

                    <div className="rounded border border-white/10 bg-white/5 backdrop-blur-sm p-4 lg:col-span-2">
                      <p className="text-xs uppercase tracking-[0.18em] text-muted-foreground mb-3">
                        Latest Run
                      </p>
                      {project.latestRun ? (
                        <div className="space-y-2">
                          <div className="flex items-center gap-2">
                            <Link
                              href={`/runs/${project.latestRun.runId}`}
                              className="inline-flex items-center gap-2 font-medium text-foreground transition-colors hover:text-primary"
                            >
                              {project.latestRun.runName}
                              <ArrowUpRight className="w-4 h-4 text-muted-foreground" />
                            </Link>
                            <span
                              className={`inline-flex items-center rounded-full border px-2 py-1 text-[11px] font-medium ${statusTone(project.latestRun.status)}`}
                            >
                              {project.latestRun.status}
                            </span>
                          </div>
                          <p className="text-sm text-muted-foreground">
                            Run by: {project.latestRun.collaboratorName ?? 'Unassigned'}
                          </p>
                          <p className="text-sm text-muted-foreground">
                            Started: {formatDate(project.latestRun.startedAt)}
                          </p>
                          <p className="text-sm text-muted-foreground">
                            Last activity: {formatDate(project.latestRun.lastRecordAt)}
                          </p>
                        </div>
                      ) : (
                        <p className="text-sm text-muted-foreground">No runs yet</p>
                      )}
                    </div>
                  </div>
                </section>
              )
            })
          )}
        </div>
      </div>
    </DashboardLayout>
  )
}
