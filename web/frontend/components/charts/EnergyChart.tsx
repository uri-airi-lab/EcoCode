'use client'

import { useMemo, useRef, useState, useEffect } from 'react'
import {
  LineChart,
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  Legend,
  ReferenceLine,
} from 'recharts'
import { MetricRecord } from '@/lib/mock-data'
import { format } from 'date-fns'
import { ChartInfoTooltip } from '@/components/ChartInfoTooltip'
import { chartColor } from '@/lib/chart-colors'

const GPU_COLORS = [1, 2, 3, 4, 5, 6].map(n => chartColor(n as 1 | 2 | 3 | 4 | 5 | 6))

const PX_PER_POINT = 12
const MIN_WIDTH = 600

interface RunData {
  runId: string
  runName: string
  records: MetricRecord[]
}

interface EnergyChartProps {
  runs?: RunData[]
  data?: MetricRecord[]
}

// Custom label rendered inside the ReferenceLine
function SeparatorLabel({ viewBox, runName }: { viewBox?: { x?: number; y?: number; height?: number }; runName: string }) {
  const x = (viewBox?.x ?? 0) + 6
  const y = (viewBox?.y ?? 0) + 14
  return (
    <g>
      <text x={x} y={y} fill="hsl(var(--muted-foreground))" fontSize={10} fontWeight={500}>
        {runName}
      </text>
    </g>
  )
}

