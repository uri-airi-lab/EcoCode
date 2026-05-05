'use client'

import React from 'react'
import { Smartphone, Car, TreePine, Leaf } from 'lucide-react'
import { MetricCard } from '../MetricCard'

interface EnvironmentalImpactProps {
  totalEnergyWh: number
  totalCo2G?: number | null
  carbonIntensity?: number | null
  totalCost?: number | null
  currency?: string | null
  className?: string
}

/** Format grams with auto-scaling: sub-milligram → μg, sub-gram → mg, else g */
function formatCo2(grams: number): { value: string; unit: string } {
  if (grams === 0) return { value: '0.000', unit: 'g CO₂' }
  if (grams < 0.001)  return { value: (grams * 1_000_000).toFixed(2), unit: 'μg CO₂' }
  if (grams < 1)      return { value: (grams * 1000).toFixed(3),     unit: 'mg CO₂' }
  if (grams < 1000)   return { value: grams.toFixed(4),              unit: 'g CO₂' }
  return               { value: (grams / 1000).toFixed(4),           unit: 'kg CO₂' }
}

export function EnvironmentalImpact({ 
  totalEnergyWh, 
  totalCo2G, 
  totalCost, 
  currency,
  className = ""
}: EnvironmentalImpactProps) {
  // Use DB-stored CO2 if available and non-zero, otherwise fall back to world average (445 g/kWh)
  const co2G = (totalCo2G != null && totalCo2G > 0)
    ? totalCo2G
    : (totalEnergyWh / 1000) * 445

  const { value: co2Value, unit: co2Unit } = formatCo2(co2G)

  const equivalents = [
    {
      label: 'CO₂ Emissions',
      value: co2Value,
      unit: co2Unit,
      icon: Leaf,
    },
    {
      label: 'Car Travel',
      // avg car: 170 g CO2/km
      value: co2G >= 0.17 ? (co2G / 170).toFixed(3) : `${((co2G / 170) * 1000).toFixed(2)} m`,
      unit: co2G >= 0.17 ? 'km' : '',
      icon: Car,
    },
    {
      label: 'Trees Offset',
      // avg tree absorbs 22 kg CO2/year = ~60.27 g/day
      value: (co2G / (22000 / 365)).toFixed(6),
      unit: 'tree-days',
      icon: TreePine,
    },
    {
      label: 'Device Charges',
      // avg smartphone charge: 8.2 g CO2
      value: co2G >= 0.082
        ? (co2G / 8.2).toFixed(2)
        : `${((co2G / 8.2) * 100).toFixed(2)}%`,
      unit: co2G >= 0.082 ? 'charges' : 'of 1 charge',
      icon: Smartphone,
    },
  ]

  return (
    <div className={`grid grid-cols-2 lg:grid-cols-4 gap-3 ${className}`}>
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
