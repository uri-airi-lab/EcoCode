'use client'

import React, { useMemo, useRef } from 'react'
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
import { format } from 'date-fns'
import { ChartInfoTooltip } from './ChartInfoTooltip'
import { chartColor } from '@/lib/chart-colors'

const PX_PER_POINT = 12
const MIN_WIDTH = 600

interface RunData {
  runId: string
  runName: string
  records: MetricRecord[]
}

interface CumulativePowerChartProps {
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

export function CumulativePowerChart({ runs: runsProp, data }: CumulativePowerChartProps) {
  const scrollRef = useRef<HTMLDivElement>(null)

  // Normalize: accept either runs[] (multi-run dashboard) or data[] (single-run detail)
  const runs: RunData[] = runsProp ?? (data ? [{ runId: 'single', runName: '', records: data }] : [])

  const { chartData, separatorIndices } = useMemo(() => {
    const data: Record<string, string | number>[] = []
    const seps: { index: number; runName: string }[] = []

    runs.forEach((run, runIdx) => {
      const records = run.records.length > 1 ? run.records.slice(1) : run.records

      if (runIdx > 0 && data.length > 0) {
        seps.push({ index: data.length, runName: run.runName })
      }

      records.forEach((record, index) => {
        const prev = index > 0
          ? records[index - 1]
          : (run.records.length > 1 ? run.records[0] : null)
        const deltaMs = prev ? record.timestamp - prev.timestamp : 1000
        const deltaS = (deltaMs / 1000) || 1

        data.push({
          time: format(new Date(record.timestamp), 'HH:mm:ss'),
          index: data.length,
          cpu: Number(((record.cpu_energy ?? 0) / deltaS).toFixed(2)),
          gpu: Number(((record.gpu_energy ?? 0) / deltaS).toFixed(2)),
          mem: Number(((record.mem_energy ?? 0) / deltaS).toFixed(2)),
          igpu: Number(((record.igpu_energy ?? 0) / deltaS).toFixed(2)),
        })
      })
    })

    return { chartData: data, separatorIndices: seps }
  }, [runs])

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
          Total System Power Stack
        </h3>
        <ChartInfoTooltip content="A stacked area view of power consumption across all runs. Vertical markers separate individual runs." />
      </div>
      <div ref={scrollRef} className="overflow-x-auto">
        <AreaChart width={chartWidth} height={300} data={chartData}>
          <CartesianGrid strokeDasharray="3 3" stroke={gridStroke} />
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
            tick={{ fontSize: 10, fill: 'hsl(var(--muted-foreground))' }}
            label={{ value: 'Watts', angle: -90, position: 'insideLeft', style: { fill: 'hsl(var(--muted-foreground))', fontSize: '10px' } }}
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

          <Area type="monotone" dataKey="cpu" stackId="1" stroke={chartColor(1)} fill={chartColor(1)} fillOpacity={0.6} name="CPU Power (W)" isAnimationActive={false} />
          <Area type="monotone" dataKey="gpu" stackId="1" stroke={chartColor(2)} fill={chartColor(2)} fillOpacity={0.6} name="GPU Power (W)" isAnimationActive={false} />
          <Area type="monotone" dataKey="mem" stackId="1" stroke={chartColor(3)} fill={chartColor(3)} fillOpacity={0.6} name="Memory Power (W)" isAnimationActive={false} />
          <Area type="monotone" dataKey="igpu" stackId="1" stroke={chartColor(4)} fill={chartColor(4)} fillOpacity={0.6} name="iGPU Power (W)" isAnimationActive={false} />
        </AreaChart>
      </div>
    </div>
  )
}