export function EnergyChart({ runs: runsProp, data }: EnergyChartProps) {
  const scrollRef = useRef<HTMLDivElement>(null)
  const [visibleRange, setVisibleRange] = useState({ start: 0, end: 100 })

  // Normalize: accept either runs[] (multi-run dashboard) or data[] (single-run detail)
  const runs: RunData[] = runsProp ?? (data ? [{ runId: 'single', runName: '', records: data }] : [])

  const gpuCount = useMemo(() => {
    let maxIdx = -1
    for (const run of runs) {
      for (const record of run.records) {
        for (const g of record.gpu_details ?? []) {
          if (g.device_index > maxIdx) maxIdx = g.device_index
        }
      }
    }
    return maxIdx + 1
  }, [runs])

  // Build flat chart data; track separator indices (index of first point of each new run after the first)
  const { chartData, separatorIndices } = useMemo(() => {
    const data: Record<string, string | number | null>[] = []
    // { index: number, runName: string } — index is the first point of the *next* run
    const seps: { index: number; runName: string }[] = []

    runs.forEach((run, runIdx) => {
      const records = run.records.length > 1 ? run.records.slice(1) : run.records

      if (runIdx > 0 && data.length > 0) {
        // Record the index where this run starts
        seps.push({ index: data.length, runName: run.runName })
      }

      records.forEach((record, index) => {
        const prev = index > 0
          ? records[index - 1]
          : (run.records.length > 1 ? run.records[0] : null)
        const deltaMs = prev ? record.timestamp - prev.timestamp : 1000
        const deltaS = (deltaMs / 1000) || 1

        const point: Record<string, string | number | null> = {
          time: format(new Date(record.timestamp), 'HH:mm:ss'),
          index: data.length,
          cpu: Number(((record.cpu_energy ?? 0) / deltaS).toFixed(2)),
          gpu: Number(((record.gpu_energy ?? 0) / deltaS).toFixed(2)),
          mem: Number(((record.mem_energy ?? 0) / deltaS).toFixed(2)),
          igpu: Number(((record.igpu_energy ?? 0) / deltaS).toFixed(2)),
          total: Number((
            ((record.cpu_energy ?? 0) + (record.gpu_energy ?? 0) +
              (record.mem_energy ?? 0) + (record.igpu_energy ?? 0)) / deltaS
          ).toFixed(2)),
        }

        for (let i = 0; i < gpuCount; i++) point[`gpu${i}`] = 0
        for (const g of record.gpu_details ?? []) {
          point[`gpu${g.device_index}`] = Number(((g.gpu_energy ?? 0) / deltaS).toFixed(2))
        }

        data.push(point)
      })
    })

    return { chartData: data, separatorIndices: seps }
  }, [runs, gpuCount])

  const chartWidth = Math.max(MIN_WIDTH, chartData.length * PX_PER_POINT)

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
      
      const start = Math.floor(startPercent * chartData.length)
      const end = Math.ceil(endPercent * chartData.length)
      
      setVisibleRange({ start, end })
    }

    const container = scrollRef.current
    if (container) {
      handleScroll() // Initial call
      container.addEventListener('scroll', handleScroll)
      return () => container.removeEventListener('scroll', handleScroll)
    }
  }, [chartData.length])

  // Calculate visible tick positions
  const visibleTicks = useMemo(() => {
    const tickCount = 8
    const step = Math.max(1, Math.floor((visibleRange.end - visibleRange.start) / tickCount))
    const ticks = []
    for (let i = visibleRange.start; i < visibleRange.end; i += step) {
      if (i < chartData.length) {
        ticks.push(i)
      }
    }
    return ticks
  }, [visibleRange, chartData.length])

  // Resolve border color for CartesianGrid
  const borderColor = typeof window !== 'undefined'
    ? getComputedStyle(document.documentElement).getPropertyValue('--border').trim()
    : '0 0% 24%'
  const gridStroke = `hsla(${borderColor}, 0.6)`

  return (
    <div className="chart-container">
      <div className="flex flex-row items-center gap-2 mb-4">
        <h3 className="text-lg font-semibold text-foreground">
          Energy Consumption Over Time
        </h3>
        <ChartInfoTooltip content="Shows a timeline of the raw power (Watts) being drawn by your hardware components. All runs are shown in sequence with vertical separators between them." />
      </div>
      <div className="relative">
        <div ref={scrollRef} className="overflow-x-auto">
          <LineChart width={chartWidth} height={320} data={chartData}>
            <CartesianGrid strokeDasharray="3 3" stroke={gridStroke} strokeOpacity={0.8} />
            <XAxis
              dataKey="index"
              type="number"
              domain={[0, chartData.length - 1]}
              ticks={visibleTicks}
              tickFormatter={(i) => {
                const d = chartData[i]
                return d ? String(d.time) : ''
              }}
              stroke="hsl(var(--border))"
              tick={{ fontSize: 10, fill: 'hsl(var(--muted-foreground))' }}
            />
            <YAxis
              stroke="hsl(var(--border))"
              tick={{ fontSize: 12, fill: 'hsl(var(--muted-foreground))' }}
              label={{ value: 'Watts', angle: -90, position: 'insideLeft' }}
            />
            <Tooltip
              labelFormatter={(i) => {
                const d = chartData[Number(i)]
                return d ? String(d.time) : String(i)
              }}
              contentStyle={{
                backgroundColor: 'hsl(var(--card))',
                border: '1px solid hsl(var(--border))',
                borderRadius: '4px',
                fontSize: '12px',
                color: 'hsl(var(--foreground))',
              }}
              itemStyle={{ padding: '2px 0' }}
              cursor={{ stroke: 'hsl(var(--muted-foreground))' }}
            />
            <Legend />

            {/* Run boundary separators */}
            {separatorIndices.map(({ index, runName }) => (
              <ReferenceLine
                key={`sep-${index}`}
                x={index}
                stroke="hsl(var(--muted-foreground))"
                strokeWidth={2}
                strokeDasharray="6 3"
                label={<SeparatorLabel runName={runName} />}
              />
            ))}

            <Line type="monotone" dataKey="cpu" stroke={chartColor(1)} strokeWidth={2} dot={false} name="CPU Power (W)" isAnimationActive={false} connectNulls />
            <Line type="monotone" dataKey="gpu" stroke={chartColor(2)} strokeWidth={2} dot={false} name="GPU Power (W) (Total)" isAnimationActive={false} connectNulls />
            {gpuCount > 1 && Array.from({ length: gpuCount }, (_, i) => (
              <Line key={`gpu${i}`} type="monotone" dataKey={`gpu${i}`} stroke={GPU_COLORS[i % GPU_COLORS.length]} strokeWidth={1} strokeDasharray="4 2" dot={false} name={`GPU ${i}`} isAnimationActive={false} connectNulls />
            ))}
            <Line type="monotone" dataKey="mem" stroke={chartColor(3)} strokeWidth={2} dot={false} name="Memory Power (W)" isAnimationActive={false} connectNulls />
            <Line type="monotone" dataKey="igpu" stroke={chartColor(4)} strokeWidth={2} dot={false} name="iGPU Power (W)" isAnimationActive={false} connectNulls />
          </LineChart>
        </div>
      </div>
    </div>
  )
}
