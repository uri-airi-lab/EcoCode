'use client'

import React, { useMemo, useRef } from 'react'
import {
  Chart as ChartJS,
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  Title,
  Tooltip,
  Legend,
  Filler,
  ChartDataset,
} from 'chart.js'
import { Line } from 'react-chartjs-2'
import annotationPlugin from 'chartjs-plugin-annotation'
import { MetricRecord } from '@/lib/mock-data'
import { ChartInfoTooltip } from '@/components/ChartInfoTooltip'
import { formatChartTime, minIntervalMs } from '@/lib/utils'
import { chartColor } from '@/lib/chart-colors'

// Register ChartJS components
ChartJS.register(
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  Title,
  Tooltip,
  Legend,
  Filler,
  annotationPlugin
)

const PX_PER_POINT = 12
const MIN_WIDTH = 600

interface RunData {
  runId: string
  runName: string
  records: MetricRecord[]
}

interface CumulativePowerChartProps {
  runs?: RunData[]
  data?: MetricRecord[]
}

export function CumulativePowerChart({ runs: runsProp, data }: CumulativePowerChartProps) {
  const scrollRef = useRef<HTMLDivElement>(null)

  // Normalize: accept either runs[] (multi-run dashboard) or data[] (single-run detail)
  const runs: RunData[] = runsProp ?? (data ? [{ runId: 'single', runName: '', records: data }] : [])

  const { chartLabels, datasets, separatorIndices } = useMemo(() => {
    const labels: string[] = []
    const cpuData: number[] = []
    const gpuData: number[] = []
    const memData: number[] = []
    const igpuData: number[] = []
    const seps: { index: number; runName: string }[] = []

    const allTimestamps = runs.flatMap(r => r.records.map(rec => rec.timestamp))
    const intervalMs = minIntervalMs(allTimestamps)

    runs.forEach((run, runIdx) => {
      const records = run.records.length > 1 ? run.records.slice(1) : run.records

      if (runIdx > 0 && labels.length > 0) {
        seps.push({ index: labels.length, runName: run.runName })
      }

      records.forEach((record, index) => {
        const prev = index > 0
          ? records[index - 1]
          : (run.records.length > 1 ? run.records[0] : null)
        const deltaMs = prev ? record.timestamp - prev.timestamp : 1000
        const deltaS = (deltaMs / 1000) || 1

        labels.push(formatChartTime(record.timestamp, intervalMs))
        cpuData.push(Number(((record.cpu_energy ?? 0) / deltaS).toFixed(2)))
        gpuData.push(Number(((record.gpu_energy ?? 0) / deltaS).toFixed(2)))
        memData.push(Number(((record.mem_energy ?? 0) / deltaS).toFixed(2)))
        igpuData.push(Number(((record.igpu_energy ?? 0) / deltaS).toFixed(2)))
      })
    })

    const finalDatasets: ChartDataset<'line'>[] = [
      {
        label: 'CPU Power (W)',
        data: cpuData,
        borderColor: chartColor(1),
        backgroundColor: chartColor(1, 0.4),
        fill: true,
        borderWidth: 1,
        pointRadius: 0,
        tension: 0.1,
      },
      {
        label: 'GPU Power (W)',
        data: gpuData,
        borderColor: chartColor(2),
        backgroundColor: chartColor(2, 0.4),
        fill: true,
        borderWidth: 1,
        pointRadius: 0,
        tension: 0.1,
      },
      {
        label: 'Memory Power (W)',
        data: memData,
        borderColor: chartColor(3),
        backgroundColor: chartColor(3, 0.4),
        fill: true,
        borderWidth: 1,
        pointRadius: 0,
        tension: 0.1,
      },
      {
        label: 'iGPU Power (W)',
        data: igpuData,
        borderColor: chartColor(4),
        backgroundColor: chartColor(4, 0.4),
        fill: true,
        borderWidth: 1,
        pointRadius: 0,
        tension: 0.1,
      },
    ]

    return { chartLabels: labels, datasets: finalDatasets, separatorIndices: seps }
  }, [runs])

  const chartWidth = Math.max(MIN_WIDTH, chartLabels.length * PX_PER_POINT)

  const options = {
    responsive: true,
    maintainAspectRatio: false,
    interaction: {
      mode: 'index' as const,
      intersect: false,
    },
    scales: {
      x: {
        grid: {
          display: true,
          color: 'rgba(255, 255, 255, 0.05)',
        },
        ticks: {
          color: 'hsl(215 20.2% 65.1%)',
          font: { size: 10 },
          maxRotation: 0,
          autoSkip: true,
          maxTicksLimit: 10,
        },
      },
      y: {
        stacked: true,
        grid: {
          display: true,
          color: 'rgba(255, 255, 255, 0.05)',
        },
        ticks: {
          color: 'hsl(215 20.2% 65.1%)',
          font: { size: 10 },
        },
        title: {
          display: true,
          text: 'Watts',
          color: 'hsl(215 20.2% 65.1%)',
          font: { size: 10 },
        },
      },
    },
    plugins: {
      legend: {
        position: 'bottom' as const,
        labels: {
          color: 'hsl(215 20.2% 65.1%)',
          usePointStyle: true,
          padding: 20,
          font: { size: 11 },
        },
      },
      tooltip: {
        backgroundColor: 'hsl(222 47% 10%)',
        borderColor: 'hsl(217.2 32.6% 17.5%)',
        borderWidth: 1,
        titleFont: { size: 12, family: 'var(--font-jetbrains-mono)' },
        bodyFont: { size: 11, family: 'var(--font-jetbrains-mono)' },
        padding: 12,
        cornerRadius: 4,
      },
      annotation: {
        annotations: separatorIndices.reduce((acc, sep) => {
          acc[`sep-${sep.index}`] = {
            type: 'line',
            xMin: sep.index,
            xMax: sep.index,
            borderColor: 'rgba(255, 255, 255, 0.2)',
            borderWidth: 1,
            borderDash: [6, 6],
            label: {
              display: true,
              content: sep.runName,
              position: 'start',
              backgroundColor: 'rgba(0,0,0,0.5)',
              color: 'hsl(215 20.2% 65.1%)',
              font: { size: 10, weight: 'bold' },
              padding: 4,
            }
          }
          return acc
        }, {} as any)
      }
    },
  }

  return (
    <div className="chart-container">
      <div className="flex flex-row items-center gap-2 mb-4">
        <h3 className="text-lg font-semibold text-foreground">
          Stacked Power Over Time
        </h3>
        <ChartInfoTooltip content="A stacked area view of instantaneous power draw (Watts) per component across all runs. Vertical markers separate individual runs." />
      </div>
      <div className="relative h-[260px]">
        <div ref={scrollRef} className="overflow-x-auto h-full">
          <div style={{ width: chartWidth, height: '100%' }}>
            <Line data={{ labels: chartLabels, datasets }} options={options as any} />
          </div>
        </div>
      </div>
    </div>
  )
}
