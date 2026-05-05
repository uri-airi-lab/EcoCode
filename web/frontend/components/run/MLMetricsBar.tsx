'use client'

import { useMemo } from 'react'
import { MetricRecord } from '@/lib/mock-data'
import { TrendingDown, Zap, Hash, Layers, Activity } from 'lucide-react'

interface MLStats {
  currentLoss:     number | null
  currentLR:       number | null
  currentStep:     number | null
  tokensProcessed: number | null
  sequenceLength:  number | null
  currentPhase:    string | null
  energyPerStep:   number | null   // J per optimizer step
  energyPerToken:  number | null   // J per 1k tokens
  currentAccuracy: number | null
}

function extractStats(records: MetricRecord[]): MLStats {
  if (records.length === 0) {
    return {
      currentLoss: null, currentLR: null, currentStep: null,
      tokensProcessed: null, sequenceLength: null, currentPhase: null,
      energyPerStep: null, energyPerToken: null, currentAccuracy: null,
    }
  }

  // Walk backwards for the most recent non-null values
  let currentLoss:     number | null = null
  let currentLR:       number | null = null
  let currentStep:     number | null = null
  let tokensProcessed: number | null = null
  let sequenceLength:  number | null = null
  let currentPhase:    string | null = null
  let currentAccuracy: number | null = null

  for (let i = records.length - 1; i >= 0; i--) {
    const r = records[i]
    if (currentLoss     == null && r.loss             != null) currentLoss     = r.loss
    if (currentLR       == null && r.learning_rate    != null) currentLR       = r.learning_rate
    if (currentStep     == null && r.training_step    != null) currentStep     = r.training_step
    if (tokensProcessed == null && r.tokens_processed != null) tokensProcessed = r.tokens_processed
    if (sequenceLength  == null && r.sequence_length  != null) sequenceLength  = r.sequence_length
    if (currentPhase    == null && r.phase            != null) currentPhase    = r.phase
    if (currentAccuracy == null && r.accuracy         != null) currentAccuracy = r.accuracy

    if (currentLoss != null && currentLR != null && currentStep != null &&
        tokensProcessed != null && sequenceLength != null && currentPhase != null && currentAccuracy != null) break
  }

  // Derive efficiency metrics
  const totalEnergyJ = records.reduce(
    (s, r) => s + r.gpu_energy + r.cpu_energy + (r.mem_energy ?? 0) + (r.igpu_energy ?? 0), 0
  )
  const energyPerStep   = currentStep && currentStep > 0 && totalEnergyJ > 0
    ? totalEnergyJ / currentStep
    : null
  const energyPerToken  = tokensProcessed && tokensProcessed > 1000 && totalEnergyJ > 0
    ? (totalEnergyJ / tokensProcessed) * 1000   // J per 1k tokens
    : null

  return {
    currentLoss, currentLR, currentStep, tokensProcessed,
    sequenceLength, currentPhase, energyPerStep, energyPerToken, currentAccuracy,
  }
}

function formatPhase(phase: string): string {
  if (phase.startsWith('epoch:')) return `Epoch ${phase.split(':')[1]}`
  if (phase.startsWith('deepspeed_')) return phase.replace('deepspeed_', 'DS ').replace('_init', '')
  return phase.replace(/_/g, ' ').replace(/\b\w/g, c => c.toUpperCase())
}

function formatTokens(n: number): string {
  if (n >= 1_000_000_000) return `${(n / 1_000_000_000).toFixed(1)}B`
  if (n >= 1_000_000)     return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000)         return `${(n / 1_000).toFixed(1)}K`
  return n.toString()
}

function Stat({ icon, label, value, sub }: {
  icon: React.ReactNode
  label: string
  value: string
  sub?: string
}) {
  return (
    <div className="flex items-start gap-3 px-4 py-3">
      <div className="mt-0.5 text-muted-foreground shrink-0">{icon}</div>
      <div className="min-w-0">
        <p className="text-[11px] text-muted-foreground uppercase tracking-wide font-medium">{label}</p>
        <p className="text-lg font-bold text-foreground font-mono leading-tight">{value}</p>
        {sub && <p className="text-[11px] text-muted-foreground mt-0.5">{sub}</p>}
      </div>
    </div>
  )
}

interface MLMetricsBarProps {
  records: MetricRecord[]
}

export function MLMetricsBar({ records }: MLMetricsBarProps) {
  const stats = useMemo(() => extractStats(records), [records])

  const hasAny = stats.currentLoss != null || stats.currentStep != null ||
                 stats.tokensProcessed != null || stats.currentPhase != null

  if (!hasAny) return null

  return (
    <div className="chart-container p-0 overflow-hidden">
      <div className="px-4 pt-3 pb-1 border-b border-border">
        <h2 className="text-sm font-semibold text-foreground">Training Overview</h2>
      </div>
      <div className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-5 divide-x divide-y sm:divide-y-0 divide-border">

        {stats.currentPhase != null && (
          <Stat
            icon={<Activity className="w-4 h-4" />}
            label="Current Phase"
            value={formatPhase(stats.currentPhase)}
          />
        )}

        {stats.currentLoss != null && (
          <Stat
            icon={<TrendingDown className="w-4 h-4" />}
            label="Loss"
            value={stats.currentLoss.toFixed(4)}
          />
        )}

        {stats.currentLR != null && (
          <Stat
            icon={<Activity className="w-4 h-4" />}
            label="Learning Rate"
            value={stats.currentLR.toExponential(2)}
          />
        )}

        {stats.currentAccuracy != null && (
          <Stat
            icon={<Layers className="w-4 h-4" />}
            label="Accuracy"
            value={`${stats.currentAccuracy.toFixed(2)}%`}
          />
        )}

        {stats.currentStep != null && (
          <Stat
            icon={<Hash className="w-4 h-4" />}
            label="Step"
            value={stats.currentStep.toLocaleString()}
          />
        )}

        {stats.tokensProcessed != null && (
          <Stat
            icon={<Layers className="w-4 h-4" />}
            label="Tokens Seen"
            value={formatTokens(stats.tokensProcessed)}
            sub={stats.sequenceLength ? `seq len ${stats.sequenceLength.toLocaleString()}` : undefined}
          />
        )}

        {stats.energyPerStep != null && (
          <Stat
            icon={<Zap className="w-4 h-4" />}
            label="J / Step"
            value={stats.energyPerStep < 1
              ? `${(stats.energyPerStep * 1000).toFixed(0)}mJ`
              : `${stats.energyPerStep.toFixed(2)}J`}
            sub="energy per optimizer step"
          />
        )}

        {stats.energyPerToken != null && (
          <Stat
            icon={<Zap className="w-4 h-4" />}
            label="J / 1K Tokens"
            value={stats.energyPerToken < 1
              ? `${(stats.energyPerToken * 1000).toFixed(0)}mJ`
              : `${stats.energyPerToken.toFixed(3)}J`}
            sub="energy efficiency"
          />
        )}

      </div>
    </div>
  )
}
