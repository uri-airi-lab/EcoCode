'use client'

import { useMemo, useRef, useState, useCallback, useEffect } from 'react'
import dynamic from 'next/dynamic'
import type { RefObject } from 'react'
import type { UPlotChartHandle } from './UPlotChart'
import { makeTooltipPlugin, AXIS_STROKE, GRID_STROKE, AXIS_FONT } from './uplot-helpers'
import { MetricRecord } from '@/lib/mock-data'
import { ChartInfoTooltip } from '@/components/ChartInfoTooltip'
import { chartColor } from '@/lib/chart-colors'
import { lttbSample } from '@/lib/lttb'
import { fetchRunRecordsInRange } from '@/lib/api'
import { format } from 'date-fns'

const UPlotChart = dynamic(() => import('./UPlotChart'), { ssr: false })

const LTTB_THRESHOLD = 800
const HEIGHT = 260

// Labels and colors are stable — no GPU breakdowns in this chart
const SERIES_LABELS = ['CPU Power (W)', 'GPU Power (W)', 'Memory Power (W)', 'iGPU Power (W)']
const SERIES_COLORS = [chartColor(1), chartColor(2), chartColor(3), chartColor(4)]

type RecordW = MetricRecord & { cpuW: number; gpuW: number; memW: number; igpuW: number }

function computeWatts(records: MetricRecord[]): RecordW[] {
  return records.map((r, i) => {
    const prev = i > 0 ? records[i - 1] : null
    const deltaS = prev ? Math.max((r.timestamp - prev.timestamp) / 1000, 0.001) : 1
    return {
      ...r,
      cpuW: (r.cpu_energy ?? 0) / deltaS,
      gpuW: (r.gpu_energy ?? 0) / deltaS,
      memW: (r.mem_energy ?? 0) / deltaS,
      igpuW: (r.igpu_energy ?? 0) / deltaS,
    }
  })
}

interface RunData {
  runId: string
  runName: string
  records: MetricRecord[]
}

interface CumulativePowerChartProps {
  runs?: RunData[]
  data?: MetricRecord[]
  runId?: string
}

