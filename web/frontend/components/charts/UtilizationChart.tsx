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

interface RunData {
  runId: string
  runName: string
  records: MetricRecord[]
}

interface UtilizationChartProps {
  runs?: RunData[]
  data?: MetricRecord[]
  runId?: string
}

export function UtilizationChart({ runs: runsProp, data, runId }: UtilizationChartProps) {
  const uplotRef = useRef<UPlotChartHandle>(null) as RefObject<UPlotChartHandle>
  const [zoomedRecords, setZoomedRecords] = useState<MetricRecord[] | null>(null)
  const [zoomLoading, setZoomLoading] = useState(false)
  const [isZoomed, setIsZoomed] = useState(false)
  const zoomRangeRef = useRef<{ min: number; max: number } | null>(null)
  const zoomAbortRef = useRef<AbortController | null>(null)

  const runs: RunData[] = runsProp ?? (data ? [{ runId: runId ?? 'single', runName: '', records: data }] : [])
  const sourceRecords = runs[0]?.records ?? []

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

  const gpuCount = useMemo(() => {
    let maxIdx = -1
    for (const run of runs) {
      for (const r of run.records) {
        for (const g of r.gpu_details ?? []) {
          if (g.device_index > maxIdx) maxIdx = g.device_index
        }
      }
    }
    return maxIdx + 1
  }, [runs])

  useEffect(() => {
    if (!zoomedRecords) uplotRef.current?.resetZoom()
  }, [zoomedRecords])

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
    zoomAbortRef.current?.abort()
    const controller = new AbortController()
    zoomAbortRef.current = controller
    setZoomLoading(true)
    try {
      const newRecs = await fetchRunRecordsInRange(runId, Math.floor(min), Math.ceil(max), 600)
      if (!controller.signal.aborted) setZoomedRecords(newRecs.length > 0 ? newRecs : null)
    } catch { /* ignore (includes aborted fetches) */ }
    finally { if (!controller.signal.aborted) setZoomLoading(false) }
  }, [runId, runs])

  const baseDisplayRuns = useMemo(() => {
    const threshold = Math.max(50, Math.floor(LTTB_THRESHOLD / Math.max(1, runs.length)))
    return runs.map(run => ({
      ...run,
      recs: lttbSample(run.records, threshold, r => r.cpu_usage + r.gpu_usage),
    }))
  }, [runs])

  const separatorsRef = useRef<{ ts: number; name: string }[]>([])

  const { uplotData, separators } = useMemo(() => {
    const displayRuns: { runId: string; runName: string; recs: MetricRecord[] }[] = zoomedRecords
      ? [{ runId: runs[0]?.runId ?? 'single', runName: '', recs: zoomedRecords }]
      : baseDisplayRuns

    const ts: number[] = [], cpu: number[] = [], gpu: number[] = [], mem: number[] = []
    const gpuDetails: number[][] = Array.from({ length: gpuCount }, () => [])
    const seps: { ts: number; name: string }[] = []

    displayRuns.forEach((run, idx) => {
      if (idx > 0 && run.recs.length > 0) seps.push({ ts: run.recs[0].timestamp, name: run.runName })
      for (const r of run.recs) {
        ts.push(r.timestamp)
        cpu.push(Number((r.cpu_usage ?? 0).toFixed(2)))
        gpu.push(Number((r.gpu_usage ?? 0).toFixed(2)))
        mem.push(Number((r.mem_usage ?? 0).toFixed(2)))
        for (let i = 0; i < gpuCount; i++) {
          const g = r.gpu_details?.find(gd => gd.device_index === i)
          gpuDetails[i].push(Number((g?.gpu_usage ?? 0).toFixed(2)))
        }
      }
    })

    return { uplotData: [ts, cpu, gpu, mem, ...gpuDetails], separators: seps }
  }, [baseDisplayRuns, zoomedRecords, gpuCount, runs])
  separatorsRef.current = separators

  const seriesLabels = useMemo(() => {
    const base = ['CPU Usage', gpuCount > 1 ? 'GPU Usage (Avg)' : 'GPU Usage', 'Memory Usage']
    if (gpuCount > 1) {
      for (let i = 0; i < gpuCount; i++) base.push(`GPU ${i}`)
    }
    return base
  }, [gpuCount])

  const seriesColors = useMemo(() => {
    const base = [chartColor(1), chartColor(2), chartColor(3)]
    if (gpuCount > 1) {
      for (let i = 0; i < gpuCount; i++) base.push(chartColor((i % 6) + 1))
    }
    return base
  }, [gpuCount])

  const options = useMemo(() => ({
    height: HEIGHT,
    scales: {
      x: { time: false },
      y: { range: [0, 100] as [number, number] }, // clamp 0-100%
    },
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
        label: 'Usage %',
        stroke: AXIS_STROKE,
        grid: { stroke: GRID_STROKE, width: 1 },
        ticks: { stroke: GRID_STROKE, width: 1 },
        font: AXIS_FONT,
        values: (_u: unknown, vals: (number | null)[]) =>
          vals.map(v => v == null ? '' : `${v.toFixed(0)}%`),
      },
    ],
    series: [
      {},
      { label: 'CPU', stroke: chartColor(1), fill: chartColor(1, 0.1), width: 2 },
      { label: 'GPU', stroke: chartColor(2), fill: chartColor(2, 0.1), width: 2 },
      { label: 'Mem', stroke: chartColor(3), fill: chartColor(3, 0.1), width: 2 },
      ...Array.from({ length: gpuCount > 1 ? gpuCount : 0 }, (_, i) => ({
        label: `GPU ${i}`,
        stroke: chartColor((i % 6) + 1),
        width: 1,
        dash: [4, 4],
      })),
    ],
    legend: { show: false },
    plugins: [
      makeTooltipPlugin({
        labels: seriesLabels,
        colors: seriesColors,
        formatX: v => format(new Date(v), 'HH:mm:ss'),
        unit: '%',
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
            if (x < u.bbox.left || x > u.bbox.left + u.bbox.width) continue
            ctx.beginPath()
            ctx.moveTo(x, u.bbox.top)
            ctx.lineTo(x, u.bbox.top + u.bbox.height)
            ctx.stroke()
          }
          ctx.restore()
        },
      ],
    },
  // separators read via ref — options only recreates when series structure changes
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }), [gpuCount, seriesLabels, seriesColors])

  return (
    <div className="chart-container">
      <div className="flex flex-row items-center justify-between mb-4">
        <div className="flex flex-row items-center gap-2">
          <h3 className="text-lg font-semibold text-foreground">Resource Utilization</h3>
          <ChartInfoTooltip content="Tracks the percentage of CPU, GPU, and RAM being actively used. Scroll to zoom, click Reset to restore full view." />
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
        {seriesLabels.map((label, i) => (
          <div key={label} className="flex items-center gap-1.5 text-xs text-muted-foreground">
            <span className="w-2.5 h-2.5 rounded-sm inline-block flex-shrink-0" style={{ background: seriesColors[i] }} />
            {label}
          </div>
        ))}
      </div>
    </div>
  )
}
