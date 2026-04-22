'use client'

import React from 'react'
import { Smartphone, Zap, Car, Lightbulb } from 'lucide-react'

interface EnvironmentalImpactProps {
  totalEnergyWh: number
}

export function EnvironmentalImpact({ totalEnergyWh }: EnvironmentalImpactProps) {
  // Equivalents
  // Smartphone charge: ~10Wh
  // EV (Tesla Model 3): ~150Wh per km
  // LED Bulb (9W): 9Wh per hour
  
  const metrics = [
    {
      label: 'Smartphone Charges',
      value: (totalEnergyWh / 10).toFixed(1),
      icon: Smartphone,
      color: 'text-blue-400',
      bg: 'bg-blue-400/10'
    },
    {
      label: 'EV Kilometers',
      value: (totalEnergyWh / 150).toFixed(2),
      icon: Car,
      color: 'text-emerald-400',
      bg: 'bg-emerald-400/10'
    },
    {
      label: 'LED Bulb Hours',
      value: (totalEnergyWh / 9).toFixed(1),
      icon: Lightbulb,
      color: 'text-amber-400',
      bg: 'bg-amber-400/10'
    }
  ]

  return (
    <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
      {metrics.map((m) => (
        <div key={m.label} className="metric-card flex items-center gap-4 py-4">
          <div className={`p-3 rounded-lg ${m.bg} backdrop-blur-sm`}>
            <m.icon className={`w-5 h-5 ${m.color}`} />
          </div>
          <div>
            <p className="text-2xl font-bold text-foreground">{m.value}</p>
            <p className="text-xs text-muted-foreground uppercase tracking-wider">{m.label}</p>
          </div>
        </div>
      ))}
    </div>
  )
}
