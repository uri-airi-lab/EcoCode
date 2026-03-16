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
  'hsl(49 89% 52%)',   // GPU 0 - gold
  'hsl(20 89% 55%)',   // GPU 1 - orange
  'hsl(340 82% 52%)',  // GPU 2 - pink
  'hsl(300 70% 50%)',  // GPU 3 - magenta
  'hsl(180 80% 45%)',  // GPU 4 - teal
  'hsl(60 80% 50%)',   // GPU 5 - lime
  'hsl(0 85% 55%)',    // GPU 6 - red
  'hsl(240 70% 60%)',  // GPU 7 - indigo
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
    // Create time buckets for better visualization
    const buckets: Record<string, { cpu: number; gpu: number; mem: number; igpu: number; count: number; gpus: Record<string, number> }> = {}

    run.records.forEach((record) => {
      const time = new Date(record.timestamp)
      const minute = Math.floor(time.getTime() / 60000) * 60000
      const key = new Date(minute).toLocaleTimeString('en-US', {
        hour: '2-digit',
        minute: '2-digit',
      })

      if (!buckets[key]) {
        buckets[key] = { cpu: 0, gpu: 0, mem: 0, igpu: 0, count: 0, gpus: {} }
      }

      buckets[key].cpu += record.cpu_energy
      buckets[key].gpu += record.gpu_energy
      buckets[key].mem += record.mem_energy
      buckets[key].igpu += record.igpu_energy
      buckets[key].count += 1

      for (const g of record.gpu_details ?? []) {
        const gKey = `gpu${g.device_index}`
        buckets[key].gpus[gKey] = (buckets[key].gpus[gKey] ?? 0) + g.gpu_energy
      }
    })

    return Object.entries(buckets)
      .slice(-20)
      .map(([time, data]) => {
        const point: Record<string, string | number> = {
          time,
          cpu: Number((data.cpu / data.count).toFixed(2)),
          gpu: Number((data.gpu / data.count).toFixed(2)),
          mem: Number((data.mem / data.count).toFixed(2)),
          igpu: Number((data.igpu / data.count).toFixed(2)),
        }
        for (const [gKey, total] of Object.entries(data.gpus)) {
          point[gKey] = Number((total / data.count).toFixed(2))
        }
        return point
      })
  }, [run.records])

  return (
    <div className="chart-container">
      <h3 className="text-lg font-semibold mb-4 text-foreground">
        Energy Breakdown by Component
      </h3>
      <ResponsiveContainer width="100%" height={300}>
        <BarChart data={chartData}>
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
          />
          <Legend wrapperStyle={{ fontSize: '12px' }} />
          <Bar
            dataKey="cpu"
            fill="hsl(160 84% 39%)"
            radius={[4, 4, 0, 0]}
            name="CPU Energy (W)"
            isAnimationActive={false}
          />
          <Bar
            dataKey="gpu"
            fill="hsl(49 89% 52%)"
            radius={[4, 4, 0, 0]}
            name={gpuCount > 1 ? "GPU Energy Total (W)" : "GPU Energy (W)"}
            isAnimationActive={false}
          />
          {gpuCount > 1 && Array.from({ length: gpuCount }, (_, i) => (
            <Bar
              key={`gpu${i}`}
              dataKey={`gpu${i}`}
              fill={GPU_COLORS[i % GPU_COLORS.length]}
              radius={[4, 4, 0, 0]}
              name={`GPU ${i} (W)`}
              isAnimationActive={false}
            />
          ))}
          <Bar
            dataKey="mem"
            fill="hsl(210 100% 50%)"
            radius={[4, 4, 0, 0]}
            name="Memory Energy (W)"
            isAnimationActive={false}
          />
          <Bar
            dataKey="igpu"
            fill="hsl(280 100% 50%)"
            radius={[4, 4, 0, 0]}
            name="iGPU Energy (W)"
            isAnimationActive={false}
          />
        </BarChart>
      </ResponsiveContainer>
    </div>
  )
}
