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

const GPU_COLORS = [
  'hsl(199 89% 48%)',   // Blue
  'hsl(280 65% 60%)',   // Purple
  'hsl(49 89% 52%)',    // Gold
  'hsl(340 82% 52%)',   // Pink
  'hsl(20 89% 55%)',    // Orange
  'hsl(180 80% 45%)',   // Teal
  'hsl(60 80% 50%)',    // Lime
  'hsl(158 94% 45%)',   // Neon Green (moved to end)
]

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
      <h3 className="text-lg font-semibold mb-4 text-foreground">
        Total Accumulated Energy Breakdown
      </h3>
      <ResponsiveContainer width="100%" height={200}>
        <BarChart
          data={chartData}
          layout="vertical"
          margin={{ top: 5, right: 30, left: 40, bottom: 5 }}
        >
          <CartesianGrid strokeDasharray="3 3" stroke="rgba(255,255,255,0.1)" horizontal={false} />
          <XAxis type="number" stroke="rgba(255,255,255,0.4)" style={{ fontSize: '12px' }} />
          <YAxis
            dataKey="name"
            type="category"
            stroke="rgba(255,255,255,0.4)"
            style={{ fontSize: '12px' }}
          />
          <Tooltip
            contentStyle={{
              backgroundColor: 'hsl(0 0% 6%)',
              border: '1px solid hsl(0 0% 12%)',
              borderRadius: '4px',
              fontSize: '12px',
            }}
            formatter={(value: number) => [`${value} Wh`, '']}
          />
          <Legend wrapperStyle={{ fontSize: '12px', paddingTop: '20px' }} />
          <Bar
            dataKey="cpu"
            stackId="a"
            fill="hsl(158 94% 45%)"
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
              fill="hsl(199 89% 48%)"
              name="GPU"
              isAnimationActive={false}
            />
          )}
          <Bar
            dataKey="mem"
            stackId="a"
            fill="hsl(20 89% 55%)"
            name="Memory"
            isAnimationActive={false}
          />
          <Bar
            dataKey="igpu"
            stackId="a"
            fill="hsl(49 89% 52%)"
            name="iGPU"
            isAnimationActive={false}
          />
        </BarChart>
      </ResponsiveContainer>
    </div>
  )
}
