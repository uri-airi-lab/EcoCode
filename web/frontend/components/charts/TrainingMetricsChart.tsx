'use client'

import { useMemo, useRef } from 'react'
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
import { chartColor } from '@/lib/chart-colors'
import { ChartInfoTooltip } from '@/components/ChartInfoTooltip'

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

interface EpochMarker {
  step: number
  epoch: number
}

const CUSTOM_COLOR_OFFSET = 6
const PX_PER_POINT = 12
const MIN_WIDTH = 600

interface TrainingMetricsChartProps {
  records: MetricRecord[]
}

export function TrainingMetricsChart({ records }: TrainingMetricsChartProps) {
  const scrollRef = useRef<HTMLDivElement>(null)

  const { chartLabels, datasets, epochs, hasLoss, hasLR, hasAccuracy, hasCustom, customKeys } = useMemo(() => {
    const labels: string[] = []
    const lossData: (number | null)[] = []
    const lrData: (number | null)[] = []
    const accuracyData: (number | null)[] = []
    const epochs: EpochMarker[] = []
    const customKeySet = new Set<string>()
    const customMetricData: Record<string, (number | null)[]> = {}
    
    let lastEpoch: string | null = null

    // Pre-pass for custom keys
    records.forEach(r => {
      if (r.custom_metrics) {
        Object.keys(r.custom_metrics).forEach(key => customKeySet.add(key))
      }
    })
    const sortedCustomKeys = Array.from(customKeySet).sort()
    sortedCustomKeys.forEach(key => {
      customMetricData[key] = []
    })

    records.forEach((r) => {
      if (r.training_step == null) return

      // Detect epoch boundaries
      if (r.phase?.startsWith('epoch:')) {
        const e = parseInt(r.phase.split(':')[1], 10)
        if (r.phase !== lastEpoch && !isNaN(e)) {
          epochs.push({ step: r.training_step, epoch: e })
          lastEpoch = r.phase
        }
      }

      const hasAnyMetric = r.loss != null || r.learning_rate != null || r.accuracy != null ||
        (r.custom_metrics && Object.keys(r.custom_metrics).length > 0)
      if (!hasAnyMetric) return

      labels.push(`Step ${r.training_step}`)
      lossData.push(r.loss ?? null)
      lrData.push(r.learning_rate ?? null)
      accuracyData.push(r.accuracy ?? null)

      sortedCustomKeys.forEach(key => {
        customMetricData[key].push(r.custom_metrics?.[key] ?? null)
      })
    })

    const finalDatasets: ChartDataset<'line'>[] = []

    if (lossData.some(v => v !== null)) {
      finalDatasets.push({
        label: 'Loss',
        data: lossData as any,
        borderColor: chartColor(5),
        backgroundColor: chartColor(5, 0.05),
        fill: true,
        borderWidth: 2,
        pointRadius: 0,
        yAxisID: 'y-loss',
        tension: 0.1,
      })
    }

    if (lrData.some(v => v !== null)) {
      finalDatasets.push({
        label: 'Learning Rate',
        data: lrData as any,
        borderColor: chartColor(1),
        backgroundColor: 'transparent',
        borderWidth: 1.5,
        borderDash: [5, 2],
        pointRadius: 0,
        yAxisID: 'y-lr',
        tension: 0.1,
      })
    }

    if (accuracyData.some(v => v !== null)) {
      finalDatasets.push({
        label: 'Accuracy',
        data: accuracyData as any,
        borderColor: chartColor(3),
        backgroundColor: chartColor(3, 0.05),
        fill: true,
        borderWidth: 2,
        pointRadius: 0,
        yAxisID: 'y-acc',
        tension: 0.1,
      })
    }

    sortedCustomKeys.forEach((key, i) => {
      finalDatasets.push({
        label: key,
        data: customMetricData[key] as any,
        borderColor: chartColor(CUSTOM_COLOR_OFFSET + i),
        backgroundColor: 'transparent',
        borderWidth: 1.5,
        borderDash: [3, 3],
        pointRadius: 0,
        yAxisID: 'y-loss', // Custom metrics share Loss axis by default
        tension: 0.1,
      })
    })

    return { 
      chartLabels: labels, 
      datasets: finalDatasets, 
      epochs,
      hasLoss: lossData.some(v => v !== null),
      hasLR: lrData.some(v => v !== null),
      hasAccuracy: accuracyData.some(v => v !== null),
      hasCustom: sortedCustomKeys.length > 0,
      customKeys: sortedCustomKeys
    }
  }, [records])

  if (datasets.length === 0) return null

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
        grid: { display: false },
        ticks: {
          color: 'hsl(215 20.2% 65.1%)',
          font: { size: 10 },
          maxRotation: 0,
          autoSkip: true,
          maxTicksLimit: 10,
        },
      },
      'y-loss': {
        type: 'linear' as const,
        display: hasLoss || hasCustom,
        position: 'left' as const,
        grid: { color: 'rgba(255, 255, 255, 0.05)' },
        ticks: { color: 'hsl(215 20.2% 65.1%)', font: { size: 10 } },
        title: { display: true, text: 'Loss', color: 'hsl(215 20.2% 65.1%)', font: { size: 10 } },
      },
      'y-lr': {
        type: 'linear' as const,
        display: hasLR,
        position: 'right' as const,
        grid: { display: false },
        ticks: { 
          color: 'hsl(215 20.2% 65.1%)', 
          font: { size: 10 },
          callback: (value: any) => value.toExponential(1)
        },
        title: { display: true, text: 'LR', color: 'hsl(215 20.2% 65.1%)', font: { size: 10 } },
      },
      'y-acc': {
        type: 'linear' as const,
        display: hasAccuracy,
        position: 'right' as const,
        min: 0,
        max: 100,
        grid: { display: false },
        ticks: { color: 'hsl(215 20.2% 65.1%)', font: { size: 10 }, callback: (v: any) => `${v}%` },
        title: { display: true, text: 'Accuracy', color: 'hsl(215 20.2% 65.1%)', font: { size: 10 } },
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
        callbacks: {
          label: (context: any) => {
            let label = context.dataset.label || ''
            if (label) label += ': '
            if (context.dataset.label === 'Learning Rate') {
              label += context.parsed.y.toExponential(2)
            } else {
              label += context.parsed.y.toFixed(4)
            }
            return label
          }
        }
      },
      annotation: {
        annotations: epochs.reduce((acc, e, idx) => {
          // Find approximate index of the step in our labels
          const stepLabel = `Step ${e.step}`
          const labelIndex = chartLabels.indexOf(stepLabel)
          if (labelIndex === -1) return acc

          acc[`epoch-${idx}`] = {
            type: 'line',
            xMin: labelIndex,
            xMax: labelIndex,
            borderColor: 'rgba(255, 255, 255, 0.2)',
            borderWidth: 1,
            borderDash: [4, 4],
            label: {
              display: true,
              content: `Epoch ${e.epoch}`,
              position: 'start',
              backgroundColor: 'rgba(0,0,0,0.5)',
              color: 'hsl(215 20.2% 65.1%)',
              font: { size: 9 },
              padding: 2,
            }
          }
          return acc
        }, {} as any)
      }
    },
  }

  return (
    <div className="chart-container">
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-2">
          <h2 className="text-lg font-semibold text-foreground">Training Metrics</h2>
          <ChartInfoTooltip content="Loss and learning rate plotted over optimizer steps. Epoch boundaries are shown as vertical dashed lines. Custom metrics from log_custom() are also displayed." />
        </div>
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
