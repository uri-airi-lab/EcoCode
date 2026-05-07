'use client'

/**
 * VirtualizedChartList — renders one uPlot chart per custom metric,
 * but only mounts charts that are within (or near) the viewport.
 *
 * Off-screen charts are unmounted entirely, keeping DOM count low
 * even when a run logs dozens of custom metrics.
 */

import { useRef, useMemo } from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'
import dynamic from 'next/dynamic'
import { makeTooltipPlugin, AXIS_STROKE, GRID_STROKE, AXIS_FONT } from './uplot-helpers'
import { chartColor } from '@/lib/chart-colors'

const UPlotChart = dynamic(() => import('./UPlotChart'), { ssr: false })

export interface MetricSeries {
  key: string
  /** x values (training steps) */
  steps: number[]
  /** y values corresponding to each step */
  values: number[]
}

interface VirtualizedChartListProps {
  items: MetricSeries[]
  /** Height of each chart card in pixels. Defaults to 220. */
  itemHeight?: number
}

const CHART_HEIGHT = 180

function SingleMetricChart({ item }: { item: MetricSeries }) {
  const color = chartColor(1)

  const uplotData = useMemo(
    () => [item.steps, item.values] as any,
    [item.steps, item.values]
  )

  const options = useMemo(() => ({
    height: CHART_HEIGHT,
    scales: { x: { time: false } },
    axes: [
      {
        stroke: AXIS_STROKE,
        grid: { stroke: GRID_STROKE, width: 1 },
        ticks: { stroke: GRID_STROKE, width: 1 },
        font: AXIS_FONT,
        values: (_u: unknown, vals: (number | null)[]) =>
          vals.map(v => v == null ? '' : `Step ${v}`),
      },
      {
        stroke: AXIS_STROKE,
        grid: { stroke: GRID_STROKE, width: 1 },
        ticks: { stroke: GRID_STROKE, width: 1 },
        font: AXIS_FONT,
        values: (_u: unknown, vals: (number | null)[]) =>
          vals.map(v => v == null ? '' : v.toFixed(4)),
      },
    ],
    series: [
      {},
      { label: item.key, stroke: color, fill: chartColor(1, 0.08), width: 1.5 },
    ],
    legend: { show: false },
    plugins: [
      makeTooltipPlugin({
        labels: [item.key],
        colors: [color],
        formatX: v => `Step ${v}`,
        formatY: v => v.toFixed(4),
      }),
    ],
  // key in deps ensures options recreate when metric key changes
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }), [item.key])

  return (
    <div className="chart-container">
      <h4 className="text-sm font-medium text-foreground mb-3">{item.key}</h4>
      <UPlotChart data={uplotData} options={options as any} style={{ height: CHART_HEIGHT }} />
    </div>
  )
}

export function VirtualizedChartList({ items, itemHeight = CHART_HEIGHT + 80 }: VirtualizedChartListProps) {
  const parentRef = useRef<HTMLDivElement>(null)

  const virtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => itemHeight,
    overscan: 1, // mount 1 extra item above/below viewport
  })

  if (items.length === 0) return null

  return (
    <div className="space-y-2">
      <h3 className="text-base font-semibold text-foreground">Custom Metrics</h3>
      {/* Scrollable window; height = ~3 cards */}
      <div
        ref={parentRef}
        style={{ height: Math.min(items.length * itemHeight, itemHeight * 3 + 20), overflowY: 'auto' }}
        className="rounded-lg"
      >
        {/* Total height container so scroll thumb is correctly sized */}
        <div style={{ height: virtualizer.getTotalSize(), position: 'relative' }}>
          {virtualizer.getVirtualItems().map(vItem => (
            <div
              key={vItem.key}
              style={{
                position: 'absolute',
                top: 0,
                left: 0,
                width: '100%',
                height: vItem.size,
                transform: `translateY(${vItem.start}px)`,
                padding: '0 0 8px 0',
              }}
            >
              <SingleMetricChart item={items[vItem.index]} />
            </div>
          ))}
        </div>
      </div>
    </div>
  )
}
