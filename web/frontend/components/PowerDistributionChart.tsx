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

interface PowerDistributionChartProps {
  record: MetricRecord
}

export function PowerDistributionChart({ record }: PowerDistributionChartProps) {
  const data = [
    { name: 'CPU', value: record.cpu_energy ?? 0, color: 'hsl(158 94% 45%)' },
    { name: 'GPU', value: record.gpu_energy ?? 0, color: 'hsl(199 89% 48%)' },
    { name: 'Memory', value: record.mem_energy ?? 0, color: 'hsl(20 89% 55%)' },
    { name: 'iGPU', value: record.igpu_energy ?? 0, color: 'hsl(49 89% 52%)' },
  ].filter(item => item.value > 0)

  const totalWatts = data.reduce((sum, item) => sum + item.value, 0)

  return (
    <div className="chart-container flex flex-col h-full">
      <h3 className="text-lg font-semibold mb-4 text-foreground">
        Live Power Distribution
      </h3>
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
                backgroundColor: 'hsl(0 0% 6%)',
                border: '1px solid hsl(0 0% 12%)',
                borderRadius: '4px',
                fontSize: '12px',
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
