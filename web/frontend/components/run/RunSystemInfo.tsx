'use client'

import { useMemo } from 'react'
import { Run } from '@/lib/mock-data'
import { Clock, User, Activity, Zap } from 'lucide-react'

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

  // Get socket domains for display
  const socketDomains = useMemo(() => {
    if (run.records.length === 0) return new Map<number, string[]>()
    const firstRecord = run.records[0]
    const domainsMap = new Map<number, string[]>()
    
    firstRecord.socket_details?.forEach((s: any) => {
      if (!domainsMap.has(s.socket)) {
        domainsMap.set(s.socket, [])
      }
      if (!domainsMap.get(s.socket)!.includes(s.domain)) {
        domainsMap.get(s.socket)!.push(s.domain)
      }
    })
    
    return domainsMap
  }, [run.records])

  const runStats = [
    { label: 'Collaborator', value: run.collaboratorName ?? 'Unassigned', icon: User },
    { label: 'Duration', value: `${run.records.length}s`, icon: Clock },
    { label: 'Avg CPU', value: `${run.avgCpuUsage.toFixed(1)}%`, icon: Activity },
    { label: 'Avg GPU', value: `${run.avgGpuUsage.toFixed(1)}%`, icon: Activity },
    { label: 'Total Energy', value: `${Number(run.totalEnergy.toFixed(2))} Wh`, icon: Zap },
  ]

  // Helper to pad text to fit within terminal box
  const padLine = (text: string, width: number = 52) => {
    const padding = Math.max(0, width - text.length)
    return text + ' '.repeat(padding)
  }

  return (
    <div className="space-y-6">
      {/* Hardware Section - Terminal Style */}
      {run.systemInfo ? (
        <div className="chart-container overflow-hidden">
          {/* Run Metrics */}
          <h3 className="text-lg font-semibold text-foreground mb-4">Run Metrics</h3>
          <div className="grid grid-cols-2 gap-3 mb-6">
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

          <h3 className="text-lg font-semibold text-foreground mb-4">System Hardware</h3>
          <div className="bg-card/30 rounded-lg p-6 font-mono text-sm border border-primary/20 overflow-x-auto">
            <div className="text-primary leading-relaxed">
              <div>╔════════════════════════════════════════════════════╗</div>
              <div>║              EcoCode System Summary                ║</div>
              <div>╠════════════════════════════════════════════════════╣</div>
              <div className="text-cyan-400">║  CPU: {padLine(run.systemInfo.cpuName.substring(0, 44), 44)}║</div>
              <div className="text-emerald-400">║  Cores / Threads: {padLine(run.systemInfo.cpuCores.toString(), 32)}║</div>
              <div className="text-amber-400">║  Total RAM: {padLine(`${run.systemInfo.ramGb.toFixed(1)} GB`, 38)}║</div>
              <div className="text-blue-400">║  CPU Sockets (RAPL): {padLine(socketCount.toString(), 29)}║</div>
              {Array.from(socketDomains.entries()).map(([socket, domains]) => (
                <div key={socket} className="text-blue-300">║    Socket {socket}: {padLine(domains.join(', '), 37)}║</div>
              ))}
              <div className="text-purple-400">║  NVIDIA GPUs: {padLine(run.systemInfo.gpuCount.toString(), 36)}║</div>
              {run.systemInfo.gpuCount > 0 && run.systemInfo.gpuNames && (
                typeof run.systemInfo.gpuNames === 'string' 
                  ? run.systemInfo.gpuNames.split(',').map((gpu, i) => (
                      <div key={i} className="text-purple-300">║    GPU {i}: {padLine(gpu.trim().substring(0, 40), 40)}║</div>
                    ))
                  : run.systemInfo.gpuNames.map((gpu, i) => (
                      <div key={i} className="text-purple-300">║    GPU {i}: {padLine(gpu.substring(0, 40), 40)}║</div>
                    ))
              )}
              <div>╚════════════════════════════════════════════════════╝</div>
            </div>
          </div>
        </div>
      ) : (
        <div className="chart-container">
          {/* Run Metrics */}
          <h3 className="text-lg font-semibold text-foreground mb-4">Run Metrics</h3>
          <div className="grid grid-cols-2 gap-3 mb-6">
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

          <h3 className="text-lg font-semibold text-foreground mb-4">System Hardware</h3>
          <div className="bg-card/30 rounded-lg p-6 font-mono text-sm border border-primary/20 overflow-x-auto">
            <div className="text-primary">
              <div>╔════════════════════════════════════════════════════╗</div>
              <div>║              Hardware Overview                     ║</div>
              <div>╠════════════════════════════════════════════════════╣</div>
              <div>║  Detected Components:                              ║</div>
              <div>║    {padLine(`${socketCount} Socket${socketCount !== 1 ? 's' : ''}${gpuCount > 0 ? ` + ${gpuCount} GPU${gpuCount !== 1 ? 's' : ''}` : ''}`, 46)}║</div>
              <div>╚════════════════════════════════════════════════════╝</div>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
