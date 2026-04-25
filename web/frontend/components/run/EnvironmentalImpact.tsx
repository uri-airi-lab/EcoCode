'use client'

import React from 'react'
import { Smartphone, Zap, Car, TreePine, Globe, Coins } from 'lucide-react'

interface EnvironmentalImpactProps {
  totalEnergyWh: number
  totalCo2G?: number | null
  carbonIntensity?: number | null
  totalCost?: number | null
  currency?: string | null
}

export function EnvironmentalImpact({ totalEnergyWh, totalCo2G, carbonIntensity, totalCost, currency }: EnvironmentalImpactProps) {
  // Use DB-stored CO2 if available, otherwise fallback to world average calculation
  // World average: 494 gCO2/kWh
  const co2G = totalCo2G != null ? totalCo2G : (totalEnergyWh / 1000) * 494

  const metrics = [
    {
      label: 'Financial Impact',
      description: 'Estimated energy cost',
      value: totalCost != null ? totalCost.toFixed(4) : '--',
      unit: currency || 'USD',
      icon: Coins,
      color: 'text-amber-400',
    },
    {
      label: 'Trees Absorption',
      description: '1 day of absorption',
      value: (co2G / (21000 / 365)).toFixed(2),
      unit: 'Trees',
      icon: TreePine,
      color: 'text-emerald-400',
    },
    {
      label: 'Car Distance',
      description: 'Passenger vehicle',
      value: (co2G / 120).toFixed(2),
      unit: 'km',
      icon: Car,
      color: 'text-blue-400',
    },
    {
      label: 'Smartphone Charges',
      description: 'Full battery cycles',
      value: (co2G / 8.2).toFixed(1),
      unit: 'Charges',
      icon: Smartphone,
      color: 'text-purple-400',
    }
  ]

  return (
    <div className="space-y-4">
      {/* Total Energy Card */}
      <div className="p-5 rounded-xl border border-border/40 bg-card/50">
        <div className="flex items-center gap-3">
          <Zap className="w-5 h-5 text-primary" />
          <div className="flex-1">
            <p className="text-xs text-muted-foreground uppercase tracking-wider mb-1">
              Total Energy Consumption
            </p>
            <div className="flex items-baseline gap-2">
              <span className="text-2xl font-bold text-foreground">
                {Number(totalEnergyWh.toFixed(2)).toLocaleString()}
              </span>
              <span className="text-sm text-muted-foreground">Wh</span>
            </div>
          </div>
        </div>
      </div>

      {/* Carbon Footprint Card */}
      <div className="p-5 rounded-xl border border-border/40 bg-card/50">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <Globe className="w-5 h-5 text-emerald-400" />
            <div>
              <p className="text-xs text-muted-foreground uppercase tracking-wider mb-1">Carbon Footprint</p>
              <div className="flex items-baseline gap-2">
                <span className="text-2xl font-bold text-foreground">
                  {co2G > 1000 ? (co2G / 1000).toFixed(2) : co2G.toFixed(0)}
                </span>
                <span className="text-sm text-muted-foreground">
                  {co2G > 1000 ? 'kg CO₂' : 'g CO₂'}
                </span>
              </div>
            </div>
          </div>
          
          {carbonIntensity && (
            <div className="flex items-center gap-2 px-3 py-2 rounded-lg border border-border/40 bg-card/30">
              <Zap className="w-4 h-4 text-amber-400" />
              <div>
                <p className="text-[10px] text-muted-foreground uppercase">Intensity</p>
                <p className="text-sm font-semibold text-foreground">{carbonIntensity} <span className="text-xs text-muted-foreground">g/kWh</span></p>
              </div>
            </div>
          )}
        </div>
      </div>

      {/* Separator */}
      <div className="relative">
        <div className="absolute inset-0 flex items-center">
          <div className="w-full border-t border-border/40"></div>
        </div>
        <div className="relative flex justify-center text-xs uppercase">
          <span className="bg-background px-2 text-muted-foreground">Environmental Equivalents</span>
        </div>
      </div>

      {/* Equivalents Grid */}
      <div className="grid grid-cols-2 gap-3">
        {metrics.map((m) => (
          <div key={m.label} className="p-4 rounded-xl border border-border/40 bg-card/50">
            <div className="flex items-center gap-2 mb-3">
              <m.icon className={`w-4 h-4 ${m.color}`} />
              <div className="flex-1">
                <p className="text-xs font-medium text-muted-foreground uppercase tracking-wider">{m.label}</p>
                <p className="text-[10px] text-muted-foreground/70">{m.description}</p>
              </div>
            </div>
            <div className="flex items-baseline gap-1.5">
              <span className="text-xl font-bold text-foreground">{m.value}</span>
              <span className="text-xs text-muted-foreground">{m.unit}</span>
            </div>
          </div>
        ))}
      </div>
    </div>
  )
}
