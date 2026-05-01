'use client'

import { memo } from 'react'
import Link from 'next/link'
import { useRouter } from 'next/navigation'
import { Run } from '@/lib/mock-data'
import { format } from 'date-fns'
import { ChevronRight } from 'lucide-react'
import { useWorkspace } from '@/components/layout/WorkspaceProvider'

interface RecentRunsTableProps {
  runs: Omit<Run, 'records'>[]
  totalCount?: number
  currentPage?: number
  pageSize?: number
  onPageChange?: (page: number) => void
}

export const RecentRunsTable = memo(function RecentRunsTable({ 
  runs, 
  totalCount = runs.length, 
  currentPage = 1, 
  pageSize = 10, 
  onPageChange 
}: RecentRunsTableProps) {
  const { currentWorkspace } = useWorkspace()
  const router = useRouter()
  const workspaceSlug = currentWorkspace?.slug ?? ''

  const handleRowClick = (runId: string) => {
    router.push(`/${workspaceSlug}/runs/${runId}`)
  }

  const totalPages = Math.ceil(totalCount / pageSize)

  return (
    <div className="chart-container">
      <div className="flex items-center justify-between mb-4">
        <h3 className="text-lg font-semibold text-foreground">Project Runs</h3>
        <div className="flex items-center gap-2 text-sm text-muted-foreground">
          Showing {runs.length} of {totalCount} runs
        </div>
      </div>

      <div className="overflow-x-auto">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-border">
              <th className="text-left py-3 px-4 text-muted-foreground font-medium uppercase tracking-widest text-[11px] w-1/4">
                Run Name
              </th>
              <th className="text-left py-3 px-4 text-muted-foreground font-medium uppercase tracking-widest text-[11px] w-1/6">
                Run Time-Stamp
              </th>
              <th className="text-left py-3 px-4 text-muted-foreground font-medium uppercase tracking-widest text-[11px] w-1/6">
                Run Owner
              </th>
              <th className="text-right py-3 px-4 text-muted-foreground font-medium uppercase tracking-widest text-[11px] w-1/8">
                Energy (Wh)
              </th>
              <th className="text-right py-3 px-4 text-muted-foreground font-medium uppercase tracking-widest text-[11px] w-1/8">
                CO₂ (g)
              </th>
              <th className="text-left py-3 px-4 text-muted-foreground font-medium uppercase tracking-widest text-[11px]">
                Exporter
              </th>
              <th className="text-center py-3 px-4 text-muted-foreground font-medium uppercase tracking-widest text-[11px] w-1/8">
                Status
              </th>
              <th className="text-right py-3 px-4 w-8" />
            </tr>
          </thead>
          <tbody>
            {runs.map((run) => (
              <tr
                key={run.id}
                className="border-b border-border hover:bg-muted/30 transition-colors cursor-pointer"
                onClick={() => handleRowClick(run.id)}
              >
                <td className="py-3 px-4">
                  <div>
                    <p className="font-medium text-foreground truncate">{run.name}</p>
                  </div>
                </td>
                <td className='py-3 px-4'>
                  <p className="text-sm text-muted-foreground">
                    {format(new Date(run.timestamp), 'MMM d, HH:mm')}
                  </p> 
                </td>
                <td className='py-3 px-4'>
                  <p className="text-sm text-muted-foreground truncate">
                    {run.collaboratorName ?? 'Unassigned'}
                  </p>
                </td>
                <td className="text-right py-3 px-4 text-foreground font-medium font-mono">
                  {Number(run.totalEnergy.toFixed(2)).toLocaleString()}
                </td>
                <td className="text-right py-3 px-4 text-foreground font-medium font-mono">
                  {Number(run.carbonFootprint.toFixed(2)).toLocaleString()}
                </td>
                <td className="py-3 px-4">
                  {run.exporterType && (
                    <span className="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-secondary text-secondary-foreground font-mono">
                      {run.exporterType}
                    </span>
                  )}
                </td>
                <td className="text-center py-3 px-4">
                  <span
                    className={`inline-block px-3 py-1 rounded text-xs font-medium ${run.status === 'finished'
                      ? 'bg-primary/15 text-primary'
                      : run.status === 'running'
                        ? 'bg-chart-1/20 text-chart-1'
                        : run.status === 'canceled'
                          ? 'bg-amber-500/15 text-amber-500'
                          : 'bg-destructive/15 text-destructive'
                      }`}
                  >
                    {run.status}
                  </span>
                </td>
                <td className="text-right py-3 px-4">
                  <ChevronRight className="w-4 h-4 text-muted-foreground" />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {totalPages > 1 && (
        <div className="flex items-center justify-center gap-4 mt-6">
          <button
            onClick={() => onPageChange?.(currentPage - 1)}
            disabled={currentPage === 1}
            className="px-4 py-2 text-sm font-medium rounded-lg border border-border hover:bg-muted disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          >
            Previous
          </button>
          <div className="flex items-center gap-2">
            {[...Array(totalPages)].map((_, i) => (
              <button
                key={i + 1}
                onClick={() => onPageChange?.(i + 1)}
                className={`w-8 h-8 rounded-lg text-sm font-medium transition-colors ${
                  currentPage === i + 1
                    ? 'bg-primary text-primary-foreground'
                    : 'hover:bg-muted border border-border'
                }`}
              >
                {i + 1}
              </button>
            ))}
          </div>
          <button
            onClick={() => onPageChange?.(currentPage + 1)}
            disabled={currentPage === totalPages}
            className="px-4 py-2 text-sm font-medium rounded-lg border border-border hover:bg-muted disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          >
            Next
          </button>
        </div>
      )}
    </div>
  )
})
