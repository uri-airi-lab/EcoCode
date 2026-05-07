'use client'

import { useMemo, useRef, useState, useCallback, useEffect } from 'react'
import dynamic from 'next/dynamic'
import type { RefObject } from 'react'
import type { UPlotChartHandle } from './UPlotChart'
import { makeTooltipPlugin, AXIS_STROKE, GRID_STROKE, AXIS_FONT } from './uplot-helpers'
import { MetricRecord } from '@/lib/mock-data'
import { chartColor } from '@/lib/chart-colors'
import { ChartInfoTooltip } from '@/components/ChartInfoTooltip'
import { lttbSample } from '@/lib/lttb'
import { VirtualizedChartList } from './VirtualizedChartList'
import type { MetricSeries } from './VirtualizedChartList'

const UPlotChart = dynamic(() => import('./UPlotChart'), { ssr: false })

const LTTB_THRESHOLD = 600
const HEIGHT = 260

interface EpochMarker { step: number; epoch: number }

interface TrainingMetricsChartProps {
  records: MetricRecord[]
}

export function TrainingMetricsChart({ records }: TrainingMetricsChartProps) {
  const uplotRef = useRef<UPlotChartHandle>(null) as RefObject<UPlotChartHandle>
  const [zoomRange, setZoomRange] = useState<{ min: number; max: number } | null>(null)
  const [isZoomed, setIsZoomed] = useState(false)

  const handleZoom = useCallback((min: number, max: number) => {
    setIsZoomed(true)
    setZoomRange({ min, max })
    uplotRef.current?.setScale(min, max)
  }, [])

  // When zoom is cleared, reset the chart scale
  useEffect(() => {
    if (!zoomRange) uplotRef.current?.resetZoom()
  }, [zoomRange])

  // Records with training_step AND at least one metric
  const stepRecords = useMemo(() => records.filter(r => {
    if (r.training_step == null) return false
    return r.loss != null || r.learning_rate != null || r.accuracy != null ||
      (r.custom_metrics && Object.keys(r.custom_metrics).length > 0)
  }), [records])

  // Epoch boundaries for annotation lines
  const allEpochs = useMemo(() => {
    const epochs: EpochMarker[] = []
    let lastEpochNum: number | null = null
    let lastEpochPhase: string | null = null
    records.forEach(r => {
      if (r.training_step == null) return
      if (r.epoch_num != null) {
        if (r.epoch_num !== lastEpochNum) {
          epochs.push({ step: r.training_step, epoch: r.epoch_num })
          lastEpochNum = r.epoch_num
        }
      } else if (r.phase?.startsWith('epoch:')) {
        const e = parseInt(r.phase.split(':')[1], 10)
        if (r.phase !== lastEpochPhase && !isNaN(e)) {
          epochs.push({ step: r.training_step, epoch: e })
          lastEpochPhase = r.phase
        }
      }
    })
    return epochs
  }, [records])

  // Client-side zoom: filter stepRecords by step range when zoomed, LTTB otherwise
  const visibleRecords = useMemo(() => {
    if (zoomRange) {
      return stepRecords.filter(r =>
        r.training_step! >= zoomRange.min && r.training_step! <= zoomRange.max
      )
    }
    return lttbSample(stepRecords, LTTB_THRESHOLD, r =>
      r.loss ?? r.accuracy ?? r.learning_rate ?? 0
    )
  }, [stepRecords, zoomRange])

  // ── Custom metrics — extracted to VirtualizedChartList ──────────────────

  const customKeys = useMemo(() => {
    const keys = new Set<string>()
    stepRecords.forEach(r => {
      if (r.custom_metrics) Object.keys(r.custom_metrics).forEach(k => keys.add(k))
    })
    return Array.from(keys).sort()
  }, [stepRecords])

  const customSeries = useMemo((): MetricSeries[] =>
    customKeys.map(key => {
      const steps: number[] = [], values: number[] = []
      visibleRecords.forEach(r => {
        const v = r.custom_metrics?.[key]
        if (v != null) { steps.push(r.training_step!); values.push(v) }
      })
      return { key, steps, values }
    }),
    [customKeys, visibleRecords]
  )

  // ── Main chart: Loss, LR, Accuracy ───────────────────────────────────────

  const { uplotData, hasLoss, hasLR, hasAccuracy } = useMemo(() => {
    const steps: number[] = [], loss: (number | null)[] = []
    const lr: (number | null)[] = [], acc: (number | null)[] = []

    visibleRecords.forEach(r => {
      steps.push(r.training_step!)
      loss.push(r.loss ?? null)
      lr.push(r.learning_rate ?? null)
      acc.push(r.accuracy ?? null)
    })

    return {
      uplotData: [steps, loss, lr, acc] as any,
      hasLoss: loss.some(v => v != null),
      hasLR: lr.some(v => v != null),
      hasAccuracy: acc.some(v => v != null),
    }
  }, [visibleRecords])

  // Axis and series configuration matches what was visible — recreates only when flags change
  const options = useMemo(() => {
    const axes: object[] = [
      // x-axis (training step)
      {
        stroke: AXIS_STROKE,
        grid: { stroke: GRID_STROKE, width: 1 },
        ticks: { stroke: GRID_STROKE, width: 1 },
        font: AXIS_FONT,
        values: (_u: unknown, vals: (number | null)[]) =>
          vals.map(v => v == null ? '' : `Step ${v}`),
      },
    ]

    if (hasLoss) {
      axes.push({
        scale: 'loss',
        side: 3, // left
        label: 'Loss',
        stroke: AXIS_STROKE,
        grid: { stroke: GRID_STROKE, width: 1 },
        ticks: { stroke: GRID_STROKE, width: 1 },
        font: AXIS_FONT,
        values: (_u: unknown, vals: (number | null)[]) =>
          vals.map(v => v == null ? '' : v.toFixed(3)),
      })
    }

    if (hasLR) {
      axes.push({
        scale: 'lr',
        side: 1, // right
        label: 'LR',
        stroke: AXIS_STROKE,
        grid: { show: false },
        ticks: { stroke: GRID_STROKE, width: 1 },
        font: AXIS_FONT,
        values: (_u: unknown, vals: (number | null)[]) =>
          vals.map(v => v == null ? '' : Number(v).toExponential(1)),
      })
    }

    if (hasAccuracy) {
      axes.push({
        scale: 'acc',
        side: 1, // right
        label: 'Acc %',
        stroke: AXIS_STROKE,
        grid: { show: false },
        ticks: { stroke: GRID_STROKE, width: 1 },
        font: AXIS_FONT,
        values: (_u: unknown, vals: (number | null)[]) =>
          vals.map(v => v == null ? '' : `${v.toFixed(0)}%`),
      })
    }

    const scales: Record<string, object> = { x: { time: false } }
    if (hasLoss) scales['loss'] = {}
    if (hasLR)   scales['lr']   = {}
    if (hasAccuracy) scales['acc'] = { range: [0, 100] as [number, number] }

    const series: object[] = [{}] // x series
    const tooltipLabels: string[] = []
    const tooltipColors: string[] = []

    if (hasLoss) {
      series.push({ scale: 'loss', label: 'Loss', stroke: chartColor(5), fill: chartColor(5, 0.05), width: 2 })
      tooltipLabels.push('Loss'); tooltipColors.push(chartColor(5))
    }
    if (hasLR) {
      series.push({ scale: 'lr', label: 'LR', stroke: chartColor(1), width: 1.5, dash: [5, 2] })
      tooltipLabels.push('Learning Rate'); tooltipColors.push(chartColor(1))
    }
    if (hasAccuracy) {
      series.push({ scale: 'acc', label: 'Accuracy', stroke: chartColor(3), fill: chartColor(3, 0.05), width: 2 })
      tooltipLabels.push('Accuracy'); tooltipColors.push(chartColor(3))
    }

    return {
      height: HEIGHT,
      scales,
      axes,
      series,
      legend: { show: false },
      plugins: [
        makeTooltipPlugin({
          labels: tooltipLabels,
          colors: tooltipColors,
          formatX: v => `Step ${v}`,
          formatY: (val, i) => {
            const label = tooltipLabels[i] ?? ''
            if (label === 'Learning Rate') return val.toExponential(2)
            if (label === 'Accuracy') return `${val.toFixed(2)}%`
            return val.toFixed(4)
          },
        }),
      ],
      hooks: {
        draw: [
          (u: any) => {
            if (allEpochs.length === 0) return
            const ctx = u.ctx
            ctx.save()
            ctx.strokeStyle = 'rgba(255,255,255,0.2)'
            ctx.lineWidth = 1
            ctx.setLineDash([4, 4])
            for (const ep of allEpochs) {
              const x = Math.round(u.valToPos(ep.step, 'x', true))
              if (x < u.bbox.left || x > u.bbox.left + u.bbox.width) continue
              ctx.beginPath()
              ctx.moveTo(x, u.bbox.top)
              ctx.lineTo(x, u.bbox.top + u.bbox.height)
              ctx.stroke()
              ctx.setLineDash([])
              ctx.font = '9px sans-serif'
              ctx.fillStyle = 'hsl(215 20.2% 65.1%)'
              ctx.fillText(`Epoch ${ep.epoch}`, x + 3, u.bbox.top + 10)
              ctx.setLineDash([4, 4])
            }
            ctx.restore()
          },
        ],
      },
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [hasLoss, hasLR, hasAccuracy, allEpochs])

  const seriesEntries = useMemo(() => {
    const entries: { label: string; color: string }[] = []
    if (hasLoss) entries.push({ label: 'Loss', color: chartColor(5) })
    if (hasLR)   entries.push({ label: 'Learning Rate', color: chartColor(1) })
    if (hasAccuracy) entries.push({ label: 'Accuracy', color: chartColor(3) })
    return entries
  }, [hasLoss, hasLR, hasAccuracy])

  if (!hasLoss && !hasLR && !hasAccuracy && customKeys.length === 0) return null

  return (
    <div className="space-y-4">
      {/* Main chart: Loss / LR / Accuracy */}
      {(hasLoss || hasLR || hasAccuracy) && (
        <div className="chart-container">
          <div className="flex items-center justify-between mb-4">
            <div className="flex items-center gap-2">
              <h2 className="text-lg font-semibold text-foreground">Training Metrics</h2>
              <ChartInfoTooltip content="Loss, learning rate, and accuracy plotted over training steps. Epoch boundaries shown as dashed lines. Drag to zoom." />
            </div>
            {isZoomed && (
              <button
                className="text-xs text-muted-foreground hover:text-foreground px-2 py-0.5 border border-border rounded"
                onClick={() => { setZoomRange(null); setIsZoomed(false) }}
              >
                Reset zoom
              </button>
            )}
          </div>
          <UPlotChart
            ref={uplotRef}
            data={uplotData}
            options={options as any}
            onZoom={handleZoom}
            style={{ height: HEIGHT }}
          />
          <div className="flex flex-wrap justify-center gap-4 mt-3">
            {seriesEntries.map(s => (
              <div key={s.label} className="flex items-center gap-1.5 text-xs text-muted-foreground">
                <span className="w-2.5 h-2.5 rounded-sm inline-block flex-shrink-0" style={{ background: s.color }} />
                {s.label}
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Custom metrics — virtualized, one chart per key */}
      {customSeries.length > 0 && <VirtualizedChartList items={customSeries} />}
    </div>
  )
}
