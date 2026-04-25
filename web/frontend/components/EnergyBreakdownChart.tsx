'use client'

import React, { useMemo } from 'react'
import {
  BarChart,
  Bar,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  Legend,
  ResponsiveContainer,
} from 'recharts'
import { Run } from '@/lib/mock-data'
import { ChartInfoTooltip } from './ChartInfoTooltip'
import { chartColor } from '@/lib/chart-colors'

const GPU_COLORS = [1, 2, 3, 4, 5, 6].map(n => chartColor(n as 1 | 2 | 3 | 4 | 5 | 6))

interface EnergyBreakdownChartProps {
  run: Run
}

export function EnergyBreakdownChart({ run }: EnergyBreakdownChartProps) {
  // Detect max GPU count across all records
  const gpuCount = useMemo(() => {
    let maxIdx = -1
    for (const record of run.records) {
      for (const g of record.gpu_details ?? []) {
        if (g.device_index > maxIdx) maxIdx = g.device_index
      }
    }
    return maxIdx + 1
  }, [run.records])

  const chartData = useMemo(() => {
    let cpu = 0, gpu = 0, mem = 0, igpu = 0
    const gpus: Record<string, number> = {}

    run.records.forEach((record) => {
      cpu += record.cpu_energy ?? 0
      gpu += record.gpu_energy ?? 0
      mem += record.mem_energy ?? 0
      igpu += record.igpu_energy ?? 0

      for (const g of record.gpu_details ?? []) {
        const gKey = `gpu${g.device_index}`
        gpus[gKey] = (gpus[gKey] ?? 0) + (g.gpu_energy ?? 0)
      }
    })

    // Assuming 1 second interval for energy calculation (W * s / 3600 = Wh)
    const factor = 1 / 3600

    const point: Record<string, string | number> = {
      name: 'Energy (Wh)',
      cpu: Number((cpu * factor).toFixed(4)),
      gpu: Number((gpu * factor).toFixed(4)),
      mem: Number((mem * factor).toFixed(4)),
      igpu: Number((igpu * factor).toFixed(4)),
    }

    for (const [gKey, total] of Object.entries(gpus)) {
      point[gKey] = Number((total * factor).toFixed(4))
    }

    return [point]
  }, [run.records])

  return (
    <div className="chart-container">
      <div className="flex flex-row items-center gap-2 mb-4">
        <h3 className="text-lg font-semibold text-foreground">
          Total Accumulated Energy Breakdown
        </h3>
        <ChartInfoTooltip content="Displays the total energy (in Watt-hours) consumed by each hardware component over the entire duration of the run." />
      </div>
      <ResponsiveContainer width="100%" height={200}>
        <BarChart
          data={chartData}
          layout="vertical"
          margin={{ top: 5, right: 30, left: 40, bottom: 5 }}
        >
          <CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--border) / 0.6)" horizontal={false} />
          <XAxis type="number" stroke="hsl(var(--border))" tick={{ fontSize: 12, fill: 'hsl(var(--muted-foreground))' }} />
          <YAxis
            dataKey="name"
            type="category"
            stroke="hsl(var(--border))"
            tick={{ fontSize: 12, fill: 'hsl(var(--muted-foreground))' }}
          />
          <Tooltip
            contentStyle={{
              backgroundColor: 'hsl(var(--card))',
              border: '1px solid hsl(var(--border))',
              borderRadius: '4px',
              fontSize: '12px',
              color: 'hsl(var(--foreground))',
            }}
            formatter={(value: number) => [`${value} Wh`, '']}
          />
          <Legend wrapperStyle={{ fontSize: '12px', paddingTop: '20px' }} />
          <Bar
            dataKey="cpu"
            stackId="a"
            fill={chartColor(1)}
            name="CPU"
            isAnimationActive={false}
          />
          {gpuCount > 1 ? (
            Array.from({ length: gpuCount }, (_, i) => (
              <Bar
                key={`gpu${i}`}
                dataKey={`gpu${i}`}
                stackId="a"
                fill={GPU_COLORS[i % GPU_COLORS.length]}
                name={`GPU ${i}`}
                isAnimationActive={false}
              />
            ))
          ) : (
            <Bar
              dataKey="gpu"
              stackId="a"
              fill={chartColor(2)}
              name="GPU"
              isAnimationActive={false}
            />
          )}
          <Bar
            dataKey="mem"
            stackId="a"
            fill={chartColor(3)}
            name="Memory"
            isAnimationActive={false}
          />
          <Bar
            dataKey="igpu"
            stackId="a"
            fill={chartColor(4)}
            name="iGPU"
            isAnimationActive={false}
          />
        </BarChart>
      </ResponsiveContainer>
    </div>
  )
}