export function CumulativePowerChart({ runs: runsProp, data, runId }: CumulativePowerChartProps) {
  const uplotRef = useRef<UPlotChartHandle>(null) as RefObject<UPlotChartHandle>
  const [zoomedRecords, setZoomedRecords] = useState<MetricRecord[] | null>(null)
  const [zoomLoading, setZoomLoading] = useState(false)
  const [isZoomed, setIsZoomed] = useState(false)
  const zoomRangeRef = useRef<{ min: number; max: number } | null>(null)

  const runs: RunData[] = runsProp ?? (data ? [{ runId: runId ?? 'single', runName: '', records: data }] : [])
  const sourceRecords = runs[0]?.records ?? []

  useEffect(() => {
    if (!zoomedRecords) uplotRef.current?.resetZoom()
  }, [zoomedRecords])

  // Keep the zoomed snapshot live: append new in-range records as SSE delivers them
  useEffect(() => {
    if (!zoomRangeRef.current) return
    const { max } = zoomRangeRef.current
    setZoomedRecords(prev => {
      if (!prev) return null
      const lastTs = prev[prev.length - 1]?.timestamp ?? 0
      const fresh = sourceRecords.filter(r => r.timestamp > lastTs && r.timestamp <= max)
      return fresh.length > 0 ? [...prev, ...fresh] : prev
    })
  }, [sourceRecords])

  const handleZoom = useCallback(async (min: number, max: number) => {
    setIsZoomed(true)
    zoomRangeRef.current = { min, max }
    if (!runId || !runs[0]?.records.length) return
    const recs = runs[0].records
    const fullRange = recs[recs.length - 1].timestamp - recs[0].timestamp
    if (max - min > fullRange * 0.95) {
      setZoomedRecords(null)
      return
    }
    setZoomLoading(true)
    try {
      const newRecs = await fetchRunRecordsInRange(runId, Math.floor(min), Math.ceil(max), 600)
      setZoomedRecords(newRecs)
    } catch { /* ignore */ }
    finally { setZoomLoading(false) }
  }, [runId, runs])

  const baseDisplayRuns = useMemo(() => {
    const threshold = Math.max(50, Math.floor(LTTB_THRESHOLD / Math.max(1, runs.length)))
    return runs.map(run => {
      const recs = run.records.length > 1 ? run.records.slice(1) : run.records
      const withW = computeWatts(recs)
      return {
        ...run,
        recs: lttbSample(withW, threshold, r => r.cpuW + r.gpuW + r.memW + r.igpuW),
      }
    })
  }, [runs])

  const zoomedDisplayRecs = useMemo(
    () => (zoomedRecords ? computeWatts(zoomedRecords) : null),
    [zoomedRecords]
  )

  const separatorsRef = useRef<{ ts: number; name: string }[]>([])

  // Build columnar data with cumulative stacking: each series is summed from the bottom
  const { uplotData, separators } = useMemo(() => {
    const displayRuns: { runId: string; runName: string; recs: RecordW[] }[] = zoomedDisplayRecs
      ? [{ runId: runs[0]?.runId ?? 'single', runName: '', recs: zoomedDisplayRecs }]
      : baseDisplayRuns

    const ts: number[] = [], cpu: number[] = [], cpuGpu: number[] = []
    const cpuGpuMem: number[] = [], total: number[] = []
    const seps: { ts: number; name: string }[] = []

    displayRuns.forEach((run, idx) => {
      if (idx > 0 && run.recs.length > 0) seps.push({ ts: run.recs[0].timestamp, name: run.runName })
      for (const r of run.recs) {
        const c = Number(r.cpuW.toFixed(2))
        const g = Number(r.gpuW.toFixed(2))
        const m = Number(r.memW.toFixed(2))
        const ig = Number(r.igpuW.toFixed(2))
        ts.push(r.timestamp)
        cpu.push(c)
        cpuGpu.push(Number((c + g).toFixed(2)))
        cpuGpuMem.push(Number((c + g + m).toFixed(2)))
        total.push(Number((c + g + m + ig).toFixed(2)))
      }
    })

    return { uplotData: [ts, cpu, cpuGpu, cpuGpuMem, total], separators: seps }
  }, [baseDisplayRuns, zoomedDisplayRecs, runs])
  separatorsRef.current = separators

  const options = useMemo(() => ({
    height: HEIGHT,
    scales: { x: { time: false } },
    axes: [
      {
        stroke: AXIS_STROKE,
        grid: { stroke: GRID_STROKE, width: 1 },
        ticks: { stroke: GRID_STROKE, width: 1 },
        font: AXIS_FONT,
        values: (_u: unknown, vals: (number | null)[]) =>
          vals.map(v => v == null ? '' : format(new Date(Number(v)), 'HH:mm:ss')),
      },
      {
        label: 'Watts',
        stroke: AXIS_STROKE,
        grid: { stroke: GRID_STROKE, width: 1 },
        ticks: { stroke: GRID_STROKE, width: 1 },
        font: AXIS_FONT,
        values: (_u: unknown, vals: (number | null)[]) =>
          vals.map(v => v == null ? '' : v.toFixed(1)),
      },
    ],
    // Series rendered back-to-front so fills visually stack from bottom
    // Each series value is cumulative, fills from 0 — creates stacked area effect
    series: [
      {},
      { label: 'CPU', stroke: chartColor(1), fill: chartColor(1, 0.5), width: 1 },
      { label: 'CPU+GPU', stroke: chartColor(2), fill: chartColor(2, 0.4), width: 1 },
      { label: 'CPU+GPU+Mem', stroke: chartColor(3), fill: chartColor(3, 0.35), width: 1 },
      { label: 'Total', stroke: chartColor(4), fill: chartColor(4, 0.25), width: 1 },
    ],
    legend: { show: false },
    plugins: [
      makeTooltipPlugin({
        labels: SERIES_LABELS,
        colors: SERIES_COLORS,
        formatX: v => format(new Date(v), 'HH:mm:ss'),
        // Show individual (non-cumulative) values by reading directly from series index
        formatY: (val, i) => {
          // The cumulative series values: we report the increment, not the total
          // ts[0]=cpu, ts[1]=cpu+gpu, ts[2]=cpu+gpu+mem, ts[3]=total
          // This is handled at data-build time — tooltip shows cumulative totals
          return val.toFixed(2)
        },
        unit: ' W',
      }),
    ],
    hooks: {
      draw: [
        (u: any) => {
          const seps = separatorsRef.current
          if (seps.length === 0) return
          const ctx = u.ctx
          ctx.save()
          ctx.strokeStyle = 'rgba(255,255,255,0.2)'
          ctx.lineWidth = 1
          ctx.setLineDash([6, 6])
          for (const sep of seps) {
            const x = Math.round(u.valToPos(sep.ts, 'x', true))
            ctx.beginPath()
            ctx.moveTo(x, u.bbox.top)
            ctx.lineTo(x, u.bbox.top + u.bbox.height)
            ctx.stroke()
          }
          ctx.restore()
        },
      ],
    },
  // separators read via ref — options is stable for the lifetime of the component
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }), [])

  return (
    <div className="chart-container">
      <div className="flex flex-row items-center justify-between mb-4">
        <div className="flex flex-row items-center gap-2">
          <h3 className="text-lg font-semibold text-foreground">Stacked Power Over Time</h3>
          <ChartInfoTooltip content="A stacked area view of instantaneous power draw (Watts) per component. Scroll to zoom, click Reset to restore full view." />
        </div>
        <div className="flex items-center gap-2">
          {zoomLoading && <span className="text-xs text-muted-foreground">Loading...</span>}
          {isZoomed && (
            <button
              className="text-xs text-muted-foreground hover:text-foreground px-2 py-0.5 border border-border rounded"
              onClick={() => { setZoomedRecords(null); setIsZoomed(false); zoomRangeRef.current = null }}
            >
              Reset zoom
            </button>
          )}
        </div>
      </div>
      <UPlotChart
        ref={uplotRef}
        data={uplotData as any}
        options={options as any}
        onZoom={handleZoom}
        style={{ height: HEIGHT }}
      />
      <div className="flex flex-wrap justify-center gap-4 mt-3">
        {SERIES_LABELS.map((label, i) => (
          <div key={label} className="flex items-center gap-1.5 text-xs text-muted-foreground">
            <span className="w-2.5 h-2.5 rounded-sm inline-block flex-shrink-0" style={{ background: SERIES_COLORS[i] }} />
            {label}
          </div>
        ))}
      </div>
    </div>
  )
}
