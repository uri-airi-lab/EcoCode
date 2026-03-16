'use client'

import { useMemo } from 'react'
import { Run } from '@/lib/mock-data'

interface RunSystemInfoProps {
  run: Run
}

export function RunSystemInfo({ run }: RunSystemInfoProps) {
  const gpuCount = useMemo(() => {
    if (run.records.length === 0) return 0
    const firstRecord = run.records[0]
    return firstRecord.gpu_details?.length ?? 0
  }, [run.records])

  const socketCount = useMemo(() => {
    if (run.records.length === 0) return 0
    const firstRecord = run.records[0]
    const sockets = new Set(firstRecord.socket_details?.map(s => s.socket))
    return sockets.size
  }, [run.records])

  const specs = [
    {
      label: 'Collaborator',
      value: run.collaboratorName ?? 'Unassigned',
    },
    {
      label: 'Status',
      value: run.status.charAt(0).toUpperCase() + run.status.slice(1),
    },
    {
      label: 'Duration',
      value: `${run.records.length}s`,
    },
    {
      label: 'Hardware',
      value: `${socketCount} Socket${socketCount !== 1 ? 's' : ''}${gpuCount > 0 ? ` + ${gpuCount} GPU${gpuCount !== 1 ? 's' : ''}` : ''}`,
    },
    {
      label: 'Avg CPU Usage',
      value: `${run.avgCpuUsage.toFixed(1)}%`,
    },
    {
      label: 'Avg GPU Usage',
      value: `${run.avgGpuUsage.toFixed(1)}%`,
    },
    {
      label: 'Total Energy',
      value: `${Number(run.totalEnergy.toFixed(2))} Wh`,
    },
  ]

  return (
    <div className="chart-container">
      <h3 className="text-lg font-semibold text-foreground mb-4">Run Details</h3>
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
        {specs.map((spec) => (
          <div
            key={spec.label}
            className="p-4 rounded-lg bg-white/5 border border-white/10"
          >
            <p className="text-xs text-muted-foreground mb-1">{spec.label}</p>
            <p className="text-lg font-semibold text-foreground">
              {spec.value}
            </p>
          </div>
        ))}
      </div>
    </div>
  )
}
