'use client'

import React, { useMemo } from 'react'
import {
  AreaChart,
  Area,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  Legend
} from 'recharts'
import { MetricRecord } from '@/lib/mock-data'
import { format } from 'date-fns'
import { ChartInfoTooltip } from './ChartInfoTooltip'

interface CumulativePowerChartProps {
  data: MetricRecord[]
}

export function CumulativePowerChart({ data }: CumulativePowerChartProps) {
  const chartData = useMemo(() => {
    // Skip first record (startup warmup) and use it as a ghost prev for
    // deltaS so the first displayed point shows the real interval, not 1s.
    const records = data.length > 1 ? data.slice(1) : data

    return records.map((record, index) => {
      // For the first displayed record use data[0] (skipped warmup) as prev
      // so deltaS reflects the actual interval instead of the 1s fallback.
      const prev = index > 0
        ? records[index - 1]
        : (data.length > 1 ? data[0] : null)
      const deltaMs = prev
        ? new Date(record.timestamp).getTime() - new Date(prev.timestamp).getTime()
        : 1000
      const deltaS = (deltaMs / 1000) || 1

      return {
        time: format(new Date(record.timestamp), 'HH:mm:ss'),
        cpu: Number(((record.cpu_energy ?? 0) / deltaS).toFixed(2)),
        gpu: Number(((record.gpu_energy ?? 0) / deltaS).toFixed(2)),
        mem: Number(((record.mem_energy ?? 0) / deltaS).toFixed(2)),
        igpu: Number(((record.igpu_energy ?? 0) / deltaS).toFixed(2)),
      }
    })
  }, [data])

  return (
    <div className="chart-container">
      <div className="flex flex-row items-center gap-2 mb-4">
        <h3 className="text-lg font-semibold text-foreground">
          Total System Power Stack
        </h3>
        <ChartInfoTooltip content="A stacked area view of power consumption. The top line represents the total power draw of all tracked components combined." />
      </div>
      <ResponsiveContainer width="100%" height={300}>
        <AreaChart data={chartData}>
          <CartesianGrid strokeDasharray="3 3" stroke="rgba(255,255,255,0.05)" />
          <XAxis
            dataKey="time"
            stroke="rgba(255,255,255,0.4)"
            style={{ fontSize: '10px' }}
          />
          <YAxis
            stroke="rgba(255,255,255,0.4)"
            style={{ fontSize: '10px' }}
            label={{ value: 'Watts', angle: -90, position: 'insideLeft', style: { fill: 'rgba(255,255,255,0.4)', fontSize: '10px' } }}
          />
          <Tooltip
            contentStyle={{
              backgroundColor: 'hsl(0 0% 6%)',
              border: '1px solid hsl(0 0% 12%)',
              borderRadius: '4px',
              fontSize: '12px',
            }}
          />
          <Legend />
          <Area
            type="monotone"
            dataKey="cpu"
            stackId="1"
            stroke="hsl(158 94% 45%)"
            fill="hsl(158 94% 45%)"
            fillOpacity={0.6}
            name="CPU Power (W)"
            isAnimationActive={false}
          />
          <Area
            type="monotone"
            dataKey="gpu"
            stackId="1"
            stroke="hsl(199 89% 48%)"
            fill="hsl(199 89% 48%)"
            fillOpacity={0.6}
            name="GPU Power (W)"
            isAnimationActive={false}
          />
          <Area
            type="monotone"
            dataKey="mem"
            stackId="1"
            stroke="hsl(20 89% 55%)"
            fill="hsl(20 89% 55%)"
            fillOpacity={0.6}
            name="Memory Power (W)"
            isAnimationActive={false}
          />
          <Area
            type="monotone"
            dataKey="igpu"
            stackId="1"
            stroke="hsl(49 89% 52%)"
            fill="hsl(49 89% 52%)"
            fillOpacity={0.6}
            name="iGPU Power (W)"
            isAnimationActive={false}
          />
        </AreaChart>
      </ResponsiveContainer>
    </div>
  )
}
