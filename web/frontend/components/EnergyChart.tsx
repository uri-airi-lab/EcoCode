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
import { format } from 'date-fns'
import { ChartInfoTooltip } from './ChartInfoTooltip'

const GPU_COLORS = [
  'hsl(199 89% 48%)',   // Blue
  'hsl(280 65% 60%)',   // Purple
  'hsl(49 89% 52%)',    // Gold
  'hsl(340 82% 52%)',   // Pink
  'hsl(20 89% 55%)',    // Orange
  'hsl(180 80% 45%)',   // Teal
  'hsl(60 80% 50%)',    // Lime
  'hsl(158 94% 45%)',   // Neon Green
]

interface EnergyChartProps {
  data: MetricRecord[]
}

export function EnergyChart({ data }: EnergyChartProps) {
  // Detect max number of GPUs across all records
  const gpuCount = useMemo(() => {
    let maxIdx = -1
    for (const record of data) {
      for (const g of record.gpu_details ?? []) {
        if (g.device_index > maxIdx) maxIdx = g.device_index
      }
    }
    return maxIdx + 1
  }, [data])

  const chartData = useMemo(() => {
    return data.map((record, index) => {
      // Calculate time delta in seconds from previous record (fallback to 1s for first record)
      const prev = index > 0 ? data[index - 1] : null
      const deltaMs = prev 
        ? new Date(record.timestamp).getTime() - new Date(prev.timestamp).getTime()
        : 1000
      const deltaS = (deltaMs / 1000) || 1

      const point: Record<string, string | number> = {
        time: format(new Date(record.timestamp), 'HH:mm:ss'),
        cpu: Number(((record.cpu_energy ?? 0) / deltaS).toFixed(2)),
        gpu: Number(((record.gpu_energy ?? 0) / deltaS).toFixed(2)),
        mem: Number(((record.mem_energy ?? 0) / deltaS).toFixed(2)),
        igpu: Number(((record.igpu_energy ?? 0) / deltaS).toFixed(2)),
        total: Number((((record.cpu_energy ?? 0) + (record.gpu_energy ?? 0) + (record.mem_energy ?? 0) + (record.igpu_energy ?? 0)) / deltaS).toFixed(2)),
      }

      // Initialize all GPU keys to 0
      for (let i = 0; i < gpuCount; i++) {
        point[`gpu${i}`] = 0
      }

      // Add per-GPU fields
      for (const g of record.gpu_details ?? []) {
        point[`gpu${g.device_index}`] = Number(((g.gpu_energy ?? 0) / deltaS).toFixed(2))
      }

      return point
    })
  }, [data, gpuCount])

  return (
    <div className="chart-container">
      <div className="flex flex-row items-center gap-2 mb-4">
        <h3 className="text-lg font-semibold text-foreground">
          Energy Consumption Over Time
        </h3>
        <ChartInfoTooltip content="Shows a timeline of the raw power (Watts) being drawn by your hardware components second by second." />
      </div>
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
              backgroundColor: 'hsl(0 0% 6%)',
              border: '1px solid hsl(0 0% 12%)',
              borderRadius: '4px',
              fontSize: '12px',
            }}
            itemStyle={{ padding: '2px 0' }}
            cursor={{ stroke: 'hsl(0 0% 20%)' }}
          />
          <Line
            type="monotone"
            dataKey="cpu"
            stroke="hsl(158 94% 45%)"
            strokeWidth={2}
            dot={false}
            name="CPU Energy"
            isAnimationActive={false}
            connectNulls={true}
          />
          <Line
            type="monotone"
            dataKey="gpu"
            stroke="hsl(199 89% 48%)"
            strokeWidth={2}
            dot={false}
            name="GPU Energy (Total)"
            isAnimationActive={false}
            connectNulls={true}
          />
          {gpuCount > 1 && Array.from({ length: gpuCount }, (_, i) => (
            <Line
              key={`gpu${i}`}
              type="monotone"
              dataKey={`gpu${i}`}
              stroke={GPU_COLORS[i % GPU_COLORS.length]}
              strokeWidth={1}
              strokeDasharray="4 2"
              dot={false}
              name={`GPU ${i}`}
              isAnimationActive={false}
              connectNulls={true}
            />
          ))}
          <Line
            type="monotone"
            dataKey="mem"
            stroke="hsl(20 89% 55%)"
            strokeWidth={2}
            dot={false}
            name="Memory Energy"
            isAnimationActive={false}
            connectNulls={true}
          />
          <Line
            type="monotone"
            dataKey="igpu"
            stroke="hsl(49 89% 52%)"
            strokeWidth={2}
            dot={false}
            name="iGPU Energy"
            isAnimationActive={false}
            connectNulls={true}
          />
          <Brush
            dataKey="time"
            height={30}
            stroke="rgba(255,255,255,0.2)"
            fill="rgba(0,0,0,0.5)"
            travellerWidth={10}
          />
        </LineChart>
      </ResponsiveContainer>
    </div>
  )
}

