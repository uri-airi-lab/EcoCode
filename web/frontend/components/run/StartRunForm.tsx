'use client'

import React from 'react'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Switch } from '@/components/ui/switch'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import {
  AlertTriangle, Coins, FileCode, Folder, Globe, Key, MapPin,
  Plus, Rocket, Settings2, Tag, Timer, User,
} from 'lucide-react'
import { ProjectSummary } from '@/lib/api'
import { useWorkspace } from '@/components/layout/WorkspaceProvider'

export interface StartRunFormState {
  mode: 'existing' | 'new'
  setMode: (v: 'existing' | 'new') => void
  selectedProjectId: string
  setSelectedProjectId: (v: string) => void
  projectName: string
  setProjectName: (v: string) => void
  intensity: string
  setIntensity: (v: string) => void
  cost: string
  setCost: (v: string) => void
  currency: string
  setCurrency: (v: string) => void
  address: string
  setAddress: (v: string) => void
  useApiCost: boolean
  setUseApiCost: (v: boolean) => void
  useApiIntensity: boolean
  setUseApiIntensity: (v: boolean) => void
  outputFormat: string
  setOutputFormat: (v: string) => void
  outputFile: string
  setOutputFile: (v: string) => void
  interval: string
  setInterval: (v: string) => void
  collaborator: string
  setCollaborator: (v: string) => void
  runName: string
  setRunName: (v: string) => void
  missingConfig: boolean
  existingOnly: boolean
  projects: ProjectSummary[]
}

