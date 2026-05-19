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

const DOMAIN_COLORS: Record<string, string> = {
  cpu:  chartColor(1),
  dram: chartColor(3),
  igpu: chartColor(4),
}

const SOCKET_DASH_PATTERNS = [
  undefined,   // Socket 0: solid
  [8, 4],      // Socket 1: long dash
  [4, 2],      // Socket 2: short dash
  [12, 4, 4, 4], // Socket 3: dash-dot
]

interface SocketEnergyChartProps {
  data: MetricRecord[]
  runId?: string
}

export function SocketEnergyChart({ data, runId }: SocketEnergyChartProps) {
  const uplotRef = useRef<UPlotChartHandle>(null) as RefObject<UPlotChartHandle>
  const [zoomedRecords, setZoomedRecords] = useState<MetricRecord[] | null>(null)
  const [zoomLoading, setZoomLoading] = useState(false)
  const [isZoomed, setIsZoomed] = useState(false)
  const zoomRangeRef = useRef<{ min: number; max: number } | null>(null)
  const zoomAbortRef = useRef<AbortController | null>(null)

  // Keep the zoomed snapshot live: append new in-range records as SSE delivers them
  useEffect(() => {
    if (!zoomRangeRef.current) return
    const { max } = zoomRangeRef.current
    setZoomedRecords(prev => {
      if (!prev) return null
      const lastTs = prev[prev.length - 1]?.timestamp ?? 0
      const fresh = data.filter(r => r.timestamp > lastTs && r.timestamp <= max)
      return fresh.length > 0 ? [...prev, ...fresh] : prev
    })
  }, [data])

  // Discover all unique socket+domain combinations, sorted for stable ordering
  const socketKeys = useMemo(() => {
    const keySet = new Set<string>()
    for (const record of data) {
      for (const s of record.socket_details ?? []) {
        keySet.add(`s${s.socket}_${s.domain}`)
      }
    }
    return Array.from(keySet).sort()
  }, [data])

  useEffect(() => {
    if (!isZoomed) uplotRef.current?.resetZoom()
  }, [isZoomed])

  const handleZoom = useCallback(async (min: number, max: number) => {
    setIsZoomed(true)
    zoomRangeRef.current = { min, max }
    if (!runId || !data.length) return
    const fullRange = data[data.length - 1].timestamp - data[0].timestamp
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
  }, [runId, data])

  // Convert energy J → watts using delta-time, then LTTB
  const baseRecs = useMemo(() => {
    const records = data.length > 1 ? data.slice(1) : data
    type RecordW = MetricRecord & { socketW: Record<string, number> }
    const withW: RecordW[] = records.map((r, i) => {
      const prev = i > 0 ? records[i - 1] : null
      const deltaS = prev ? Math.max((r.timestamp - prev.timestamp) / 1000, 0.001) : 1
      const socketW: Record<string, number> = {}
      for (const key of socketKeys) socketW[key] = 0
      for (const s of r.socket_details ?? []) {
        socketW[`s${s.socket}_${s.domain}`] = (s.energy ?? 0) / deltaS
      }
      return { ...r, socketW }
    })
    const threshold = Math.max(50, LTTB_THRESHOLD)
    return lttbSample(withW, threshold, r => Object.values(r.socketW).reduce((a, b) => a + b, 0)) as RecordW[]
  }, [data, socketKeys])

  const zoomedRecs = useMemo(() => {
    if (!zoomedRecords) return null
    type RecordW = MetricRecord & { socketW: Record<string, number> }
    return zoomedRecords.map((r, i) => {
      const prev = i > 0 ? zoomedRecords[i - 1] : null
      const deltaS = prev ? Math.max((r.timestamp - prev.timestamp) / 1000, 0.001) : 1
      const socketW: Record<string, number> = {}
      for (const key of socketKeys) socketW[key] = 0
      for (const s of r.socket_details ?? []) {
        socketW[`s${s.socket}_${s.domain}`] = (s.energy ?? 0) / deltaS
      }
      return { ...r, socketW } as RecordW
    })
  }, [zoomedRecords, socketKeys])

  const uplotData = useMemo(() => {
    const recs = zoomedRecs ?? baseRecs
    const ts: number[] = []
    const seriesCols: number[][] = socketKeys.map(() => [])
    for (const r of recs) {
      ts.push(r.timestamp)
      socketKeys.forEach((key, i) => {
        seriesCols[i].push(Number(((r as any).socketW[key] ?? 0).toFixed(2)))
      })
    }
    return [ts, ...seriesCols]
  }, [baseRecs, zoomedRecs, socketKeys])

  const seriesLabels = useMemo(() =>
    socketKeys.map(key => {
      const parts = key.split('_')
      const socketNum = parseInt(parts[0].replace('s', ''))
      const domain = parts.slice(1).join('_')
      return `Socket ${socketNum} · ${domain.toUpperCase()}`
    }),
    [socketKeys]
  )

  const seriesColors = useMemo(() =>
    socketKeys.map(key => {
      const domain = key.split('_').slice(1).join('_')
      return DOMAIN_COLORS[domain] ?? chartColor(5)
    }),
    [socketKeys]
  )

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
      ...socketKeys.map((key, i) => {
        const socketNum = parseInt(key.split('_')[0].replace('s', ''))
        const dash = SOCKET_DASH_PATTERNS[socketNum % SOCKET_DASH_PATTERNS.length]
        return {
          label: seriesLabels[i],
          stroke: seriesColors[i],
          fill: 'transparent',
          width: 2,
          ...(dash ? { dash } : {}),
        }
      }),
    ],
    legend: { show: false },
    plugins: [
      makeTooltipPlugin({
        labels: seriesLabels,
        colors: seriesColors,
        formatX: v => format(new Date(v), 'HH:mm:ss'),
        unit: ' W',
      }),
    ],
  }), [socketKeys, seriesLabels, seriesColors])

  if (socketKeys.length === 0) return null

  return (
    <div className="chart-container">
      <div className="flex flex-row items-center justify-between mb-4">
        <div className="flex flex-row items-center gap-2">
          <h3 className="text-lg font-semibold text-foreground">Per-Socket Energy (RAPL)</h3>
          <ChartInfoTooltip content="Displays precise Intel RAPL measurements for individual physical CPU sockets and their internal domains (core, uncore, dram). Scroll to zoom, click Reset to restore full view." />
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
        {seriesLabels.map((label, i) => {
          const socketNum = parseInt(socketKeys[i].split('_')[0].replace('s', ''))
          const dash = SOCKET_DASH_PATTERNS[socketNum % SOCKET_DASH_PATTERNS.length]
          return (
            <div key={label} className="flex items-center gap-1.5 text-xs text-muted-foreground">
              <svg width="16" height="10" className="flex-shrink-0">
                <line
                  x1="0" y1="5" x2="16" y2="5"
                  stroke={seriesColors[i]}
                  strokeWidth="2"
                  strokeDasharray={dash ? dash.join(' ') : undefined}
                />
              </svg>
              {label}
            </div>
          )
        })}
      </div>
    </div>
  )
}
