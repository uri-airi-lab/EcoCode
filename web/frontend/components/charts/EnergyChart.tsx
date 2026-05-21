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

// Dynamic import prevents uPlot from executing on the server (it needs document)
const UPlotChart = dynamic(() => import('./UPlotChart'), { ssr: false })

const LTTB_THRESHOLD = 800
const HEIGHT = 260

type RecordW = MetricRecord & {
  cpuW: number
  gpuW: number
  memW: number
  igpuW: number
  gpuDetailW: { device_index: number; watts: number }[]
}

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
      gpuDetailW: (r.gpu_details ?? []).map(g => ({
        device_index: g.device_index,
        watts: (g.gpu_energy ?? 0) / deltaS,
      })),
    }
  })
}

interface RunData {
  runId: string
  runName: string
  records: MetricRecord[]
}

interface EnergyChartProps {
  runs?: RunData[]
  data?: MetricRecord[]
  runId?: string
}

export function EnergyChart({ runs: runsProp, data, runId }: EnergyChartProps) {
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

  // When zoom is cleared, reset scale after the child's setData has already run.
  // React runs child effects before parent effects, so u.data[0] is the full base
  // dataset by the time this fires — resetZoom reads the correct full range.
  useEffect(() => {
    if (!isZoomed) uplotRef.current?.resetZoom()
  }, [isZoomed])

  // UPlotChart debounces and fires onZoom after scroll settles; no inner debounce needed.
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
    // Cancel any in-flight zoom fetch before starting a new one.
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

  // Precompute watts + LTTB for the base (unzoomed) view
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

  // separatorsRef lets the draw hook read the latest separators without being
  // captured in the options useMemo closure — keeps options stable across record updates.
  const separatorsRef = useRef<{ ts: number; name: string }[]>([])

  // Build uPlot columnar data — avoids per-point object allocation
  const { uplotData, separators } = useMemo(() => {
    const displayRuns: { runId: string; runName: string; recs: RecordW[] }[] = zoomedDisplayRecs
      ? [{ runId: runs[0]?.runId ?? 'single', runName: '', recs: zoomedDisplayRecs }]
      : baseDisplayRuns

    const ts: number[] = [], cpu: number[] = [], gpu: number[] = []
    const mem: number[] = [], igpu: number[] = []
    const gpuDetails: number[][] = Array.from({ length: gpuCount }, () => [])
    const seps: { ts: number; name: string }[] = []

    displayRuns.forEach((run, idx) => {
      if (idx > 0 && run.recs.length > 0) seps.push({ ts: run.recs[0].timestamp, name: run.runName })
      for (const r of run.recs) {
        ts.push(r.timestamp)
        cpu.push(Number(r.cpuW.toFixed(2)))
        gpu.push(Number(r.gpuW.toFixed(2)))
        mem.push(Number(r.memW.toFixed(2)))
        igpu.push(Number(r.igpuW.toFixed(2)))
        for (let i = 0; i < gpuCount; i++) {
          const g = r.gpuDetailW.find(gd => gd.device_index === i)
          gpuDetails[i].push(Number((g?.watts ?? 0).toFixed(2)))
        }
      }
    })

    const uplotData: number[][] = [ts, cpu, gpu, mem, igpu, ...gpuDetails]
    return { uplotData, separators: seps }
  }, [baseDisplayRuns, zoomedDisplayRecs, gpuCount, runs])
  separatorsRef.current = separators

  const seriesLabels = useMemo(() => {
    const base = ['CPU Power (W)', gpuCount > 1 ? 'GPU Power (W) (Total)' : 'GPU Power (W)', 'Memory Power (W)', 'iGPU Power (W)']
    if (gpuCount > 0) {
      for (let i = 0; i < gpuCount; i++) base.push(`GPU ${i}`)
    }
    return base
  }, [gpuCount])

  const seriesColors = useMemo(() => {
    const base = [chartColor(1), chartColor(2), chartColor(3), chartColor(4)]
    if (gpuCount > 0) {
      for (let i = 0; i < gpuCount; i++) base.push(chartColor((i % 6) + 1))
    }
    return base
  }, [gpuCount])

  // Stable options object — only recreates when separators or series count changes
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
    series: [
      {},
      { label: 'CPU', stroke: chartColor(1), fill: chartColor(1, 0.05), width: 2 },
      { label: 'GPU', stroke: chartColor(2), fill: chartColor(2, 0.05), width: 2 },
      { label: 'Mem', stroke: chartColor(3), fill: chartColor(3, 0.05), width: 2 },
      { label: 'iGPU', stroke: chartColor(4), fill: chartColor(4, 0.05), width: 2 },
      ...Array.from({ length: gpuCount }, (_, i) => ({
        label: `GPU ${i}`,
        stroke: chartColor((i % 6) + 1),
        width: 1,
        dash: [4, 4],
      })),
    ],
    legend: { show: false }, // we render our own static legend
    plugins: [
      makeTooltipPlugin({
        labels: seriesLabels,
        colors: seriesColors,
        formatX: v => format(new Date(v), 'HH:mm:ss'),
        unit: ' W',
      }),
    ],
    hooks: {
      draw: [
        (u: any) => {
          // Read via ref — not a closure capture — so this hook never forces options to recreate
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
            if (sep.name) {
              ctx.setLineDash([])
              ctx.font = '10px sans-serif'
              ctx.fillStyle = 'hsl(220 5% 60%)'
              ctx.fillText(sep.name, x + 4, u.bbox.top + 12)
              ctx.setLineDash([6, 6])
            }
          }
          ctx.restore()
        },
      ],
    },
  // separators intentionally excluded — read via separatorsRef to keep options stable
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }), [gpuCount, seriesLabels, seriesColors])

  return (
    <div className="chart-container">
      <div className="flex flex-row items-center justify-between mb-4">
        <div className="flex flex-row items-center gap-2">
          <h3 className="text-lg font-semibold text-foreground">Energy Consumption Over Time</h3>
          <ChartInfoTooltip content="Shows a timeline of the raw power (Watts) drawn by your hardware. Scroll to zoom, click Reset to restore full view." />
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
      {/* Static legend — does not re-render on cursor movement */}
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
