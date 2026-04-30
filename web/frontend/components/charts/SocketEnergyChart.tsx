'use client'

import { useMemo } from 'react'
import {
  LineChart,
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  Legend,
  ResponsiveContainer,
  Brush,
} from 'recharts'
import { MetricRecord } from '@/lib/mock-data'
import { ChartInfoTooltip } from '@/components/ChartInfoTooltip'
import { formatChartTime, minIntervalMs } from '@/lib/utils'
import { chartColor } from '@/lib/chart-colors'

const SOCKET_COLORS: Record<string, string> = {
  cpu:  chartColor(1),  // cyan
  dram: chartColor(3),  // green
  igpu: chartColor(4),  // purple
}

const SOCKET_LINE_STYLES = [
  '',         // Socket 0: solid
  '8 4',     // Socket 1: dashed
  '4 2',     // Socket 2: short dash
  '12 4 4 4', // Socket 3: dash-dot
]

interface SocketEnergyChartProps {
  data: MetricRecord[]
}

export function SocketEnergyChart({ data }: SocketEnergyChartProps) {
  // Discover all unique socket+domain combinations
  const socketKeys = useMemo(() => {
    const keySet = new Set<string>()
    for (const record of data) {
      for (const s of record.socket_details ?? []) {
        keySet.add(`s${s.socket}_${s.domain}`)
      }
    }
    return Array.from(keySet).sort()
  }, [data])

  const chartData = useMemo(() => {
    const records = data
    const intervalMs = minIntervalMs(data.map(r => r.timestamp))

    return records.map((record) => {
      const point: Record<string, string | number> = {
        time: formatChartTime(record.timestamp, intervalMs),
      }

      // Pre-fill all known socket keys with 0 to prevent gaps
      for (const key of socketKeys) {
        point[key] = 0
      }

      for (const s of record.socket_details ?? []) {
        point[`s${s.socket}_${s.domain}`] = Number((s.energy ?? 0).toFixed(2))
      }

      return point
    })
  }, [data])

  if (socketKeys.length === 0) {
    return null // Don't render if no socket data
  }

  return (
    <div className="chart-container">
      <div className="flex flex-row items-center gap-2 mb-4">
        <h3 className="text-lg font-semibold text-foreground">
          Per-Socket Energy (RAPL)
        </h3>
        <ChartInfoTooltip content="Displays precise Intel RAPL measurements for individual physical CPU sockets and their internal domains (core, uncore, dram)." />
      </div>
      <ResponsiveContainer width="100%" height={300}>
        <LineChart data={chartData}>
          <CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--border))" strokeOpacity={0.8} />
          <XAxis
            dataKey="time"
            stroke="hsl(var(--border))"
            tick={{ fontSize: 12, fill: 'hsl(var(--muted-foreground))' }}
          />
          <YAxis
            stroke="hsl(var(--border))"
            tick={{ fontSize: 12, fill: 'hsl(var(--muted-foreground))' }}
            label={{ value: 'Watts', angle: -90, position: 'insideLeft' }}
          />
          <Tooltip
            contentStyle={{
              backgroundColor: 'hsl(var(--card))',
              border: '1px solid hsl(var(--border))',
              borderRadius: '4px',
              fontSize: '12px',
              color: 'hsl(var(--foreground))',
            }}
            cursor={{ stroke: 'hsl(var(--muted-foreground))' }}
          />
          <Legend wrapperStyle={{ fontSize: '12px' }} />
          {socketKeys.map((key) => {
            // key format: "s0_cpu", "s1_dram", etc.
            const parts = key.split('_')
            const socketNum = parseInt(parts[0].replace('s', ''))
            const domain = parts.slice(1).join('_')
            const color = SOCKET_COLORS[domain] ?? 'hsl(var(--muted-foreground))'
            const dashArray = SOCKET_LINE_STYLES[socketNum % SOCKET_LINE_STYLES.length]

            return (
              <Line
                key={key}
                type="monotone"
                dataKey={key}
                stroke={color}
                strokeWidth={2}
                strokeDasharray={dashArray || undefined}
                dot={false}
                name={`Socket ${socketNum} · ${domain.toUpperCase()}`}
                isAnimationActive={false}
                connectNulls={true}
              />
            )
          })}
          <Brush
            dataKey="time"
            height={30}
            stroke="hsl(var(--border))"
            fill="hsl(var(--card))"
            travellerWidth={10}
          />
        </LineChart>
      </ResponsiveContainer>
    </div>
  )
}
