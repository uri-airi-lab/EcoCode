'use client'

import { useMemo, useState } from 'react'
import { MetricRecord } from '@/lib/mock-data'

// ── Phase colour palette ───────────────────────────────────────────────────────
const PHASE_COLORS: Record<string, { bg: string; text: string; label: string }> = {
  data_loading:    { bg: 'bg-slate-500',   text: 'text-slate-100',  label: 'Data Loading'    },
  tokenization:    { bg: 'bg-slate-400',   text: 'text-slate-900',  label: 'Tokenization'    },
  warmup:          { bg: 'bg-slate-600',   text: 'text-slate-100',  label: 'Warmup'          },
  forward:         { bg: 'bg-blue-600',    text: 'text-blue-100',   label: 'Forward'         },
  backward:        { bg: 'bg-indigo-600',  text: 'text-indigo-100', label: 'Backward'        },
  optimizer_step:  { bg: 'bg-emerald-600', text: 'text-emerald-100',label: 'Optimizer Step'  },
  eval:            { bg: 'bg-violet-600',  text: 'text-violet-100', label: 'Eval'            },
  checkpoint_save: { bg: 'bg-amber-600',   text: 'text-amber-900',  label: 'Checkpoint'      },
  policy_rollout:  { bg: 'bg-cyan-600',    text: 'text-cyan-100',   label: 'Policy Rollout'  },
  reference_forward:{ bg: 'bg-sky-500',    text: 'text-sky-100',    label: 'Ref Forward'     },
  reward_model:    { bg: 'bg-rose-500',    text: 'text-rose-100',   label: 'Reward Model'    },
  ppo_update:      { bg: 'bg-pink-600',    text: 'text-pink-100',   label: 'PPO Update'      },
  dpo_update:      { bg: 'bg-fuchsia-600', text: 'text-fuchsia-100',label: 'DPO Update'      },
  grpo_update:     { bg: 'bg-teal-600',    text: 'text-teal-100',   label: 'GRPO Update'     },
  prefill:         { bg: 'bg-violet-500',  text: 'text-violet-100', label: 'Prefill'         },
  decode:          { bg: 'bg-purple-500',  text: 'text-purple-100', label: 'Decode'          },
  model_load:      { bg: 'bg-orange-600',  text: 'text-orange-900', label: 'Model Load'      },
  kv_cache_ops:    { bg: 'bg-amber-500',   text: 'text-amber-900',  label: 'KV Cache'        },
  done:            { bg: 'bg-emerald-500', text: 'text-emerald-100',label: 'Done'            },
}

const FALLBACK = { bg: 'bg-muted', text: 'text-muted-foreground', label: '' }

function phaseStyle(phase: string) {
  // Handle "epoch:N" labels
  if (phase.startsWith('epoch:')) return { bg: 'bg-indigo-400', text: 'text-indigo-100', label: phase.replace('epoch:', 'Epoch ') }
  // Handle "deepspeed_zeroN_init"
  if (phase.startsWith('deepspeed_')) return { bg: 'bg-cyan-600', text: 'text-cyan-100', label: phase.replace('deepspeed_', 'DS ') }
  return PHASE_COLORS[phase] ?? { ...FALLBACK, label: phase }
}

// ── Types ──────────────────────────────────────────────────────────────────────

interface PhaseSegment {
  phase: string
  startMs: number
  endMs: number
  energyJ: number    // sum of all energy components (gpu + cpu + mem + igpu) in this segment
  recordCount: number
}

// ── Helpers ────────────────────────────────────────────────────────────────────

/** Sum all four energy components for one record (all in Joules, all nullable except cpu/gpu). */
function recordEnergy(r: MetricRecord): number {
  return r.gpu_energy + r.cpu_energy + (r.mem_energy ?? 0) + (r.igpu_energy ?? 0)
}

function buildSegments(records: MetricRecord[]): PhaseSegment[] {
  const phased = records.filter(r => r.phase)
  if (phased.length === 0) return []

  const segments: PhaseSegment[] = []
  let current: PhaseSegment | null = null

  for (const r of phased) {
    const phase = r.phase!
    if (!current || current.phase !== phase) {
      if (current) {
        current.endMs = r.timestamp
        segments.push(current)
      }
      current = {
        phase,
        startMs: r.timestamp,
        endMs: r.timestamp,
        energyJ: recordEnergy(r),
        recordCount: 1,
      }
    } else {
      current.endMs = r.timestamp
      current.energyJ += recordEnergy(r)
      current.recordCount++
    }
  }
  if (current) {
    current.endMs = phased[phased.length - 1].timestamp
    segments.push(current)
  }

  return segments
}

