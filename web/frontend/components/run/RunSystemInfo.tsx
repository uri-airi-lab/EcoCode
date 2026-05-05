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
      <div className="chart-container overflow-hidden">
        {/* Run Metrics */}
        <h4 className="text-[10px] font-bold text-primary uppercase tracking-[0.2em] mb-6 flex items-center gap-2">
          <Activity className="w-3 h-3" />
          Execution Summary
        </h4>
        <div className="grid grid-cols-2 md:grid-cols-5 gap-3 mb-8">
          {runStats.map((stat, idx) => (
            <div key={idx} className="p-4 rounded-lg border border-border/40 bg-primary/[0.02] flex flex-col items-center justify-center text-center group hover:bg-primary/[0.05] transition-all duration-300">
              <stat.icon className="w-4 h-4 text-muted-foreground/60 mb-2 group-hover:text-primary transition-colors" />
              <p className="text-[9px] font-bold text-muted-foreground/40 uppercase tracking-tighter mb-1">{stat.label}</p>
              <p className="text-sm font-mono font-bold text-foreground">{stat.value}</p>
            </div>
          ))}
        </div>

        <h4 className="text-[10px] font-bold text-muted-foreground uppercase tracking-[0.2em] mb-6 flex items-center gap-2">
          <Zap className="w-3 h-3" />
          Hardware Specification
        </h4>
        
        {run.systemInfo ? (
          <div className="relative rounded-lg overflow-hidden border border-border/60 glass-strong">
            {/* Terminal Header */}
            <div className="bg-muted/40 px-4 py-2 flex items-center justify-between border-b border-border/40">
              <div className="flex gap-1.5">
                <div className="w-2.5 h-2.5 rounded-full bg-rose-500/40" />
                <div className="w-2.5 h-2.5 rounded-full bg-amber-500/40" />
                <div className="w-2.5 h-2.5 rounded-full bg-emerald-500/40" />
              </div>
              <span className="text-[10px] font-mono text-muted-foreground/60">ecocode-sys-summary --verbose</span>
            </div>
            
            {/* Terminal Content */}
            <div className="p-6 font-mono text-[13px] leading-relaxed bg-black/20">
              <div className="flex items-start gap-4 mb-3 pb-3 border-b border-white/5">
                <span className="text-primary font-bold">CPU:</span>
                <span className="text-foreground/90">{run.systemInfo.cpuName}</span>
              </div>
              
              <div className="grid grid-cols-1 md:grid-cols-2 gap-x-8 gap-y-2">
                <div className="flex justify-between border-b border-white/5 pb-1">
                  <span className="text-muted-foreground/60">Cores / Threads:</span>
                  <span className="text-foreground">{run.systemInfo.cpuCores} / {run.systemInfo.cpuCores * 2}</span>
                </div>
                <div className="flex justify-between border-b border-white/5 pb-1">
                  <span className="text-muted-foreground/60">Total RAM:</span>
                  <span className="text-foreground">{run.systemInfo.ramGb.toFixed(1)} GB</span>
                </div>
                <div className="flex justify-between border-b border-white/5 pb-1">
                  <span className="text-muted-foreground/60">CPU Sockets:</span>
                  <span className="text-foreground">{socketCount}</span>
                </div>
                <div className="flex justify-between border-b border-white/5 pb-1">
                  <span className="text-muted-foreground/60">GPU Units:</span>
                  <span className="text-foreground">{run.systemInfo.gpuCount}</span>
                </div>
              </div>

              {socketCount > 0 && (
                <div className="mt-4 pt-4 border-t border-white/5">
                  <span className="text-primary/60 text-[11px] font-bold uppercase mb-2 block tracking-widest">Socket Mapping</span>
                  {Array.from(socketDomains.entries()).map(([socket, domains]) => (
                    <div key={socket} className="flex gap-4 items-center text-xs mb-1">
                      <span className="text-primary/40">[{socket}]</span>
                      <span className="text-foreground/70">{domains.join(', ')}</span>
                    </div>
                  ))}
                </div>
              )}

              {run.systemInfo.gpuCount > 0 && run.systemInfo.gpuNames && (
                <div className="mt-4 pt-4 border-t border-white/5">
                  <span className="text-primary/60 text-[11px] font-bold uppercase mb-2 block tracking-widest">Accelerator Devices</span>
                  {(typeof run.systemInfo.gpuNames === 'string' 
                    ? run.systemInfo.gpuNames.split(',')
                    : run.systemInfo.gpuNames).map((gpu, i) => (
                      <div key={i} className="flex gap-4 items-center text-xs mb-1">
                        <span className="text-primary/40">GPU:{i}</span>
                        <span className="text-foreground/70">{gpu.trim()}</span>
                      </div>
                    ))
                  }
                </div>
              )}
            </div>
          </div>
        ) : (
          <div className="p-12 text-center border border-dashed border-border rounded-lg bg-muted/5">
            <p className="text-sm text-muted-foreground">No detailed hardware specification available for this run.</p>
          </div>
        )}
      </div>
    </div>
  )
}
