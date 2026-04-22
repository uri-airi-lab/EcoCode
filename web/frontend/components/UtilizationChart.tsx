'use client'

import { useMemo } from 'react'
import {
  AreaChart,
  Area,
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

interface UtilizationChartProps {
  data: MetricRecord[]
}

export function UtilizationChart({ data }: UtilizationChartProps) {
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
    const records = data

    return records.map((record) => {
      const point: Record<string, string | number> = {
        time: format(new Date(record.timestamp), 'HH:mm:ss'),
        cpu: Number((record.cpu_usage ?? 0).toFixed(2)),
        gpu: Number((record.gpu_usage ?? 0).toFixed(2)),
        mem: Number((record.mem_usage ?? 0).toFixed(2)),
      }

      // Initialize all GPU keys to 0 to prevent gaps
      for (let i = 0; i < gpuCount; i++) {
        point[`gpu${i}`] = 0
      }
      for (const g of record.gpu_details ?? []) {
        point[`gpu${g.device_index}`] = Number((g.gpu_usage ?? 0).toFixed(2))
      }

      return point
    })
  }, [data])

  return (
    <div className="chart-container">
      <h3 className="text-lg font-semibold mb-4 text-foreground">
        Resource Utilization
      </h3>
      <ResponsiveContainer width="100%" height={300}>
        <AreaChart data={chartData}>
          <defs>
            <linearGradient id="colorCpu" x1="0" y1="0" x2="0" y2="1">
              <stop offset="5%" stopColor="hsl(158 94% 45%)" stopOpacity={0.1} />
              <stop offset="95%" stopColor="hsl(158 94% 45%)" stopOpacity={0} />
            </linearGradient>
            <linearGradient id="colorGpu" x1="0" y1="0" x2="0" y2="1">
              <stop offset="5%" stopColor="hsl(199 89% 48%)" stopOpacity={0.1} />
              <stop offset="95%" stopColor="hsl(199 89% 48%)" stopOpacity={0} />
            </linearGradient>
            <linearGradient id="colorMem" x1="0" y1="0" x2="0" y2="1">
              <stop offset="5%" stopColor="hsl(20 89% 55%)" stopOpacity={0.1} />
              <stop offset="95%" stopColor="hsl(20 89% 55%)" stopOpacity={0} />
            </linearGradient>
          </defs>
          <CartesianGrid strokeDasharray="3 3" stroke="rgba(255,255,255,0.1)" />
          <XAxis
            dataKey="time"
            stroke="rgba(255,255,255,0.4)"
            style={{ fontSize: '12px' }}
          />
          <YAxis
            stroke="rgba(255,255,255,0.4)"
            style={{ fontSize: '12px' }}
            label={{ value: 'Usage %', angle: -90, position: 'insideLeft' }}
            domain={[0, 100]}
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
          <Area
            type="monotone"
            dataKey="cpu"
            stroke="hsl(158 94% 45%)"
            fillOpacity={1}
            fill="url(#colorCpu)"
            name="CPU Usage"
            isAnimationActive={false}
            connectNulls={true}
          />
          <Area
            type="monotone"
            dataKey="gpu"
            stroke="hsl(199 89% 48%)"
            fillOpacity={1}
            fill="url(#colorGpu)"
            name={gpuCount > 1 ? "GPU Usage (Avg)" : "GPU Usage"}
            isAnimationActive={false}
            connectNulls={true}
          />
          {gpuCount > 1 && Array.from({ length: gpuCount }, (_, i) => (
            <Area
              key={`gpu${i}`}
              type="monotone"
              dataKey={`gpu${i}`}
              stroke={GPU_COLORS[i % GPU_COLORS.length]}
              strokeDasharray="4 2"
              fillOpacity={0}
              fill="transparent"
              name={`GPU ${i}`}
              isAnimationActive={false}
              connectNulls={true}
            />
          ))}
          <Area
            type="monotone"
            dataKey="mem"
            stroke="hsl(20 89% 55%)"
            fillOpacity={1}
            fill="url(#colorMem)"
            name="Memory Usage"
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
        </AreaChart>
      </ResponsiveContainer>
    </div>
  )
}

