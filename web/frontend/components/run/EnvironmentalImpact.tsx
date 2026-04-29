'use client'

import React from 'react'
import { Smartphone, Zap, Car, TreePine, Globe, Coins } from 'lucide-react'

import { MetricCard } from '../MetricCard'

interface EnvironmentalImpactProps {
  totalEnergyWh: number
  totalCo2G?: number | null
  carbonIntensity?: number | null
  totalCost?: number | null
  currency?: string | null
  className?: string
}

export function EnvironmentalImpact({ 
  totalEnergyWh, 
  totalCo2G, 
  totalCost, 
  currency,
  className = ""
}: EnvironmentalImpactProps) {
  // Use DB-stored CO2 if available, otherwise fallback to world average calculation
  const co2G = totalCo2G != null ? totalCo2G : (totalEnergyWh / 1000) * 445

  const equivalents = [
    {
      label: 'Trees Offset',
      value: (co2G / (22000 / 365)).toFixed(2),
      unit: 'Trees',
      icon: TreePine,
    },
    {
      label: 'Car Travel',
      value: (co2G / 170).toFixed(2),
      unit: 'km',
      icon: Car,
    },
    {
      label: 'Device Charges',
      value: (co2G / 8.2).toFixed(0),
      unit: 'Charges',
      icon: Smartphone,
    }
  ]

  return (
    <div className={`grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4 ${className}`}>
      {equivalents.map((m) => (
        <MetricCard
          key={m.label}
          icon={m.icon}
          label={m.label}
          value={m.value}
          unit={m.unit}
        />
      ))}
    </div>
  )
}


