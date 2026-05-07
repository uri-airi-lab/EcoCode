'use client'

import { useMemo, useState, useEffect } from 'react'
import { createPortal } from 'react-dom'
import { MetricRecord } from '@/lib/mock-data'

// ── Phase colour palette ───────────────────────────────────────────────────────
// ── Phase colour palette ───────────────────────────────────────────────────────
const PHASE_COLORS: Record<string, { bg: string; text: string; label: string }> = {
  data_loading:    { bg: 'bg-slate-500/80',   text: 'text-slate-50',  label: 'Data Loading'    },
  tokenization:    { bg: 'bg-slate-400/80',   text: 'text-slate-50',  label: 'Tokenization'    },
  warmup:          { bg: 'bg-indigo-400/70',  text: 'text-indigo-50', label: 'Warmup'          },
  forward:         { bg: 'bg-sky-500/80',     text: 'text-white',     label: 'Forward'         },
  backward:        { bg: 'bg-sky-600/80',     text: 'text-white',     label: 'Backward'        },
  optimizer_step:  { bg: 'bg-blue-600/80',    text: 'text-white',     label: 'Optimizer Step'  },
  eval:            { bg: 'bg-violet-600/80',  text: 'text-white',     label: 'Eval'            },
  checkpoint_save: { bg: 'bg-amber-500/80',   text: 'text-white',     label: 'Checkpoint'      },
  policy_rollout:  { bg: 'bg-cyan-500/80',    text: 'text-white',     label: 'Policy Rollout'  },
  reference_forward:{ bg: 'bg-sky-400/60',     text: 'text-white',     label: 'Ref Forward'     },
  reward_model:    { bg: 'bg-rose-400/80',    text: 'text-white',     label: 'Reward Model'    },
  ppo_update:      { bg: 'bg-indigo-500/80',  text: 'text-white',     label: 'PPO Update'      },
  dpo_update:      { bg: 'bg-indigo-700/80',  text: 'text-white',     label: 'DPO Update'      },
  grpo_update:     { bg: 'bg-teal-500/80',    text: 'text-white',     label: 'GRPO Update'     },
  prefill:         { bg: 'bg-violet-400/80',  text: 'text-white',     label: 'Prefill'         },
  decode:          { bg: 'bg-purple-400/80',  text: 'text-white',     label: 'Decode'          },
  model_load:      { bg: 'bg-orange-500/80',  text: 'text-white',     label: 'Model Load'      },
  kv_cache_ops:    { bg: 'bg-amber-400/70',   text: 'text-white',     label: 'KV Cache'        },
  done:            { bg: 'bg-primary',        text: 'text-primary-foreground', label: 'Done'    },
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

// ── Epoch tracking ─────────────────────────────────────────────────────────────
// Primary source: records[].epoch_num (a sticky integer set by log_epoch()).
// Fallback: records[].phase = "epoch:N" (old behaviour, kept for existing runs).

interface EpochEnergy {
  epoch: number
  energyJ: number
  durationMs: number
}

interface EpochBoundary {
  epoch: number
  timestampMs: number
}

function buildEpochBoundaries(records: MetricRecord[]): EpochBoundary[] {
  const out: EpochBoundary[] = []
  let last: number | null = null

  for (const r of records) {
    if (r.epoch_num != null && r.epoch_num !== last) {
      out.push({ epoch: r.epoch_num, timestampMs: r.timestamp })
      last = r.epoch_num
    }
  }
  if (out.length > 0) return out

  // Fallback for runs recorded before the epoch_num field existed
  let lastPhase: string | null = null
  for (const r of records) {
    if (r.phase?.startsWith('epoch:') && r.phase !== lastPhase) {
      const n = parseInt(r.phase.split(':')[1], 10)
      if (!isNaN(n)) {
        out.push({ epoch: n, timestampMs: r.timestamp })
        lastPhase = r.phase
      }
    }
  }
  return out
}

function computeEpochEnergies(records: MetricRecord[]): EpochEnergy[] {
  // Primary: group records by epoch_num
  type Acc = { energyJ: number; minMs: number; maxMs: number }
  const map = new Map<number, Acc>()
  let hasEpochNum = false

  for (const r of records) {
    if (r.epoch_num != null) {
      hasEpochNum = true
      const e = recordEnergy(r)
      const acc = map.get(r.epoch_num)
      if (acc) {
        acc.energyJ += e
        acc.maxMs = Math.max(acc.maxMs, r.timestamp)
      } else {
        map.set(r.epoch_num, { energyJ: e, minMs: r.timestamp, maxMs: r.timestamp })
      }
    }
  }

  if (hasEpochNum) {
    return Array.from(map.entries())
      .sort((a, b) => a[0] - b[0])
      .map(([epoch, { energyJ, minMs, maxMs }]) => ({ epoch, energyJ, durationMs: maxMs - minMs }))
  }

  // Fallback: infer epochs from phase = "epoch:N" segments
  const segments = buildSegments(records)
  const result: EpochEnergy[] = []
  let current: EpochEnergy | null = null
  for (const seg of segments) {
    if (seg.phase.startsWith('epoch:')) {
      if (current !== null) result.push(current)
      current = { epoch: parseInt(seg.phase.split(':')[1], 10), energyJ: seg.energyJ, durationMs: seg.endMs - seg.startMs }
    } else if (current !== null) {
      current.energyJ += seg.energyJ
      current.durationMs += seg.endMs - seg.startMs
    }
  }
  if (current !== null) result.push(current)
  return result
}

// ── Legend ─────────────────────────────────────────────────────────────────────

function PhaseLegend({ phases }: { phases: string[] }) {
  return (
    <div className="grid grid-cols-2 lg:grid-cols-4 gap-x-4 gap-y-2 mt-6">
      {phases.map(p => {
        const s = phaseStyle(p)
        return (
          <span key={p} className="flex items-center gap-2 text-xs text-muted-foreground">
            <span className={`w-3 h-3 rounded-[3px] shrink-0 ${s.bg}`} />
            <span className="truncate">{s.label || p}</span>
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
  const [mounted, setMounted] = useState(false)

  useEffect(() => {
    setMounted(true)
  }, [])

  const segments = useMemo(() => buildSegments(records), [records])
  const epochEnergies = useMemo(() => computeEpochEnergies(records), [records])
  const totalEpochEnergy = useMemo(
    () => epochEnergies.reduce((s, e) => s + e.energyJ, 0),
    [epochEnergies]
  )
  const epochBoundaries = useMemo(() => buildEpochBoundaries(records), [records])
  const epochTicks = useMemo(() => {
    if (segments.length === 0) return []
    const firstMs = segments[0].startMs
    const totalMs = segments[segments.length - 1].endMs - firstMs
    if (totalMs === 0) return []
    return epochBoundaries.map(b => ({
      epochNum: b.epoch,
      leftPct: ((b.timestampMs - firstMs) / totalMs) * 100,
    }))
  }, [epochBoundaries, segments])

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

      {/* Timeline bar — outer wrapper provides space above for epoch labels */}
      <div className="relative pt-5">
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
                className={`relative flex items-center justify-center overflow-hidden ${s.bg} border-r border-background/20 transition-opacity hover:opacity-90 cursor-pointer`}
                style={{ width: `${widthPct}%`, minWidth: widthPct > 0 ? '2px' : 0 }}
                onMouseMove={(e) => {
                  setTooltip({ seg, x: e.clientX, y: e.clientY })
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
        {/* Epoch boundary tick marks */}
        {epochTicks.map(({ epochNum, leftPct }) => (
          <div
            key={`epoch-tick-${epochNum}`}
            className="absolute top-0 flex flex-col items-center pointer-events-none"
            style={{ left: `${leftPct}%` }}
          >
            <span className="text-[9px] font-medium text-indigo-400 leading-none pb-0.5 -translate-x-1/2 whitespace-nowrap">
              E{epochNum}
            </span>
            <div className="w-px h-10 bg-white/30" />
          </div>
        ))}
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
                  className={`${s.bg} opacity-80 border-r border-background/10 transition-opacity hover:opacity-100 cursor-help`}
                  style={{ width: `${widthPct}%`, minWidth: widthPct > 0 ? '2px' : 0 }}
                  onMouseMove={(e) => {
                    setTooltip({ 
                      seg: { 
                        phase, 
                        energyJ: energy, 
                        startMs: 0, 
                        endMs: 0, 
                        recordCount: 0 
                      }, 
                      x: e.clientX, 
                      y: e.clientY 
                    })
                  }}
                  onMouseLeave={() => setTooltip(null)}
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

      {/* Energy by epoch */}
      {epochEnergies.length > 0 && (
        <>
          <p className="text-xs text-muted-foreground mt-4 mb-1.5">Energy by epoch</p>
          <div className="flex w-full h-4 rounded overflow-hidden border border-border">
            {epochEnergies.map((e, i) => {
              const widthPct = totalEpochEnergy > 0 ? (e.energyJ / totalEpochEnergy) * 100 : 0
              const bgColors = ['bg-violet-500', 'bg-violet-600', 'bg-violet-700', 'bg-violet-800']
              return (
                <div
                  key={e.epoch}
                  className={`${bgColors[i % bgColors.length]} opacity-80 border-r border-background/10 transition-opacity hover:opacity-100 cursor-help`}
                  style={{ width: `${widthPct}%`, minWidth: widthPct > 0 ? '2px' : 0 }}
                  onMouseMove={(ev) => {
                    setTooltip({
                      seg: { phase: `epoch:${e.epoch}`, energyJ: e.energyJ, startMs: 0, endMs: e.durationMs, recordCount: 0 },
                      x: ev.clientX, y: ev.clientY,
                    })
                  }}
                  onMouseLeave={() => setTooltip(null)}
                />
              )
            })}
          </div>
          <div className="flex flex-wrap gap-x-4 gap-y-1 mt-2">
            {epochEnergies.map((e) => {
              const pct = totalEpochEnergy > 0 ? (e.energyJ / totalEpochEnergy * 100).toFixed(1) : '0'
              return (
                <span key={e.epoch} className="flex items-center gap-1.5 text-xs text-muted-foreground">
                  <span className="w-2 h-2 rounded-sm bg-violet-500" />
                  Epoch {e.epoch}:{' '}
                  <span className="text-foreground font-medium">{formatEnergy(e.energyJ)}</span>
                  <span className="text-muted-foreground/60">({pct}%)</span>
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

      {/* Floating tooltip using Portal */}
      {mounted && tooltip && createPortal(
        <div
          className="fixed z-[9999] pointer-events-none bg-popover border border-border rounded-lg shadow-lg px-3 py-2 text-xs -translate-x-1/2 -translate-y-full whitespace-nowrap"
          style={{ 
            left: tooltip.x, 
            top: tooltip.y - 12,
          }}
        >
          <p className="font-semibold text-foreground mb-1">{phaseStyle(tooltip.seg.phase).label || tooltip.seg.phase}</p>
          {tooltip.seg.endMs > 0 && (
            <p className="text-muted-foreground font-mono">Duration: <span className="text-foreground">{formatMs(tooltip.seg.endMs - tooltip.seg.startMs)}</span></p>
          )}
          {tooltip.seg.energyJ > 0 && (
            <p className="text-muted-foreground font-mono">Energy: <span className="text-foreground">{formatEnergy(tooltip.seg.energyJ)}</span></p>
          )}
          {tooltip.seg.recordCount > 0 && (
            <p className="text-muted-foreground font-mono">Records: <span className="text-foreground">{tooltip.seg.recordCount}</span></p>
          )}
        </div>,
        document.body
      )}
    </div>
  )
}
