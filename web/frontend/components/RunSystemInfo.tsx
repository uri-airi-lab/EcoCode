'use client'

import { Run } from '@/lib/mock-data'

interface RunSystemInfoProps {
  run: Run
}

export function RunSystemInfo({ run }: RunSystemInfoProps) {
  const specs = [
    {
      label: 'Total Records',
      value: run.records.length,
    },
    {
      label: 'Avg CPU Usage',
      value: `${run.avgCpuUsage.toFixed(1)}%`,
    },
    {
      label: 'Avg GPU Usage',
      value: `${run.avgGpuUsage.toFixed(1)}%`,
    },
    {
      label: 'Total Energy',
      value: `${Number(run.totalEnergy.toFixed(2))} Wh`,
    },
    // {
    //   label: 'Carbon Footprint',
    //   value: `${Number(run.carbonFootprint.toFixed(2))} g CO₂`,
    // },
    // {
    //   label: 'Water Consumption',
    //   value: `${Number(run.waterConsumption.toFixed(2))} mL`,
    // },
  ]

  return (
    <div className="chart-container">
      <h3 className="text-lg font-semibold text-foreground mb-4">Run Details</h3>
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
        {specs.map((spec) => (
          <div
            key={spec.label}
            className="p-4 rounded-lg bg-white/5 border border-white/10"
          >
            <p className="text-xs text-muted-foreground mb-1">{spec.label}</p>
            <p className="text-lg font-semibold text-foreground">
              {spec.value}
            </p>
          </div>
        ))}
      </div>
    </div>
  )
}