function formatMs(ms: number): string {
  if (ms < 1000) return `${Math.round(ms)}ms`
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`
  return `${(ms / 60_000).toFixed(1)}m`
}

function formatEnergy(j: number): string {
  if (j < 1) return `${(j * 1000).toFixed(0)}mJ`
  if (j < 1000) return `${j.toFixed(1)}J`
  return `${(j / 3600).toFixed(2)}Wh`
}

// ── Legend ─────────────────────────────────────────────────────────────────────

function PhaseLegend({ phases }: { phases: string[] }) {
  return (
    <div className="flex flex-wrap gap-2 mt-3">
      {phases.map(p => {
        const s = phaseStyle(p)
        return (
          <span key={p} className="flex items-center gap-1.5 text-xs text-muted-foreground">
            <span className={`w-2.5 h-2.5 rounded-sm shrink-0 ${s.bg}`} />
            {s.label || p}
          </span>
        )
      })}
    </div>
  )
}

// ── Main component ─────────────────────────────────────────────────────────────

interface PhaseTimelineProps {
  records: MetricRecord[]
}

export function PhaseTimeline({ records }: PhaseTimelineProps) {
  const [tooltip, setTooltip] = useState<{ seg: PhaseSegment; x: number; y: number } | null>(null)

  const segments = useMemo(() => buildSegments(records), [records])

  if (segments.length === 0) return null

  const totalMs = segments[segments.length - 1].endMs - segments[0].startMs
  const uniquePhases = [...new Set(segments.map(s => s.phase))]

  // Aggregate per-phase energy for the summary bar
  const phaseEnergy: Record<string, number> = {}
  for (const s of segments) {
    phaseEnergy[s.phase] = (phaseEnergy[s.phase] ?? 0) + s.energyJ
  }
  const totalEnergy = Object.values(phaseEnergy).reduce((a, b) => a + b, 0)

  return (
    <div className="chart-container">
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-lg font-semibold text-foreground">Phase Timeline</h2>
        <span className="text-xs text-muted-foreground">{segments.length} transitions · {formatMs(totalMs)} total</span>
      </div>

      {/* Timeline bar */}
      <div
        className="relative flex w-full h-10 rounded-md overflow-hidden border border-border"
        onMouseLeave={() => setTooltip(null)}
      >
        {segments.map((seg, i) => {
          const widthPct = totalMs > 0 ? ((seg.endMs - seg.startMs) / totalMs) * 100 : 0
          const s = phaseStyle(seg.phase)
          return (
            <div
              key={i}
              className={`relative flex items-center justify-center overflow-hidden ${s.bg} transition-opacity hover:opacity-90 cursor-pointer`}
              style={{ width: `${widthPct}%`, minWidth: widthPct > 0 ? '2px' : 0 }}
              onMouseEnter={(e) => {
                const rect = e.currentTarget.getBoundingClientRect()
                setTooltip({ seg, x: rect.left + rect.width / 2, y: rect.top })
              }}
            >
              {widthPct > 5 && (
                <span className={`text-[10px] font-medium truncate px-1 select-none ${s.text}`}>
                  {s.label || seg.phase}
                </span>
              )}
            </div>
          )
        })}
      </div>

      {/* Energy per phase breakdown */}
      {totalEnergy > 0 && (
        <>
          <p className="text-xs text-muted-foreground mt-4 mb-1.5">Energy by phase</p>
          <div className="flex w-full h-4 rounded overflow-hidden border border-border">
            {Object.entries(phaseEnergy).map(([phase, energy]) => {
              const widthPct = (energy / totalEnergy) * 100
              const s = phaseStyle(phase)
              return (
                <div
                  key={phase}
                  className={`${s.bg} opacity-80`}
                  style={{ width: `${widthPct}%`, minWidth: widthPct > 0 ? '2px' : 0 }}
                  title={`${s.label || phase}: ${formatEnergy(energy)} (${widthPct.toFixed(1)}%)`}
                />
              )
            })}
          </div>
          {/* Top 3 energy consumers */}
          <div className="flex flex-wrap gap-x-4 gap-y-1 mt-2">
            {Object.entries(phaseEnergy)
              .sort((a, b) => b[1] - a[1])
              .slice(0, 4)
              .map(([phase, energy]) => {
                const s = phaseStyle(phase)
                const pct = totalEnergy > 0 ? (energy / totalEnergy * 100).toFixed(1) : '0'
                return (
                  <span key={phase} className="flex items-center gap-1.5 text-xs text-muted-foreground">
                    <span className={`w-2 h-2 rounded-sm ${s.bg}`} />
                    {s.label || phase}: <span className="text-foreground font-medium">{pct}%</span>
                  </span>
                )
              })}
          </div>
        </>
      )}

      <PhaseLegend phases={uniquePhases} />

      <p className="text-[11px] text-muted-foreground/60 mt-3 leading-relaxed">
        Energy is attributed to the phase active at each sampling tick (default 1 s).
        Fast-cycling phases (forward / backward / optimizer_step) share a tick, so their
        individual shares are approximate. Long phases (eval, data_loading, checkpoint_save)
        are attributed accurately.
      </p>

      {/* Floating tooltip */}
      {tooltip && (
        <div
          className="fixed z-50 pointer-events-none bg-popover border border-border rounded-lg shadow-lg px-3 py-2 text-xs -translate-x-1/2 -translate-y-full"
          style={{ left: tooltip.x, top: tooltip.y - 8 }}
        >
          <p className="font-semibold text-foreground mb-1">{phaseStyle(tooltip.seg.phase).label || tooltip.seg.phase}</p>
          <p className="text-muted-foreground">Duration: <span className="text-foreground">{formatMs(tooltip.seg.endMs - tooltip.seg.startMs)}</span></p>
          {tooltip.seg.energyJ > 0 && (
            <p className="text-muted-foreground">Energy: <span className="text-foreground">{formatEnergy(tooltip.seg.energyJ)}</span></p>
          )}
          <p className="text-muted-foreground">Records: <span className="text-foreground">{tooltip.seg.recordCount}</span></p>
        </div>
      )}
    </div>
  )
}
