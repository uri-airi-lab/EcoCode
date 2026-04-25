'use client'

import React from 'react'
import {
  PieChart,
  Pie,
  Cell,
  ResponsiveContainer,
  Tooltip,
  Legend
} from 'recharts'
import { MetricRecord } from '@/lib/mock-data'
import { ChartInfoTooltip } from './ChartInfoTooltip'
import { chartColor } from '@/lib/chart-colors'

interface PowerDistributionChartProps {
  record: MetricRecord
  prevRecord?: MetricRecord
}

export function PowerDistributionChart({ record, prevRecord }: PowerDistributionChartProps) {
  if (!record) return null;

  // Without a previous record we cannot compute a valid time delta, so
  // energy / deltaS would use the 1 s fallback and wildly over-report Watts.
  // Show a loading indicator until the second record arrives (~one interval).
  if (!prevRecord) {
    return (
      <div className="chart-container flex flex-col h-full items-center justify-center min-h-[300px]">
        <h3 className="text-lg font-semibold text-foreground mb-2">Live Power Distribution</h3>
        <p className="text-sm text-muted-foreground">Waiting for second sample…</p>
      </div>
    )
  }

  const deltaS = React.useMemo(() => {
    const deltaMs = new Date(record.timestamp).getTime() - new Date(prevRecord.timestamp).getTime()
    return (deltaMs / 1000) || 1
  }, [record, prevRecord])

  const data = [
    { name: 'CPU', value: (record.cpu_energy ?? 0) / deltaS, color: chartColor(1) },
    { name: 'GPU', value: (record.gpu_energy ?? 0) / deltaS, color: chartColor(2) },
    { name: 'Memory', value: (record.mem_energy ?? 0) / deltaS, color: chartColor(3) },
    { name: 'iGPU', value: (record.igpu_energy ?? 0) / deltaS, color: chartColor(4) },
  ].filter(item => item.value > 0)

  const totalWatts = data.reduce((sum, item) => sum + item.value, 0)

  return (
    <div className="chart-container flex flex-col h-full">
      <div className="flex flex-row items-center gap-2 mb-4">
        <h3 className="text-lg font-semibold text-foreground">
          Live Power Distribution
        </h3>
        <ChartInfoTooltip content="Shows the current power draw split between your components (CPU, GPU, RAM) at the latest recorded point in time." />
      </div>
      <div className="flex-1 relative min-h-[250px]">
        <ResponsiveContainer width="100%" height="100%">
          <PieChart>
            <Pie
              data={data}
              cx="50%"
              cy="50%"
              innerRadius={60}
              outerRadius={80}
              paddingAngle={5}
              dataKey="value"
              isAnimationActive={false}
            >
              {data.map((entry, index) => (
                <Cell key={`cell-${index}`} fill={entry.color} stroke="none" />
              ))}
            </Pie>
            <Tooltip
              contentStyle={{
                backgroundColor: 'hsl(var(--card))',
                border: '1px solid hsl(var(--border))',
                borderRadius: '4px',
                fontSize: '12px',
                color: 'hsl(var(--foreground))',
              }}
              formatter={(value: number) => [`${value.toFixed(2)} W`, '']}
            />
            <Legend verticalAlign="bottom" height={36} />
          </PieChart>
        </ResponsiveContainer>
        <div className="absolute inset-0 flex flex-col items-center justify-center pointer-events-none">
          <span className="text-2xl font-bold text-foreground">
            {totalWatts.toFixed(1)}
          </span>
          <span className="text-[10px] uppercase tracking-widest text-muted-foreground">
            Total Watts
          </span>
        </div>
      </div>
    </div>
  )
}
