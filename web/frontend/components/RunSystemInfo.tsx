'use client'

import { useMemo } from 'react'
import { Run } from '@/lib/mock-data'
import { Cpu, Database, Layers, Monitor, Clock, User, Activity, Zap, Server } from 'lucide-react'

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
    const sockets = new Set(firstRecord.socket_details?.map((s: any) => s.socket))
    return sockets.size
  }, [run.records])

  const runStats = [
    { label: 'Collaborator', value: run.collaboratorName ?? 'Unassigned', icon: User },
    { label: 'Duration', value: `${run.records.length}s`, icon: Clock },
    { label: 'Avg CPU', value: `${run.avgCpuUsage.toFixed(1)}%`, icon: Activity },
    { label: 'Avg GPU', value: `${run.avgGpuUsage.toFixed(1)}%`, icon: Activity },
    { label: 'Total Energy', value: `${Number(run.totalEnergy.toFixed(2))} Wh`, icon: Zap },
  ]

  return (
    <div className="space-y-6">
      {/* Hardware Section - Only show the cool version if we have detailed systemInfo */}
      {run.systemInfo ? (
        <div className="chart-container overflow-hidden relative">
          <div className="absolute top-0 right-0 p-32 bg-primary/5 rounded-full blur-3xl -mr-16 -mt-16 pointer-events-none" />
          <h3 className="text-lg font-semibold text-foreground mb-6 flex items-center gap-2">
            <Server className="w-5 h-5 text-primary" />
            System Hardware
          </h3>
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4 relative z-10">
            {/* CPU Card */}
            <div className="flex flex-col p-4 rounded-xl border border-border/50 bg-gradient-to-br from-background to-muted/20 shadow-sm hover:shadow-md transition-all duration-300 group">
              <div className="flex items-center gap-3 mb-3">
                <div className="p-2.5 rounded-lg bg-blue-500/10 text-blue-500 group-hover:bg-blue-500 group-hover:text-white transition-colors duration-300">
                  <Cpu className="w-5 h-5" />
                </div>
                <span className="text-sm font-medium text-muted-foreground">Processor</span>
              </div>
              <p className="text-sm font-semibold text-foreground line-clamp-2" title={run.systemInfo.cpuName}>
                {run.systemInfo.cpuName}
              </p>
            </div>

            {/* Cores Card */}
            <div className="flex flex-col p-4 rounded-xl border border-border/50 bg-gradient-to-br from-background to-muted/20 shadow-sm hover:shadow-md transition-all duration-300 group">
              <div className="flex items-center gap-3 mb-3">
                <div className="p-2.5 rounded-lg bg-emerald-500/10 text-emerald-500 group-hover:bg-emerald-500 group-hover:text-white transition-colors duration-300">
                  <Layers className="w-5 h-5" />
                </div>
                <span className="text-sm font-medium text-muted-foreground">Cores / Threads</span>
              </div>
              <p className="text-xl font-bold text-foreground">
                {run.systemInfo.cpuCores}
              </p>
            </div>

            {/* RAM Card */}
            <div className="flex flex-col p-4 rounded-xl border border-border/50 bg-gradient-to-br from-background to-muted/20 shadow-sm hover:shadow-md transition-all duration-300 group">
              <div className="flex items-center gap-3 mb-3">
                <div className="p-2.5 rounded-lg bg-amber-500/10 text-amber-500 group-hover:bg-amber-500 group-hover:text-white transition-colors duration-300">
                  <Database className="w-5 h-5" />
                </div>
                <span className="text-sm font-medium text-muted-foreground">Total Memory</span>
              </div>
              <p className="text-xl font-bold text-foreground">
                {run.systemInfo.ramGb.toFixed(1)} <span className="text-sm font-normal text-muted-foreground">GB</span>
              </p>
            </div>

            {/* GPU Card */}
            <div className="flex flex-col p-4 rounded-xl border border-border/50 bg-gradient-to-br from-background to-muted/20 shadow-sm hover:shadow-md transition-all duration-300 group">
              <div className="flex items-center gap-3 mb-3">
                <div className="p-2.5 rounded-lg bg-purple-500/10 text-purple-500 group-hover:bg-purple-500 group-hover:text-white transition-colors duration-300">
                  <Monitor className="w-5 h-5" />
                </div>
                <span className="text-sm font-medium text-muted-foreground">
                  GPUs ({run.systemInfo.gpuCount})
                </span>
              </div>
              <p className="text-sm font-semibold text-foreground line-clamp-2" title={run.systemInfo.gpuNames}>
                {run.systemInfo.gpuCount > 0 ? run.systemInfo.gpuNames : 'No dedicated GPU'}
              </p>
            </div>
          </div>
        </div>
      ) : (
        <div className="chart-container">
          <h3 className="text-lg font-semibold text-foreground mb-4">Hardware Overview</h3>
          <div className="p-4 rounded border border-border bg-muted/30 flex items-center gap-4">
            <Server className="w-8 h-8 text-muted-foreground" />
            <div>
              <p className="text-sm text-muted-foreground">Detected Components</p>
              <p className="text-lg font-semibold text-foreground">
                {socketCount} Socket{socketCount !== 1 ? 's' : ''}
                {gpuCount > 0 ? ` + ${gpuCount} GPU${gpuCount !== 1 ? 's' : ''}` : ''}
              </p>
            </div>
          </div>
        </div>
      )}

      {/* General Run Stats */}
      <div className="chart-container">
        <h3 className="text-lg font-semibold text-foreground mb-4">Run Metrics</h3>
        <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-5 gap-4">
          {runStats.map((stat, idx) => (
            <div key={idx} className="p-4 rounded-xl border border-border/40 bg-card/50 flex flex-col items-center justify-center text-center gap-2">
              <stat.icon className="w-5 h-5 text-muted-foreground mb-1" />
              <div>
                <p className="text-xs text-muted-foreground uppercase tracking-wider">{stat.label}</p>
                <p className="text-lg font-bold text-foreground mt-0.5">{stat.value}</p>
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  )
}
