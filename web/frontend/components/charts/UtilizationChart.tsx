'use client'

import { useMemo, useRef } from 'react'
import {
  AreaChart,
  Area,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  Legend,
  ReferenceLine,
} from 'recharts'
import { MetricRecord } from '@/lib/mock-data'
import { ChartInfoTooltip } from '@/components/ChartInfoTooltip'
import { formatChartTime, minIntervalMs } from '@/lib/utils'
import { chartColor } from '@/lib/chart-colors'

const GPU_COLORS = [1, 2, 3, 4, 5, 6].map(n => chartColor(n as 1 | 2 | 3 | 4 | 5 | 6))

const PX_PER_POINT = 12
const MIN_WIDTH = 600

interface RunData {
  runId: string
  runName: string
  records: MetricRecord[]
}

interface UtilizationChartProps {
  runs?: RunData[]
  data?: MetricRecord[]
}

function SeparatorLabel({ viewBox, runName }: { viewBox?: { x?: number; y?: number }; runName: string }) {
  const x = (viewBox?.x ?? 0) + 6
  const y = (viewBox?.y ?? 0) + 14
  return (
    <text x={x} y={y} fill="hsl(var(--muted-foreground))" fontSize={10} fontWeight={500}>
      {runName}
    </text>
  )
}

export function UtilizationChart({ runs: runsProp, data }: UtilizationChartProps) {
  const scrollRef = useRef<HTMLDivElement>(null)

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

  const { chartData, separatorIndices } = useMemo(() => {
    const data: Record<string, string | number>[] = []
    const seps: { index: number; runName: string }[] = []

    const allTimestamps = runs.flatMap(r => r.records.map(rec => rec.timestamp))
    const intervalMs = minIntervalMs(allTimestamps)

    runs.forEach((run, runIdx) => {
      if (runIdx > 0 && data.length > 0) {
        seps.push({ index: data.length, runName: run.runName })
      }

      run.records.forEach((record) => {
        const point: Record<string, string | number> = {
          time: formatChartTime(record.timestamp, intervalMs),
          index: data.length,
          cpu: Number((record.cpu_usage ?? 0).toFixed(2)),
          gpu: Number((record.gpu_usage ?? 0).toFixed(2)),
          mem: Number((record.mem_usage ?? 0).toFixed(2)),
        }

        for (let i = 0; i < gpuCount; i++) point[`gpu${i}`] = 0
        for (const g of record.gpu_details ?? []) {
          point[`gpu${g.device_index}`] = Number((g.gpu_usage ?? 0).toFixed(2))
        }

        data.push(point)
      })
    })

    return { chartData: data, separatorIndices: seps }
  }, [runs, gpuCount])

  const chartWidth = Math.max(MIN_WIDTH, chartData.length * PX_PER_POINT)

  // Resolve border color for CartesianGrid
  const borderColor = typeof window !== 'undefined'
    ? getComputedStyle(document.documentElement).getPropertyValue('--border').trim()
    : '0 0% 24%'
  const gridStroke = `hsla(${borderColor}, 0.6)`

  return (
    <div className="chart-container">
      <div className="flex flex-row items-center gap-2 mb-4">
        <h3 className="text-lg font-semibold text-foreground">
          Resource Utilization
        </h3>
        <ChartInfoTooltip content="Tracks the percentage of CPU, GPU, and RAM being actively used across all runs. Vertical markers separate individual runs." />
      </div>
      <div ref={scrollRef} className="overflow-x-auto">
        <AreaChart width={chartWidth} height={300} data={chartData}>
          <defs>
            <linearGradient id="colorCpu" x1="0" y1="0" x2="0" y2="1">
              <stop offset="5%" stopColor={chartColor(1)} stopOpacity={0.1} />
              <stop offset="95%" stopColor={chartColor(1)} stopOpacity={0} />
            </linearGradient>
            <linearGradient id="colorGpu" x1="0" y1="0" x2="0" y2="1">
              <stop offset="5%" stopColor={chartColor(2)} stopOpacity={0.1} />
              <stop offset="95%" stopColor={chartColor(2)} stopOpacity={0} />
            </linearGradient>
            <linearGradient id="colorMem" x1="0" y1="0" x2="0" y2="1">
              <stop offset="5%" stopColor={chartColor(3)} stopOpacity={0.1} />
              <stop offset="95%" stopColor={chartColor(3)} stopOpacity={0} />
            </linearGradient>
          </defs>
          <CartesianGrid strokeDasharray="3 3" stroke={gridStroke} strokeOpacity={0.8} />
          <XAxis
            dataKey="index"
            type="number"
            domain={[0, chartData.length - 1]}
            tickFormatter={(i) => {
              const d = chartData[i]
              return d ? String(d.time) : ''
            }}
            stroke="hsl(var(--border))"
            tick={{ fontSize: 10, fill: 'hsl(var(--muted-foreground))' }}
            interval={Math.max(0, Math.floor(chartData.length / 16) - 1)}
          />
          <YAxis
            stroke="hsl(var(--border))"
            tick={{ fontSize: 12, fill: 'hsl(var(--muted-foreground))' }}
            label={{ value: 'Usage %', angle: -90, position: 'insideLeft' }}
            domain={[0, 100]}
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

          <Area type="monotone" dataKey="cpu" stroke={chartColor(1)} fillOpacity={1} fill="url(#colorCpu)" name="CPU Usage" isAnimationActive={false} connectNulls />
          <Area type="monotone" dataKey="gpu" stroke={chartColor(2)} fillOpacity={1} fill="url(#colorGpu)" name={gpuCount > 1 ? 'GPU Usage (Avg)' : 'GPU Usage'} isAnimationActive={false} connectNulls />
          {gpuCount > 1 && Array.from({ length: gpuCount }, (_, i) => (
            <Area key={`gpu${i}`} type="monotone" dataKey={`gpu${i}`} stroke={GPU_COLORS[i % GPU_COLORS.length]} strokeDasharray="4 2" fillOpacity={0} fill="transparent" name={`GPU ${i}`} isAnimationActive={false} connectNulls />
          ))}
          <Area type="monotone" dataKey="mem" stroke={chartColor(3)} fillOpacity={1} fill="url(#colorMem)" name="Memory Usage" isAnimationActive={false} connectNulls />
        </AreaChart>
      </div>
    </div>
  )
}
