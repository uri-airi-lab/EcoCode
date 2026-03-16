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
} from 'recharts'
import { MetricRecord } from '@/lib/mock-data'
import { format } from 'date-fns'

const SOCKET_COLORS: Record<string, string> = {
  cpu: 'hsl(160 84% 39%)',    // green
  dram: 'hsl(210 100% 50%)',  // blue
  igpu: 'hsl(280 100% 50%)',  // purple
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
    const records = data.slice(-100)

    return records.map((record) => {
      const point: Record<string, string | number> = {
        time: format(new Date(record.timestamp), 'HH:mm:ss'),
      }

      for (const s of record.socket_details ?? []) {
        point[`s${s.socket}_${s.domain}`] = Number(s.energy.toFixed(2))
      }

      return point
    })
  }, [data])

  if (socketKeys.length === 0) {
    return null // Don't render if no socket data
  }

  return (
    <div className="chart-container">
      <h3 className="text-lg font-semibold mb-4 text-foreground">
        Per-Socket Energy (RAPL)
      </h3>
      <ResponsiveContainer width="100%" height={300}>
        <LineChart data={chartData}>
          <CartesianGrid strokeDasharray="3 3" stroke="rgba(255,255,255,0.1)" />
          <XAxis
            dataKey="time"
            stroke="rgba(255,255,255,0.4)"
            style={{ fontSize: '12px' }}
          />
          <YAxis
            stroke="rgba(255,255,255,0.4)"
            style={{ fontSize: '12px' }}
            label={{ value: 'Watts', angle: -90, position: 'insideLeft' }}
          />
          <Tooltip
            contentStyle={{
              backgroundColor: 'rgba(0,0,0,0.8)',
              border: '1px solid rgba(255,255,255,0.2)',
              borderRadius: '8px',
            }}
            cursor={{ stroke: 'rgba(255,255,255,0.2)' }}
          />
          <Legend wrapperStyle={{ fontSize: '12px' }} />
          {socketKeys.map((key) => {
            // key format: "s0_cpu", "s1_dram", etc.
            const parts = key.split('_')
            const socketNum = parseInt(parts[0].replace('s', ''))
            const domain = parts.slice(1).join('_')
            const color = SOCKET_COLORS[domain] ?? 'hsl(0 0% 60%)'
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
              />
            )
          })}
        </LineChart>
      </ResponsiveContainer>
    </div>
  )
}
