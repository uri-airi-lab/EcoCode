'use client'

import { useEffect, useState, useMemo } from 'react'
import { Search, ChevronLeft, ChevronRight } from 'lucide-react'
import { DashboardLayout } from '@/components/layout/DashboardLayout'
import { RunsTable } from '@/components/tables/RunsTable'
import { Run } from '@/lib/mock-data'
import { fetchAllRuns, fetchAllProjects, RunSummary } from '@/lib/api'
import { useWorkspace } from '@/components/layout/WorkspaceProvider'

const PAGE_SIZE = 20

export default function RunsPage() {
  const { currentWorkspace, loading: workspaceLoading } = useWorkspace()
  const [runs, setRuns] = useState<RunSummary[]>([])
  const [loading, setLoading] = useState(true)
  const [searchQuery, setSearchQuery] = useState('')
  const [page, setPage] = useState(0)

  useEffect(() => {
    if (workspaceLoading) return
    let cancelled = false

    const fetchData = async () => {
      setLoading(true)
      try {
        // Use Promise.allSettled to handle partial failures gracefully
        const [projectsResult, runsResult] = await Promise.allSettled([
          fetchAllProjects(),
          fetchAllRuns(),
        ])

        if (!cancelled) {
          // Handle projects
          const allProjects = projectsResult.status === 'fulfilled' ? projectsResult.value : []
          const workspaceProjects = currentWorkspace
            ? allProjects.filter(p => p.workspaceSlug === currentWorkspace.slug)
            : allProjects

          // Handle runs
          const allRuns = runsResult.status === 'fulfilled' ? runsResult.value : []
          const workspaceRunList = (currentWorkspace
            ? allRuns.filter(r => workspaceProjects.some(p => p.id === r.projectId))
            : allRuns).map(run => ({
              ...run,
              projectName: run.projectName || allProjects.find(p => p.id === run.projectId)?.name || null
            }))

          setRuns(workspaceRunList)
          setPage(0)

          // Log any errors for debugging
          if (projectsResult.status === 'rejected') {
            console.warn('Failed to fetch projects:', projectsResult.reason)
          }
          if (runsResult.status === 'rejected') {
            console.warn('Failed to fetch runs:', runsResult.reason)
          }
        }
      } catch (error) {
        if (!cancelled) {
          console.error('Failed to fetch data:', error)
          setRuns([])
        }
      } finally {
        if (!cancelled) setLoading(false)
      }
    }

    fetchData()
    return () => { cancelled = true }
  }, [currentWorkspace, workspaceLoading])

  const filteredRuns = useMemo(() => {
    if (!searchQuery.trim()) return runs

    const lowerQuery = searchQuery.toLowerCase()
    return runs.filter((run) => {
      if (run.name.toLowerCase().includes(lowerQuery)) return true
      if (run.projectName?.toLowerCase().includes(lowerQuery)) return true
      if (run.collaboratorName?.toLowerCase().includes(lowerQuery)) return true

      if (searchQuery.includes(':')) {
        const [key, value] = searchQuery.split(':')
        if (key.trim() === 'status' && value) return run.status === value.trim()
      }

      if (searchQuery.includes('<') || searchQuery.includes('>')) {
        const match = searchQuery.match(/energy\s*([<>])\s*(\d+)/)
        if (match) {
          const [, operator, value] = match
          const threshold = parseFloat(value)
          return operator === '<' ? run.totalEnergy < threshold : run.totalEnergy > threshold
        }
      }

      return false
    })
  }, [runs, searchQuery])

  // Reset to first page whenever filter changes
  useEffect(() => { setPage(0) }, [searchQuery])

  const totalPages = Math.ceil(filteredRuns.length / PAGE_SIZE)
  const pagedRuns = filteredRuns.slice(page * PAGE_SIZE, (page + 1) * PAGE_SIZE)

  const reloadRuns = async () => {
    const [allProjects, allRuns] = await Promise.all([fetchAllProjects(), fetchAllRuns()])
    const workspaceProjects = currentWorkspace
      ? allProjects.filter(p => p.workspaceSlug === currentWorkspace.slug)
      : allProjects
    const workspaceRunList = (currentWorkspace
      ? allRuns.filter(r => workspaceProjects.some(p => p.id === r.projectId))
      : allRuns).map(run => ({
        ...run,
        projectName: run.projectName || allProjects.find(p => p.id === run.projectId)?.name || null
      }))
    setRuns(workspaceRunList)
  }

  return (
    <DashboardLayout>
      <div className="p-4 md:p-6 lg:p-8 w-full">
        {/* Header */}
        <div className="mb-6 md:mb-8">
          <h1 className="text-2xl md:text-3xl lg:text-4xl font-bold text-foreground mb-2">
            Run History
          </h1>
          <p className="text-sm md:text-base text-muted-foreground">
            View and manage all your EcoCode runs
          </p>
        </div>

        {/* Search Bar */}
        <div className="mb-6">
          <div className="relative">
            <Search className="absolute left-4 top-1/2 transform -translate-y-1/2 w-5 h-5 text-muted-foreground" />
            <input
              type="text"
              placeholder="Search runs... (e.g., 'Training Run', 'status:finished', 'status:canceled', 'energy < 100')"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="w-full pl-12 pr-4 py-2 bg-card border border-border rounded text-foreground placeholder-muted-foreground focus:outline-none focus:border-primary transition-all duration-200"
            />
          </div>
        </div>

        {/* Results info */}
        <div className="mb-4 text-sm text-muted-foreground">
          Showing {filteredRuns.length === 0 ? 0 : page * PAGE_SIZE + 1}–{Math.min((page + 1) * PAGE_SIZE, filteredRuns.length)} of {filteredRuns.length} runs
        </div>

        {/* Runs Table */}
        {loading ? (
          <div className="chart-container flex items-center justify-center py-12">
            <div className="flex flex-col items-center gap-3">
              <div className="w-8 h-8 border-4 border-primary border-t-transparent rounded-full animate-spin" />
              <p className="text-sm text-muted-foreground">Loading runs...</p>
            </div>
          </div>
        ) : pagedRuns.length > 0 ? (
          <>
            <RunsTable runs={pagedRuns} onRunDeleted={reloadRuns} />

            {/* Pagination controls */}
            {totalPages > 1 && (
              <div className="flex items-center justify-between mt-4">
                <button
                  onClick={() => setPage(p => Math.max(0, p - 1))}
                  disabled={page === 0}
                  className="flex items-center gap-1 px-3 py-2 rounded border border-border text-sm text-foreground disabled:opacity-40 disabled:cursor-not-allowed hover:bg-accent transition-colors"
                >
                  <ChevronLeft className="w-4 h-4" />
                  Previous
                </button>
                <span className="text-sm text-muted-foreground">
                  Page {page + 1} of {totalPages}
                </span>
                <button
                  onClick={() => setPage(p => Math.min(totalPages - 1, p + 1))}
                  disabled={page >= totalPages - 1}
                  className="flex items-center gap-1 px-3 py-2 rounded border border-border text-sm text-foreground disabled:opacity-40 disabled:cursor-not-allowed hover:bg-accent transition-colors"
                >
                  Next
                  <ChevronRight className="w-4 h-4" />
                </button>
              </div>
            )}
          </>
        ) : (
          <div className="chart-container text-center py-12">
            <p className="text-muted-foreground">
              {searchQuery ? 'No runs match your search criteria' : 'No runs found'}
            </p>
          </div>
        )}
      </div>
    </DashboardLayout>
  )
}
