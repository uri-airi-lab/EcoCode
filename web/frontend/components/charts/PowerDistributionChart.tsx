'use client'

import React from 'react'
import { PieChart, Pie, Cell, Tooltip, Legend, ResponsiveContainer } from 'recharts'
import { MetricRecord } from '@/lib/mock-data'
import { ChartInfoTooltip } from '@/components/ChartInfoTooltip'
import { chartColor } from '@/lib/chart-colors'

interface PowerDistributionChartProps {
  record: MetricRecord
  prevRecord?: MetricRecord
}

export function PowerDistributionChart({ record, prevRecord }: PowerDistributionChartProps) {
  if (!record) return null

  if (!prevRecord) {
    return (
      <div className="chart-container flex flex-col items-center justify-center min-h-[300px]">
        <h3 className="text-lg font-semibold text-foreground mb-2">Live Power Distribution</h3>
        <p className="text-sm text-muted-foreground">Waiting for second sample…</p>
      </div>
    )
  }

  const deltaS = React.useMemo(() => {
    const deltaMs = record.timestamp - prevRecord.timestamp
    return (deltaMs / 1000) || 1
  }, [record, prevRecord])

  const sourceData = React.useMemo(() => {
    const entries = [
      { label: 'CPU', value: (record.cpu_energy ?? 0) / deltaS, color: chartColor(1) },
    ]
    if (record.gpu_details && record.gpu_details.length > 0) {
      record.gpu_details.forEach((gpu, i) => {
        entries.push({
          label: `GPU ${gpu.device_index}`,
          value: (gpu.gpu_usage / 100) * ((record.gpu_energy ?? 0) / deltaS),
          color: chartColor(2 + i),
        })
      })
    } else {
      entries.push({ label: 'GPU', value: (record.gpu_energy ?? 0) / deltaS, color: chartColor(2) })
    }
    entries.push({ label: 'Memory', value: (record.mem_energy ?? 0) / deltaS, color: chartColor(3) })
    entries.push({ label: 'iGPU',   value: (record.igpu_energy ?? 0) / deltaS, color: chartColor(4) })
    return entries.filter(e => e.value > 0.05)
  }, [record, deltaS])

  const totalWatts = sourceData.reduce((s, e) => s + e.value, 0)

  return (
    <div className="chart-container flex flex-col h-full overflow-hidden">
      <div className="flex flex-row items-center gap-2 mb-4 shrink-0">
        <h3 className="text-lg font-semibold text-foreground">Live Power Distribution</h3>
        <ChartInfoTooltip content="Shows the current power draw split between your components." />
      </div>
      <div className="relative h-[240px] flex items-center justify-center overflow-hidden">
        <ResponsiveContainer width="100%" height="100%">
          <PieChart>
            <Pie
              data={sourceData}
              dataKey="value"
              nameKey="label"
              cx="50%"
              cy="50%"
              innerRadius="60%"
              outerRadius="80%"
              paddingAngle={2}
            >
              {sourceData.map((entry, i) => (
                <Cell key={i} fill={entry.color} stroke="hsl(220 12% 10%)" strokeWidth={2} />
              ))}
            </Pie>
            <Tooltip
              formatter={(value: number) => [`${value.toFixed(2)} W`]}
              contentStyle={{ background: 'hsl(220 12% 10%)', border: '1px solid hsl(220 8% 18%)', borderRadius: 4, fontSize: 11 }}
              itemStyle={{ color: 'hsl(220 5% 60%)' }}
            />
            <Legend
              iconType="square"
              iconSize={10}
              wrapperStyle={{ fontSize: 10, color: 'hsl(220 5% 60%)' }}
            />
          </PieChart>
        </ResponsiveContainer>
        <div className="absolute inset-0 flex flex-col items-center justify-center pointer-events-none -translate-y-4">
          <span className="text-4xl font-bold font-mono text-foreground tracking-tighter leading-none">
            {totalWatts.toFixed(1)}
          </span>
          <span className="text-[10px] font-bold uppercase tracking-[0.3em] text-primary/50 mt-1">
            Watts
          </span>
        </div>
      </div>
    </div>
  )
}
