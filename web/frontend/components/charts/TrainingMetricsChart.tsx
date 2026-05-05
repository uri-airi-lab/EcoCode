'use client'

import { useMemo, useRef, useState, useEffect } from 'react'
import {
  ComposedChart,
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  Legend,
  ResponsiveContainer,
  ReferenceLine,
} from 'recharts'
import { MetricRecord } from '@/lib/mock-data'
import { chartColor } from '@/lib/chart-colors'
import { ChartInfoTooltip } from '@/components/ChartInfoTooltip'

interface DataPoint {
  step: number
  loss: number | null
  lr: number | null
  accuracy: number | null
  /** epoch boundary — drawn as a reference line */
  epochBoundary?: number
  /** Dynamically added custom metric values */
  [key: string]: number | null | undefined
}

interface EpochMarker {
  step: number
  epoch: number
}

function buildSeries(records: MetricRecord[]): { points: DataPoint[]; epochs: EpochMarker[]; customKeys: string[] } {
  const points: DataPoint[] = []
  const epochs: EpochMarker[] = []
  const customKeySet = new Set<string>()
  let lastEpoch: string | null = null

  for (const r of records) {
    if (r.training_step == null) continue

    // Detect epoch boundaries from "epoch:N" phase labels
    if (r.phase?.startsWith('epoch:')) {
      const e = parseInt(r.phase.split(':')[1], 10)
      if (r.phase !== lastEpoch && !isNaN(e)) {
        epochs.push({ step: r.training_step, epoch: e })
        lastEpoch = r.phase
      }
    }

    // Collect custom metric keys
    if (r.custom_metrics) {
      for (const key of Object.keys(r.custom_metrics)) {
        customKeySet.add(key)
      }
    }

    const hasAnyMetric = r.loss != null || r.learning_rate != null || r.accuracy != null ||
      (r.custom_metrics && Object.keys(r.custom_metrics).length > 0)
    if (!hasAnyMetric) continue

    const point: DataPoint = {
      step: r.training_step,
      loss: r.loss ?? null,
      lr: r.learning_rate ?? null,
      accuracy: r.accuracy ?? null,
    }

    // Flatten custom metrics into the data point
    if (r.custom_metrics) {
      for (const [key, value] of Object.entries(r.custom_metrics)) {
        point[`custom_${key}`] = value
      }
    }

    points.push(point)
  }

  return { points, epochs, customKeys: Array.from(customKeySet) }
}

// Custom tooltip
const CustomTooltip = ({ active, payload, label }: any) => {
  if (!active || !payload?.length) return null
  return (
    <div className="bg-popover border border-border rounded-lg shadow-lg px-3 py-2 text-xs">
      <p className="font-medium text-foreground mb-1">Step {label}</p>
      {payload.map((p: any) => (
        <p key={p.dataKey} style={{ color: p.color }} className="flex gap-2 justify-between">
          <span>{p.name}:</span>
          <span className="font-mono font-semibold">
            {p.dataKey === 'lr'
              ? p.value?.toExponential(2) ?? '—'
              : p.value?.toFixed(4) ?? '—'}
          </span>
        </p>
      ))}
    </div>
  )
}

/** Colour palette offsets for custom metrics (start after the 3 built-in colours) */
const CUSTOM_COLOR_OFFSET = 6
const PX_PER_POINT = 12
const MIN_WIDTH = 600

interface TrainingMetricsChartProps {
  records: MetricRecord[]
}

