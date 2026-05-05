'use client'

import React from 'react'
import { Chart as ChartJS, ArcElement, Tooltip, Legend, TooltipItem } from 'chart.js'
import { Doughnut } from 'react-chartjs-2'
import { MetricRecord } from '@/lib/mock-data'
import { ChartInfoTooltip } from '@/components/ChartInfoTooltip'
import { chartColor } from '@/lib/chart-colors'

// Register ChartJS components
ChartJS.register(ArcElement, Tooltip, Legend)

interface PowerDistributionChartProps {
  record: MetricRecord
  prevRecord?: MetricRecord
}

export function PowerDistributionChart({ record, prevRecord }: PowerDistributionChartProps) {
  if (!record) return null;

  if (!prevRecord) {
    return (
      <div className="chart-container flex flex-col items-center justify-center min-h-[300px]">
        <h3 className="text-lg font-semibold text-foreground mb-2">Live Power Distribution</h3>
        <p className="text-sm text-muted-foreground">Waiting for second sample…</p>
      </div>
    )
  }

  const deltaS = React.useMemo(() => {
    const deltaMs = new Date(record.timestamp).getTime() - new Date(prevRecord.timestamp).getTime()
    return (deltaMs / 1000) || 1
  }, [record, prevRecord])

  const sourceData = React.useMemo(() => {
    const data = [
      { label: 'CPU', value: (record.cpu_energy ?? 0) / deltaS, color: chartColor(1) },
    ]

    // If we have individual GPU details, use them; otherwise use aggregate
    if (record.gpu_details && record.gpu_details.length > 0) {
      record.gpu_details.forEach((gpu, i) => {
        data.push({
          label: `GPU ${gpu.device_index}`,
          value: (gpu.gpu_usage / 100) * ((record.gpu_energy ?? 0) / deltaS), // Approximation of per-GPU power if not explicitly provided
          color: chartColor(2 + i)
        })
      })
    } else {
      data.push({ label: 'GPU', value: (record.gpu_energy ?? 0) / deltaS, color: chartColor(2) })
    }

    data.push({ label: 'Memory', value: (record.mem_energy ?? 0) / deltaS, color: chartColor(3) })
    data.push({ label: 'iGPU', value: (record.igpu_energy ?? 0) / deltaS, color: chartColor(4) })

    return data.filter(item => item.value > 0.05)
  }, [record, deltaS])

  const totalWatts = sourceData.reduce((sum, item) => sum + item.value, 0)

  const chartData = {
    labels: sourceData.map(d => d.label),
    datasets: [
      {
        data: sourceData.map(d => d.value),
        backgroundColor: sourceData.map(d => d.color),
        borderColor: 'hsl(222 47% 11%)',
        borderWidth: 2,
        hoverOffset: 8,
        cutout: '72%',
      },
    ],
  }

  const options = {
    responsive: true,
    maintainAspectRatio: false,
    layout: {
      padding: { bottom: 10 }
    },
    plugins: {
      legend: {
        position: 'bottom' as const,
        labels: {
          color: 'hsl(215 20.2% 65.1%)',
          usePointStyle: true,
          pointStyle: 'rectRounded',
          padding: 15,
          font: {
            size: 10,
            family: 'var(--font-jetbrains-mono)',
          },
        },
      },
      tooltip: {
        backgroundColor: 'hsl(222 47% 10%)',
        borderColor: 'hsl(217.2 32.6% 17.5%)',
        borderWidth: 1,
        padding: 12,
        cornerRadius: 4,
        callbacks: {
          label: (item: any) => ` ${item.label}: ${Number(item.raw).toFixed(2)} W`
        },
      },
    },
  }

  return (
    <div className="chart-container flex flex-col h-full overflow-hidden">
      <div className="flex flex-row items-center gap-2 mb-4 shrink-0">
        <h3 className="text-lg font-semibold text-foreground">
          Live Power Distribution
        </h3>
        <ChartInfoTooltip content="Shows the current power draw split between your components. If multiple GPUs are detected, they are shown individually." />
      </div>
      <div className="relative h-[240px] flex items-center justify-center overflow-hidden">
        <div className="w-full h-full">
          <Doughnut data={chartData} options={options} />
        </div>
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
