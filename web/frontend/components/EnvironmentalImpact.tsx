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
      bg: 'bg-amber-400/10'
    },
    {
      label: 'Trees Absorption',
      description: 'Equivalent to 1 day of absorption',
      value: (co2G / (21000 / 365)).toFixed(2),
      unit: 'Trees',
      icon: TreePine,
      color: 'text-emerald-400',
      bg: 'bg-emerald-400/10'
    },
    {
      label: 'Car Distance',
      description: 'Average passenger vehicle',
      value: (co2G / 120).toFixed(2),
      unit: 'km',
      icon: Car,
      color: 'text-blue-400',
      bg: 'bg-blue-400/10'
    },
    {
      label: 'Smartphone Charges',
      description: 'Full battery cycles',
      value: (co2G / 8.2).toFixed(1),
      unit: 'Charges',
      icon: Smartphone,
      color: 'text-purple-400',
      bg: 'bg-purple-400/10'
    }
  ]

  return (
    <div className="space-y-6">
      <div className="flex flex-col md:flex-row md:items-center justify-between p-6 rounded-2xl bg-gradient-to-br from-white/5 to-transparent border border-white/10 shadow-xl backdrop-blur-sm">
        <div className="flex items-center gap-4">
          <div className="p-3 rounded-2xl bg-emerald-500/20 shadow-lg shadow-emerald-500/10">
            <Globe className="w-6 h-6 text-emerald-400" />
          </div>
          <div>
            <p className="text-xs uppercase tracking-widest text-muted-foreground font-bold mb-1">Carbon Footprint</p>
            <div className="flex items-baseline gap-2">
              <span className="text-4xl font-black text-foreground tracking-tighter">
                {co2G > 1000 ? (co2G / 1000).toFixed(2) : co2G.toFixed(0)}
              </span>
              <span className="text-lg font-medium text-muted-foreground">
                {co2G > 1000 ? 'kg CO₂' : 'g CO₂'}
              </span>
            </div>
          </div>
        </div>
        
        {carbonIntensity && (
          <div className="mt-4 md:mt-0 flex items-center gap-2 px-4 py-2 rounded-xl bg-white/5 border border-white/10">
            <Zap className="w-4 h-4 text-amber-400" />
            <div className="text-right">
              <p className="text-[10px] text-muted-foreground uppercase font-bold">Intensity used</p>
              <p className="text-sm font-mono font-bold text-foreground">{carbonIntensity} <span className="text-[10px] opacity-60">g/kWh</span></p>
            </div>
          </div>
        )}
      </div>

      <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
        {metrics.map((m) => (
          <div key={m.label} className="metric-card p-5 group hover:bg-white/5 transition-all duration-300">
            <div className="flex items-center gap-4 mb-4">
              <div className={`p-3 rounded-xl ${m.bg} group-hover:scale-110 transition-transform`}>
                <m.icon className={`w-5 h-5 ${m.color}`} />
              </div>
              <div className="flex-1">
                <p className="text-xs font-bold text-muted-foreground uppercase tracking-widest leading-tight">{m.label}</p>
                <p className="text-[10px] text-muted-foreground/60 leading-tight">{m.description}</p>
              </div>
            </div>
            <div className="flex items-baseline gap-1.5">
              <span className="text-2xl font-black text-foreground">{m.value}</span>
              <span className="text-xs font-medium text-muted-foreground">{m.unit}</span>
            </div>
          </div>
        ))}
      </div>
    </div>
  )
}