export function StartRunForm({
  mode, setMode,
  selectedProjectId, setSelectedProjectId,
  projectName, setProjectName,
  intensity, setIntensity,
  cost, setCost,
  address, setAddress,
  useApiCost, setUseApiCost,
  useApiIntensity, setUseApiIntensity,
  outputFormat, setOutputFormat,
  outputFile, setOutputFile,
  interval, setInterval,
  collaborator, setCollaborator,
  runName, setRunName,
  missingConfig,
  existingOnly,
  projects,
}: StartRunFormState) {
  const { currentWorkspace } = useWorkspace()

  return (
    <>
      <Tabs value={mode} onValueChange={(v) => setMode(v as any)} className="w-full mt-4">
        {!existingOnly && (
          <TabsList className="grid w-full grid-cols-2 bg-white/5 p-1 rounded-xl">
            <TabsTrigger value="existing" className="rounded-lg data-[state=active]:bg-primary data-[state=active]:text-primary-foreground">
              <Folder className="w-4 h-4 mr-2" />
              Existing Project
            </TabsTrigger>
            <TabsTrigger value="new" className="rounded-lg data-[state=active]:bg-primary data-[state=active]:text-primary-foreground">
              <Plus className="w-4 h-4 mr-2" />
              New Project
            </TabsTrigger>
          </TabsList>
        )}

        <TabsContent value="existing" className="space-y-4 mt-6">
          <div className="space-y-2">
            <Label>Select Project</Label>
            <Select value={selectedProjectId} onValueChange={setSelectedProjectId}>
              <SelectTrigger className="bg-white/5 border-white/10 h-12 rounded-xl">
                <SelectValue placeholder="Choose a project..." />
              </SelectTrigger>
              <SelectContent className="glass border-white/10">
                {projects.map(p => (
                  <SelectItem key={p.id} value={p.id}>{p.name}</SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          {missingConfig && selectedProjectId && (
            <div className="p-4 rounded-xl bg-amber-500/10 border border-amber-500/20 space-y-4">
              <div className="flex items-center gap-2 text-amber-400">
                <AlertTriangle className="w-4 h-4" />
                <span className="text-sm font-bold">Incomplete Settings</span>
              </div>
              <div className="space-y-4">
                <div className="flex items-center justify-between">
                  <Label className="flex flex-col gap-1">
                    <span className="text-xs font-bold uppercase tracking-widest">Auto-fetch Intensity</span>
                    <span className="text-[10px] text-muted-foreground font-normal">Use Electricity Maps API</span>
                  </Label>
                  <Switch checked={useApiIntensity} onCheckedChange={setUseApiIntensity} />
                </div>
                {useApiIntensity ? (
                  <div className="space-y-2">
                    <Label className="text-[10px] uppercase font-bold text-muted-foreground">Location / Address</Label>
                    <div className="relative">
                      <MapPin className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-primary/50" />
                      <Input placeholder="e.g. Paris, France" value={address} onChange={(e) => setAddress(e.target.value)} className="bg-background/50 border-white/10 pl-9" />
                    </div>
                  </div>
                ) : (
                  <div className="space-y-2">
                    <Label className="text-[10px] uppercase font-bold text-muted-foreground">Manual Intensity (g/kWh)</Label>
                    <Input type="number" min="0" value={intensity} onChange={(e) => setIntensity(e.target.value)} className="bg-background/50 border-white/10" />
                  </div>
                )}
                <div className="flex items-center justify-between pt-2 border-t border-white/5">
                  <Label className="flex flex-col gap-1">
                    <span className="text-xs font-bold uppercase tracking-widest">Auto-fetch Price</span>
                    <span className="text-[10px] text-muted-foreground font-normal">Use real-time market prices</span>
                  </Label>
                  <Switch checked={useApiCost} onCheckedChange={setUseApiCost} />
                </div>
                {!useApiCost && (
                  <div className="space-y-2">
                    <Label className="text-[10px] uppercase font-bold text-muted-foreground">Manual Cost per kWh</Label>
                    <Input type="number" min="0" value={cost} onChange={(e) => setCost(e.target.value)} className="bg-background/50 border-white/10" />
                  </div>
                )}
              </div>
            </div>
          )}
        </TabsContent>

        <TabsContent value="new" className="space-y-6 mt-6">
          <div className="space-y-2">
            <Label>Project Name</Label>
            <Input placeholder="e.g. my-awesome-app" value={projectName} onChange={(e) => setProjectName(e.target.value)} className="bg-white/5 border-white/10 h-12 rounded-xl" />
          </div>
          <div className="p-4 rounded-xl bg-primary/5 border border-primary/10 space-y-6">
            <div className="space-y-4">
              <div className="flex items-center justify-between">
                <Label className="flex items-center gap-2 font-bold">
                  <Globe className="w-4 h-4 text-primary" />
                  Carbon Intensity
                </Label>
                <Switch checked={useApiIntensity} onCheckedChange={setUseApiIntensity} />
              </div>
              {useApiIntensity ? (
                <div className="relative">
                  <MapPin className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-primary/50" />
                  <Input placeholder="Location (e.g. London, UK)" value={address} onChange={(e) => setAddress(e.target.value)} className="bg-white/5 border-white/10 pl-9 rounded-xl h-12" />
                </div>
              ) : (
                <Input type="number" min="0" value={intensity} onChange={(e) => setIntensity(e.target.value)} className="bg-white/5 border-white/10 rounded-xl h-12" placeholder="gCO2/kWh" />
              )}
            </div>
            <div className="space-y-4 pt-4 border-t border-white/5">
              <div className="flex items-center justify-between">
                <Label className="flex items-center gap-2 font-bold">
                  <Coins className="w-4 h-4 text-amber-400" />
                  Energy Cost
                </Label>
                <Switch checked={useApiCost} onCheckedChange={setUseApiCost} />
              </div>
              {!useApiCost && (
                <Input type="number" min="0" value={cost} onChange={(e) => setCost(e.target.value)} className="bg-white/5 border-white/10 rounded-xl h-12" placeholder="Rate per kWh" />
              )}
            </div>
          </div>
        </TabsContent>
      </Tabs>

      {/* Execution Parameters */}
      <div className="w-full mt-4 border border-white/10 rounded-xl overflow-hidden">
        <div className="py-3 px-4 bg-white/5 border-b border-white/10">
          <div className="flex items-center gap-2 text-sm font-bold">
            <Settings2 className="w-4 h-4 text-primary" />
            Execution Parameters
          </div>
        </div>
        <div className="pt-4 px-4 space-y-6 pb-6">
          <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
            <div className="space-y-2">
              <Label className="flex items-center gap-2 text-xs font-bold text-muted-foreground uppercase tracking-widest">
                <Tag className="w-3 h-3" />
                Custom Run Name
              </Label>
              <Input placeholder="Optional name" value={runName} onChange={(e) => setRunName(e.target.value)} className="bg-background/50 border-white/5 h-10 rounded-lg text-sm" />
            </div>
            <div className={`space-y-2 ${outputFormat === 'online' ? 'opacity-50 pointer-events-none' : ''}`}>
              <Label className="flex items-center gap-2 text-xs font-bold text-muted-foreground uppercase tracking-widest">
                <User className="w-3 h-3" />
                {outputFormat === 'online' ? 'Collaborator (Linked to Account)' : 'Collaborator'}
              </Label>
              <Input
                placeholder={outputFormat === 'online' ? 'Locked to account' : 'Your name'}
                value={outputFormat === 'online' ? '' : collaborator}
                onChange={(e) => setCollaborator(e.target.value)}
                className="bg-background/50 border-white/5 h-10 rounded-lg text-sm"
                disabled={outputFormat === 'online'}
              />
            </div>
          </div>

          <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
            <div className="space-y-2">
              <Label className="flex items-center gap-2 text-xs font-bold text-muted-foreground uppercase tracking-widest">
                <Timer className="w-3 h-3" />
                Interval (sec)
              </Label>
              <Input type="number" min="0.1" step="0.1" value={interval} onChange={(e) => setInterval(e.target.value)} className="bg-background/50 border-white/5 h-10 rounded-lg text-sm" />
            </div>
            <div className="space-y-2">
              <Label className="flex items-center gap-2 text-xs font-bold text-muted-foreground uppercase tracking-widest">
                <Rocket className="w-3 h-3" />
                Output Format
              </Label>
              <Select value={outputFormat} onValueChange={setOutputFormat}>
                <SelectTrigger className="bg-background/50 border-white/5 h-10 rounded-lg text-sm">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent className="glass border-white/10">
                  <SelectItem value="terminal">Terminal</SelectItem>
                  <SelectItem value="local">Local Database</SelectItem>
                  <SelectItem value="online">Online Dashboard</SelectItem>
                  <SelectItem value="csv">CSV File</SelectItem>
                  <SelectItem value="json">JSON File</SelectItem>
                  <SelectItem value="logfile">Log File</SelectItem>
                  <SelectItem value="prometheus">Prometheus</SelectItem>
                </SelectContent>
              </Select>
            </div>
          </div>

          {['csv', 'json', 'logfile'].includes(outputFormat) && (
            <div className="space-y-2 animate-in fade-in slide-in-from-top-2">
              <Label className="flex items-center gap-2 text-xs font-bold text-muted-foreground uppercase tracking-widest">
                <FileCode className="w-3 h-3" />
                Output File Path
              </Label>
              <Input placeholder="e.g. ./results.csv" value={outputFile} onChange={(e) => setOutputFile(e.target.value)} className="bg-background/50 border-white/5 h-10 rounded-lg text-sm" />
            </div>
          )}

          {outputFormat === 'online' && (
            <div className="p-4 rounded-xl bg-emerald-500/10 border border-emerald-500/20 space-y-3 animate-in fade-in slide-in-from-top-2">
              <div className="flex items-center gap-2 text-emerald-400">
                <Key className="w-4 h-4" />
                <span className="text-sm font-bold">Authentication Required</span>
              </div>
              <p className="text-[11px] text-emerald-200/70 leading-relaxed">
                To push data to this dashboard, you must provide your API key using the <code>--api-key</code> parameter:
              </p>
              <div className="flex flex-col gap-2">
                <code className="text-[10px] bg-black/40 p-2 rounded border border-white/5 text-emerald-300 break-all font-mono">
                  --api-key "ecocode_..."
                </code>
                <a
                  href={`/${currentWorkspace?.slug}/settings/developer`}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="text-[10px] text-emerald-400 hover:text-emerald-300 font-bold flex items-center gap-1 transition-colors"
                >
                  Get your API key in Settings →
                </a>
              </div>
            </div>
          )}
        </div>
      </div>
    </>
  )
}
