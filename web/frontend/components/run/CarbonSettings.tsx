'use client'

import React, { useState, useEffect } from 'react'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Button } from '@/components/ui/button'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Slider } from '@/components/ui/slider'
import { Globe, MapPin, Zap, RefreshCw, Key, Save, Coins, Landmark, TrendingUp } from 'lucide-react'
import { Switch } from '@/components/ui/switch'
import { fetchCarbonConfig, updateCarbonConfig, refreshCarbonIntensity, fetchCarbonRegions, CarbonConfig, CarbonRegion } from '@/lib/api'
import { toast } from 'sonner'

interface CarbonSettingsProps {
  projectId: number
}

export function CarbonSettings({ projectId }: CarbonSettingsProps) {
  const [config, setConfig] = useState<CarbonConfig | null>(null)
  const [regions, setRegions] = useState<CarbonRegion[]>([])
  const [loading, setLoading] = useState(true)
  const [saving, setSaving] = useState(false)
  const [refreshing, setRefreshing] = useState(false)

  const [address, setAddress] = useState('')
  const [apiKey, setApiKey] = useState('')
  const [pue, setPue] = useState(1.0)
  const [countryCode, setCountryCode] = useState('WORLD')
  const [manualCost, setManualCost] = useState<string>('0.00')
  const [currency, setCurrency] = useState('USD')
  const [useApiForCost, setUseApiForCost] = useState(false)

  useEffect(() => {
    async function load() {
      try {
        setLoading(true)
        const [cfg, regs] = await Promise.all([
          fetchCarbonConfig(projectId),
          fetchCarbonRegions()
        ])
        setConfig(cfg)
        setRegions(regs)
        setAddress(cfg.address || '')
        setPue(cfg.pue_factor)
        setCountryCode(cfg.country_code)
        setManualCost(cfg.manual_cost_per_kwh?.toString() || '0.00')
        setCurrency(cfg.manual_currency || 'USD')
        setUseApiForCost(cfg.use_api_for_cost)
      } catch (err) {
        toast.error('Failed to load carbon settings')
      } finally {
        setLoading(false)
      }
    }
    load()
  }, [projectId])

  const handleSave = async () => {
    try {
      setSaving(true)
      const updated = await updateCarbonConfig(projectId, {
        address: address,
        country_code: countryCode,
        pue_factor: pue,
        manual_cost_per_kwh: parseFloat(manualCost) || 0,
        manual_currency: currency,
        use_api_for_cost: useApiForCost,
        ...(apiKey && { electricity_maps_api_key: apiKey }),
      })
      setConfig(updated)
      setApiKey('')
      toast.success('Carbon settings updated')
    } catch (err) {
      toast.error('Failed to update settings')
    } finally {
      setSaving(false)
    }
  }

  const handleRefresh = async () => {
    try {
      setRefreshing(true)
      const updated = await refreshCarbonIntensity(projectId)
      setConfig(updated)
      toast.success(`Intensity refreshed: ${updated.resolved_intensity_g_per_kwh} g/kWh`)
    } catch (err: any) {
      const msg = err.message || 'Failed to refresh intensity'
      toast.error(msg)
    } finally {
      setRefreshing(false)
    }
  }

  if (loading || !config) return (
    <Card className="w-full bg-card/50 backdrop-blur-md border-white/10 animate-pulse">
      <div className="h-[400px]" />
    </Card>
  )

  return (
    <Card className="w-full bg-card/50 backdrop-blur-md border-white/10 overflow-hidden">
      <CardHeader className="pb-4">
        <div className="flex items-center gap-2">
          <div className="p-2 rounded-lg bg-primary/20">
            <Zap className="w-5 h-5 text-primary" />
          </div>
          <div>
            <CardTitle className="text-xl">Carbon & Energy Settings</CardTitle>
            <CardDescription className="text-xs">
              Configure your location and power usage effectiveness for accurate CO₂ calculations.
            </CardDescription>
          </div>
        </div>
      </CardHeader>
      <CardContent className="space-y-8">
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-8">
          {/* Location & Real-time */}
          <div className="space-y-6">
            <div className="space-y-3">
              <Label htmlFor="address" className="text-sm font-semibold flex items-center gap-2">
                <MapPin className="w-4 h-4 text-primary" />
                Address / Location
              </Label>
              <div className="relative">
                <Input
                  id="address"
                  placeholder="e.g. Paris, France"
                  className="bg-background/30 border-white/5 focus:border-primary/50 transition-all"
                  value={address}
                  onChange={(e) => setAddress(e.target.value)}
                />
              </div>
              <p className="text-[10px] text-muted-foreground italic">
                Geocodes to lat/lon for Electricity Maps lookup.
              </p>
            </div>

            <div className="space-y-3">
              <Label htmlFor="api-key" className="text-sm font-semibold flex items-center gap-2">
                <Key className="w-4 h-4 text-primary" />
                Electricity Maps API Key (Optional)
              </Label>
              <div className="relative">
                <Input
                  id="api-key"
                  type="password"
                  placeholder="••••••••••••••••"
                  className="bg-background/30 border-white/5 focus:border-primary/50 transition-all"
                  value={apiKey}
                  onChange={(e) => setApiKey(e.target.value)}
                />
              </div>
              <p className="text-[10px] text-muted-foreground italic">
                Required for real-time hourly intensity data.
              </p>
            </div>
          </div>

          {/* Static Fallback & PUE */}
          <div className="space-y-6">
            <div className="space-y-3">
              <Label className="text-sm font-semibold flex items-center gap-2">
                <Globe className="w-4 h-4 text-primary" />
                Static Fallback Region
              </Label>
              <Select 
                value={countryCode} 
                onValueChange={setCountryCode}
              >
                <SelectTrigger className="bg-background/30 border-white/5 focus:border-primary/50 transition-all">
                  <SelectValue placeholder="Select region" />
                </SelectTrigger>
                <SelectContent className="bg-popover/90 backdrop-blur-xl border-white/10 max-h-[300px]">
                  {regions.map(r => (
                    <SelectItem key={r.code} value={r.code} className="hover:bg-primary/10">
                      {r.name} <span className="text-muted-foreground ml-2 text-[10px]">({r.intensity_g_per_kwh} g/kWh)</span>
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>

            <div className="space-y-5">
              <div className="flex justify-between items-center">
                <Label className="text-sm font-semibold flex items-center gap-2">
                  <Zap className="w-4 h-4 text-primary" />
                  PUE (Power Usage Effectiveness)
                </Label>
                <span className="text-xs font-mono font-bold text-primary bg-primary/10 px-2 py-1 rounded-md border border-primary/20">
                  {pue.toFixed(2)}
                </span>
              </div>
              <Slider
                value={[pue]}
                min={1.0}
                max={3.0}
                step={0.01}
                onValueChange={([val]) => setPue(val)}
                className="py-4"
              />
              <div className="flex justify-between text-[10px] font-medium text-muted-foreground uppercase tracking-tighter">
                <span className="flex flex-col gap-1"><span>1.0</span><span className="text-[8px] opacity-60">Workstation</span></span>
                <span className="flex flex-col items-center gap-1"><span>1.58</span><span className="text-[8px] opacity-60 text-center">Avg Cloud</span></span>
                <span className="flex flex-col items-end gap-1"><span>3.0</span><span className="text-[8px] opacity-60">Legacy DC</span></span>
              </div>
            </div>
          </div>
        </div>

        {/* Cost Settings Section */}
        <div className="pt-6 border-t border-white/5 space-y-6">
          <div className="flex items-center gap-2">
            <div className="p-2 rounded-lg bg-amber-500/20">
              <Coins className="w-5 h-5 text-amber-400" />
            </div>
            <div>
              <h3 className="text-lg font-semibold text-foreground">Energy Cost Tracking</h3>
              <p className="text-xs text-muted-foreground">Define your billing rates for accurate financial reporting.</p>
            </div>
          </div>

          <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
            <div className="space-y-3">
              <Label className="text-sm font-semibold flex items-center gap-2">
                <Landmark className="w-4 h-4 text-amber-400" />
                Billing Currency
              </Label>
              <Select value={currency} onValueChange={setCurrency}>
                <SelectTrigger className="bg-background/30 border-white/5 focus:border-amber-400/50 transition-all">
                  <SelectValue placeholder="Select currency" />
                </SelectTrigger>
                <SelectContent className="bg-popover/90 backdrop-blur-xl border-white/10">
                  <SelectItem value="USD">USD ($)</SelectItem>
                  <SelectItem value="EUR">EUR (€)</SelectItem>
                  <SelectItem value="GBP">GBP (£)</SelectItem>
                  <SelectItem value="JPY">JPY (¥)</SelectItem>
                  <SelectItem value="CHF">CHF (₣)</SelectItem>
                  <SelectItem value="CAD">CAD ($)</SelectItem>
                </SelectContent>
              </Select>
            </div>

            <div className="space-y-3">
              <Label className="text-sm font-semibold flex items-center gap-2">
                <TrendingUp className="w-4 h-4 text-amber-400" />
                Cost per kWh
              </Label>
              <div className="relative">
                <Input
                  type="number"
                  step="0.0001"
                  className="bg-background/30 border-white/5 focus:border-amber-400/50 transition-all pl-9"
                  value={manualCost}
                  disabled={useApiForCost}
                  onChange={(e) => setManualCost(e.target.value)}
                />
                <span className="absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground font-mono text-xs">
                  {currency === 'EUR' ? '€' : currency === 'GBP' ? '£' : '$'}
                </span>
              </div>
              <p className="text-[10px] text-muted-foreground italic">
                {useApiForCost ? 'Managed by API' : 'Residential or commercial rate.'}
              </p>
            </div>

            <div className="flex flex-col justify-end">
              <div className="flex items-center justify-between p-4 rounded-xl bg-white/5 border border-white/10 h-[52px]">
                <div className="space-y-0.5">
                  <Label className="text-xs font-bold uppercase tracking-widest cursor-pointer" htmlFor="api-cost-toggle">
                    Auto-fetch Price
                  </Label>
                  <p className="text-[9px] text-muted-foreground">Use Electricity Maps Market Price</p>
                </div>
                <Switch 
                  id="api-cost-toggle"
                  checked={useApiForCost}
                  onCheckedChange={setUseApiForCost}
                />
              </div>
            </div>
          </div>
        </div>

        <div className="flex justify-end">
          <Button 
            onClick={handleSave} 
            disabled={saving}
            className="w-full md:w-auto h-12 px-10 rounded-xl bg-primary text-primary-foreground font-bold shadow-lg shadow-primary/20 hover:scale-[1.02] active:scale-[0.98] transition-all"
          >
            {saving ? <RefreshCw className="w-5 h-5 animate-spin mr-2" /> : <Save className="w-5 h-5 mr-2" />}
            Save Project Configuration
          </Button>
        </div>

        {config && (
          <div className="relative group overflow-hidden p-6 rounded-2xl bg-gradient-to-br from-primary/10 via-primary/5 to-transparent border border-primary/20 shadow-2xl shadow-primary/5">
            <div className="relative z-10 flex flex-col md:flex-row md:items-center justify-between gap-6">
              <div className="flex items-center gap-4">
                <div className="p-3 rounded-2xl bg-primary/20 shadow-inner">
                  <Globe className="w-6 h-6 text-primary animate-pulse" />
                </div>
                <div className="space-y-1">
                  <p className="text-xs uppercase tracking-widest text-muted-foreground font-bold">Current Carbon Intensity</p>
                  <div className="flex items-baseline gap-2">
                    <span className="text-3xl font-black text-foreground tabular-nums tracking-tight">
                      {config.resolved_intensity_g_per_kwh}
                    </span>
                    <span className="text-sm font-medium text-muted-foreground">gCO₂/kWh</span>
                  </div>
                  <div className="flex items-center gap-2 text-[10px] font-medium">
                    <span className={`px-1.5 py-0.5 rounded uppercase border ${
                      config.intensity_source.startsWith('api') 
                        ? 'bg-emerald-400/10 text-emerald-400 border-emerald-400/20' 
                        : 'bg-amber-400/10 text-amber-400 border-amber-400/20'
                    }`}>
                      {config.intensity_source.replace('_', ' ')}
                    </span>
                    {config.latitude && config.longitude && (
                      <span className="text-muted-foreground/60">
                        📍 {config.latitude.toFixed(4)}, {config.longitude.toFixed(4)}
                      </span>
                    )}
                    {config.intensity_fetched_at && (
                      <span className="text-muted-foreground/60">
                        Updated {new Date(config.intensity_fetched_at).toLocaleTimeString()}
                      </span>
                    )}
                  </div>
                </div>
              </div>
              <Button 
                variant="outline" 
                onClick={handleRefresh} 
                disabled={refreshing || !config.has_api_key}
                className="h-12 px-6 gap-2 rounded-xl border-primary/20 hover:bg-primary/10 hover:border-primary/40 transition-all group"
              >
                <RefreshCw className={`w-4 h-4 transition-transform duration-700 ${refreshing ? 'animate-spin' : 'group-hover:rotate-180'}`} />
                <span className="font-semibold">Refresh Real-time</span>
              </Button>
            </div>
            {/* Decoration */}
            <div className="absolute -right-12 -bottom-12 w-48 h-48 bg-primary/10 rounded-full blur-3xl" />
          </div>
        )}
      </CardContent>
    </Card>
  )
}
