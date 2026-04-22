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

interface CumulativePowerChartProps {
  data: MetricRecord[]
}

export function CumulativePowerChart({ data }: CumulativePowerChartProps) {
  const chartData = useMemo(() => {
    return data.map((record) => ({
      time: format(new Date(record.timestamp), 'HH:mm:ss'),
      cpu: Number((record.cpu_energy ?? 0).toFixed(2)),
      gpu: Number((record.gpu_energy ?? 0).toFixed(2)),
      mem: Number((record.mem_energy ?? 0).toFixed(2)),
      igpu: Number((record.igpu_energy ?? 0).toFixed(2)),
    }))
  }, [data])

  return (
    <div className="chart-container">
      <h3 className="text-lg font-semibold mb-4 text-foreground">
        Total System Power Stack
      </h3>
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
            name="CPU"
            isAnimationActive={false}
          />
          <Area
            type="monotone"
            dataKey="gpu"
            stackId="1"
            stroke="hsl(199 89% 48%)"
            fill="hsl(199 89% 48%)"
            fillOpacity={0.6}
            name="GPU"
            isAnimationActive={false}
          />
          <Area
            type="monotone"
            dataKey="mem"
            stackId="1"
            stroke="hsl(20 89% 55%)"
            fill="hsl(20 89% 55%)"
            fillOpacity={0.6}
            name="Memory"
            isAnimationActive={false}
          />
          <Area
            type="monotone"
            dataKey="igpu"
            stackId="1"
            stroke="hsl(49 89% 52%)"
            fill="hsl(49 89% 52%)"
            fillOpacity={0.6}
            name="iGPU"
            isAnimationActive={false}
          />
        </AreaChart>
      </ResponsiveContainer>
    </div>
  )
}