export function TrainingMetricsChart({ records }: TrainingMetricsChartProps) {
  const scrollRef = useRef<HTMLDivElement>(null)
  const [visibleRange, setVisibleRange] = useState({ start: 0, end: 100 })
  const { points, epochs, customKeys } = useMemo(() => buildSeries(records), [records])

  const hasLoss     = points.some(p => p.loss != null)
  const hasLR       = points.some(p => p.lr   != null)
  const hasAccuracy = points.some(p => p.accuracy != null)
  const hasCustom   = customKeys.length > 0

  if (!hasLoss && !hasLR && !hasAccuracy && !hasCustom) return null

  const lossColor = chartColor(5)   // red
  const lrColor   = chartColor(1)   // blue
  const accColor  = chartColor(3)   // emerald

  const chartWidth = Math.max(MIN_WIDTH, points.length * PX_PER_POINT)

  // Update visible range on scroll
  useEffect(() => {
    const handleScroll = () => {
      if (!scrollRef.current) return
      const container = scrollRef.current
      const scrollLeft = container.scrollLeft
      const containerWidth = container.clientWidth
      const scrollWidth = container.scrollWidth
      
      const startPercent = scrollLeft / scrollWidth
      const endPercent = (scrollLeft + containerWidth) / scrollWidth
      
      const start = Math.floor(startPercent * points.length)
      const end = Math.ceil(endPercent * points.length)
      
      setVisibleRange({ start, end })
    }

    const container = scrollRef.current
    if (container) {
      handleScroll() 
      container.addEventListener('scroll', handleScroll)
      return () => container.removeEventListener('scroll', handleScroll)
    }
  }, [points.length])

  const visibleTicks = useMemo(() => {
    const tickCount = 8
    const step = Math.max(1, Math.floor((visibleRange.end - visibleRange.start) / tickCount))
    const ticks = []
    for (let i = visibleRange.start; i < visibleRange.end; i += step) {
      if (i < points.length) {
        ticks.push(points[i].step)
      }
    }
    return ticks
  }, [visibleRange, points])

  // Compute y-domain for LR (might span many orders of magnitude)
  const lrValues  = points.map(p => p.lr).filter(Boolean) as number[]
  const lrMin     = lrValues.length ? Math.min(...lrValues) : 0
  const lrMax     = lrValues.length ? Math.max(...lrValues) : 1

  return (
    <div className="chart-container">
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-2">
          <h2 className="text-lg font-semibold text-foreground">Training Metrics</h2>
          <ChartInfoTooltip content="Loss and learning rate plotted over optimizer steps. Epoch boundaries are shown as vertical dashed lines. Custom metrics from log_custom() are also displayed." />
        </div>
        <div className="flex items-center gap-4 text-xs text-muted-foreground flex-wrap">
          {hasLoss && (
            <span className="flex items-center gap-1.5">
              <span className="w-4 h-0.5 rounded" style={{ background: lossColor }} />
              Loss
            </span>
          )}
          {hasLR && (
            <span className="flex items-center gap-1.5">
              <span className="w-4 h-0.5 rounded" style={{ background: lrColor }} />
              LR
            </span>
          )}
          {hasAccuracy && (
            <span className="flex items-center gap-1.5">
              <span className="w-4 h-0.5 rounded" style={{ background: accColor }} />
              Accuracy
            </span>
          )}
          {customKeys.map((key, i) => (
            <span key={key} className="flex items-center gap-1.5">
              <span className="w-4 h-0.5 rounded" style={{ background: chartColor(CUSTOM_COLOR_OFFSET + i) }} />
              {key}
            </span>
          ))}
        </div>
      </div>
      <div className="relative">
        <div ref={scrollRef} className="overflow-x-auto">
          <ComposedChart width={chartWidth} height={260} data={points} margin={{ top: 4, right: 48, left: 8, bottom: 4 }}>
            <CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--border))" opacity={0.5} />
            <XAxis
              dataKey="step"
              type="number"
              domain={['dataMin', 'dataMax']}
              ticks={visibleTicks}
              tick={{ fontSize: 11, fill: 'hsl(var(--muted-foreground))' }}
              tickLine={false}
              axisLine={{ stroke: 'hsl(var(--border))' }}
              label={{ value: 'Step', position: 'insideBottomRight', offset: -4, fontSize: 11, fill: 'hsl(var(--muted-foreground))' }}
            />

            {/* Left Y: loss */}
            {hasLoss && (
              <YAxis
                yAxisId="loss"
                orientation="left"
                tick={{ fontSize: 11, fill: 'hsl(var(--muted-foreground))' }}
                tickLine={false}
                axisLine={false}
                tickFormatter={(v) => v.toFixed(2)}
                label={{ value: 'Loss', angle: -90, position: 'insideLeft', offset: 12, fontSize: 11, fill: 'hsl(var(--muted-foreground))' }}
              />
            )}

            {/* Right Y: learning rate */}
            {hasLR && (
              <YAxis
                yAxisId="lr"
                orientation="right"
                tick={{ fontSize: 11, fill: 'hsl(var(--muted-foreground))' }}
                tickLine={false}
                axisLine={false}
                tickFormatter={(v) => v.toExponential(1)}
                domain={[lrMin * 0.9, lrMax * 1.1]}
                label={{ value: 'LR', angle: 90, position: 'insideRight', offset: 24, fontSize: 11, fill: 'hsl(var(--muted-foreground))' }}
              />
            )}
  
            {hasAccuracy && (
              <YAxis
                yAxisId="accuracy"
                orientation="right"
                tick={{ fontSize: 11, fill: 'hsl(var(--muted-foreground))' }}
                tickLine={false}
                axisLine={false}
                domain={[0, 100]}
                tickFormatter={(v) => `${v}%`}
                label={{ value: 'Accuracy', angle: 90, position: 'insideRight', offset: 0, fontSize: 11, fill: 'hsl(var(--muted-foreground))' }}
              />
            )}

            {/* Custom metrics share the loss Y-axis (or get their own if loss is absent) */}
            {hasCustom && !hasLoss && (
              <YAxis
                yAxisId="custom"
                orientation="left"
                tick={{ fontSize: 11, fill: 'hsl(var(--muted-foreground))' }}
                tickLine={false}
                axisLine={false}
                tickFormatter={(v) => v.toFixed(2)}
                label={{ value: 'Custom', angle: -90, position: 'insideLeft', offset: 12, fontSize: 11, fill: 'hsl(var(--muted-foreground))' }}
              />
            )}

            <Tooltip content={<CustomTooltip />} />

            {/* Epoch boundary reference lines */}
            {epochs.map(e => (
              <ReferenceLine
                key={`epoch-${e.epoch}`}
                yAxisId={hasLoss ? 'loss' : (hasCustom ? 'custom' : (hasAccuracy ? 'accuracy' : 'lr'))}
                x={e.step}
                stroke="hsl(var(--muted-foreground))"
                strokeDasharray="4 4"
                strokeOpacity={0.5}
                label={{ value: `E${e.epoch}`, fontSize: 9, fill: 'hsl(var(--muted-foreground))', position: 'top' }}
              />
            ))}

            {hasLoss && (
              <Line
                yAxisId="loss"
                dataKey="loss"
                name="Loss"
                stroke={lossColor}
                strokeWidth={1.5}
                dot={false}
                activeDot={{ r: 3 }}
                connectNulls
                isAnimationActive={false}
              />
            )}
            {hasLR && (
              <Line
                yAxisId="lr"
                dataKey="lr"
                name="LR"
                stroke={lrColor}
                strokeWidth={1.5}
                dot={false}
                strokeDasharray="5 2"
                activeDot={{ r: 3 }}
                connectNulls
                isAnimationActive={false}
              />
            )}
            {hasAccuracy && (
              <Line
                yAxisId="accuracy"
                dataKey="accuracy"
                name="Accuracy"
                stroke={accColor}
                strokeWidth={2}
                dot={false}
                activeDot={{ r: 4 }}
                connectNulls
                isAnimationActive={false}
              />
            )}

            {/* Dynamic custom metric lines */}
            {customKeys.map((key, i) => (
              <Line
                key={key}
                yAxisId={hasLoss ? 'loss' : 'custom'}
                dataKey={`custom_${key}`}
                name={key}
                stroke={chartColor(CUSTOM_COLOR_OFFSET + i)}
                strokeWidth={1.5}
                dot={false}
                strokeDasharray="3 3"
                activeDot={{ r: 3 }}
                connectNulls
                isAnimationActive={false}
              />
            ))}
          </ComposedChart>
        </div>
      </div>
    </div>
  )
}
