'use client'

import React, { useState, useEffect } from 'react'
import { usePathname } from 'next/navigation'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
  DialogFooter,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Switch } from '@/components/ui/switch'
import { Rocket, Copy, Check, AlertTriangle, Zap, Coins, Plus, Folder, MapPin, Globe, Settings2, User, Timer, FileCode, Tag, Key } from 'lucide-react'
import { ProjectSummary, createProject, fetchCarbonConfig } from '@/lib/api-client'
import { useWorkspace } from '@/components/WorkspaceProvider'
import { toast } from 'sonner'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Accordion, AccordionContent, AccordionItem, AccordionTrigger } from '@/components/ui/accordion'

interface StartRunModalProps {
  projects: ProjectSummary[]
  onProjectCreated?: () => void
  defaultProjectId?: string
}

export function StartRunModal({ projects, onProjectCreated, defaultProjectId }: StartRunModalProps) {
  const { currentWorkspace } = useWorkspace()
  const [open, setOpen] = useState(false)
  const [mode, setMode] = useState<'existing' | 'new'>('existing')
  const [selectedProjectId, setSelectedProjectId] = useState<string>('')
  const [projectName, setProjectName] = useState('')
  const [intensity, setIntensity] = useState('494')
  const [cost, setCost] = useState('0.15')
  const [currency, setCurrency] = useState('USD')
  const [address, setAddress] = useState('')
  const [useApiCost, setUseApiCost] = useState(false)
  const [useApiIntensity, setUseApiIntensity] = useState(false)
  
  // Advanced Options
  const [outputFormat, setOutputFormat] = useState('terminal')
  const [outputFile, setOutputFile] = useState('')
  const [interval, setInterval] = useState('1.0')
  const [collaborator, setCollaborator] = useState('')
  const [runName, setRunName] = useState('')
  
  const pathname = usePathname()
  const workspaceId = currentWorkspace?.id ?? null

  useEffect(() => {
    if (defaultProjectId) {
      setSelectedProjectId(defaultProjectId)
      setMode('existing')
    }
  }, [defaultProjectId])

  const [isCreating, setIsCreating] = useState(false)
  const [copied, setCopied] = useState(false)
  const [missingConfig, setMissingConfig] = useState(false)

  // When a project is selected, check its config
  useEffect(() => {
    if (selectedProjectId && mode === 'existing') {
        const checkConfig = async () => {
            try {
                const config = await fetchCarbonConfig(Number(selectedProjectId))
                setMissingConfig((!config.manual_cost_per_kwh || config.manual_cost_per_kwh === 0) && !config.use_api_for_cost)
                
                if (config.manual_cost_per_kwh) setCost(config.manual_cost_per_kwh.toString())
                setIntensity(config.resolved_intensity_g_per_kwh.toString())
                if (config.manual_currency) setCurrency(config.manual_currency)
                if (config.address) setAddress(config.address)
                setUseApiCost(config.use_api_for_cost)
                setUseApiIntensity(config.intensity_source === 'api')
            } catch (err) {
                console.error('Failed to check project config:', err)
            }
        }
        checkConfig()
    }
  }, [selectedProjectId, mode])

  const generateCommand = () => {
    const project = mode === 'new' ? projectName : projects.find(p => p.id === selectedProjectId)?.name
    if (!project) return '...'

    let cmd = `ecocode --project "${project}"`
    
    if (runName) cmd += ` --name "${runName}"`
    if (collaborator && outputFormat !== 'online') cmd += ` --collaborator "${collaborator}"`
    if (outputFormat !== 'terminal') cmd += ` --output ${outputFormat}`
    if (outputFile && ['csv', 'json', 'logfile'].includes(outputFormat)) cmd += ` --file "${outputFile}"`
    if (interval !== '1.0') cmd += ` --interval ${interval}`

    if (mode === 'new' || (selectedProjectId && missingConfig)) {
        if (useApiIntensity && address) {
            cmd += ` --address "${address}"`
        } else if (!useApiIntensity) {
            cmd += ` --intensity ${intensity}`
        }

        if (useApiCost) {
            cmd += ` --use-api-cost`
        } else {
            cmd += ` --price ${cost}`
        }
    }
    
    if (outputFormat === 'online') {
        cmd += ` --api-key "ecocode_your_key_here"`
    }
    
    cmd += ` "<your-command>"`
    return cmd
  }

  const handleCopy = () => {
    const project = mode === 'new' ? projectName : projects.find(p => p.id === selectedProjectId)?.name
    if (!project) return

    const intensityVal = parseFloat(intensity)
    const costVal = parseFloat(cost)
    const intervalVal = parseFloat(interval)

    if (!useApiIntensity && intensityVal < 0) {
        toast.error('Intensity cannot be negative')
        return
    }
    if (!useApiCost && costVal < 0) {
        toast.error('Cost cannot be negative')
        return
    }
    if (intervalVal <= 0) {
        toast.error('Interval must be greater than 0')
        return
    }

    const cmd = generateCommand()
    if (cmd === '...') return

    navigator.clipboard.writeText(cmd)
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
    toast.success('Command copied to clipboard!')
  }

  const handleCreateAndCopy = async () => {
    if (!projectName) {
        toast.error('Project name is required')
        return
    }
    if (useApiIntensity && !address) {
        toast.error('Address is required for API intensity')
        return
    }

    const intensityVal = parseFloat(intensity)
    const costVal = parseFloat(cost)
    const intervalVal = parseFloat(interval)

    if (!useApiIntensity && intensityVal < 0) {
        toast.error('Intensity cannot be negative')
        return
    }
    if (!useApiCost && costVal < 0) {
        toast.error('Cost cannot be negative')
        return
    }
    if (intervalVal <= 0) {
        toast.error('Interval must be greater than 0')
        return
    }

    if (!workspaceId) {
        toast.error('Workspace not identified')
        return
    }

    try {
        setIsCreating(true)
        await createProject(projectName, workspaceId, { 
            intensity: useApiIntensity ? undefined : intensityVal, 
            cost: useApiCost ? undefined : costVal, 
            currency,
            address: address || undefined,
            use_api_for_cost: useApiCost
        })
        handleCopy()
        if (onProjectCreated) onProjectCreated()
        setOpen(false) 
    } catch (err) {
        const errorMsg = err instanceof Error ? err.message : 'An error occurred'
        toast.error(`Failed to create project: ${errorMsg}`)
    } finally {
        setIsCreating(false)
    }
  }

  const selectedProject = projects.find(p => p.id === selectedProjectId)

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button className="bg-primary hover:bg-primary/90 text-primary-foreground font-bold rounded-xl shadow-lg shadow-primary/20 transition-all hover:scale-105 active:scale-95 px-6">
          <Rocket className="w-4 h-4 mr-2" />
          Start New Run
        </Button>
      </DialogTrigger>
      <DialogContent className="sm:max-w-[600px] bg-background/80 backdrop-blur-2xl border-white/10 glass shadow-2xl max-h-[90vh] overflow-y-auto custom-scrollbar">
        <DialogHeader>
          <DialogTitle className="text-2xl font-bold flex items-center gap-2">
            <Zap className="text-primary w-6 h-6" />
            Prepare a New Run
          </DialogTitle>
          <DialogDescription>
            Configure your project settings and execution parameters.
          </DialogDescription>
        </DialogHeader>

        <Tabs value={mode} onValueChange={(v) => setMode(v as any)} className="w-full mt-4">
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
                                <Input 
                                    placeholder="e.g. Paris, France" 
                                    value={address} 
                                    onChange={(e) => setAddress(e.target.value)}
                                    className="bg-background/50 border-white/10 pl-9"
                                />
                            </div>
                        </div>
                    ) : (
                        <div className="space-y-2">
                            <Label className="text-[10px] uppercase font-bold text-muted-foreground">Manual Intensity (g/kWh)</Label>
                            <Input 
                                type="number" 
                                min="0"
                                value={intensity} 
                                onChange={(e) => setIntensity(e.target.value)}
                                className="bg-background/50 border-white/10"
                            />
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
                            <Input 
                                type="number" 
                                min="0"
                                value={cost} 
                                onChange={(e) => setCost(e.target.value)}
                                className="bg-background/50 border-white/10"
                            />
                        </div>
                    )}
                </div>
              </div>
            )}
          </TabsContent>

          <TabsContent value="new" className="space-y-6 mt-6">
            <div className="space-y-2">
              <Label>Project Name</Label>
              <Input 
                placeholder="e.g. my-awesome-app" 
                value={projectName} 
                onChange={(e) => setProjectName(e.target.value)}
                className="bg-white/5 border-white/10 h-12 rounded-xl"
              />
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
                            <Input 
                                placeholder="Location (e.g. London, UK)" 
                                value={address} 
                                onChange={(e) => setAddress(e.target.value)}
                                className="bg-white/5 border-white/10 pl-9 rounded-xl h-12"
                            />
                        </div>
                    ) : (
                        <Input 
                            type="number" 
                            min="0"
                            value={intensity} 
                            onChange={(e) => setIntensity(e.target.value)}
                            className="bg-white/5 border-white/10 rounded-xl h-12"
                            placeholder="gCO2/kWh"
                        />
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
                        <Input 
                            type="number" 
                            min="0"
                            value={cost} 
                            onChange={(e) => setCost(e.target.value)}
                            className="bg-white/5 border-white/10 rounded-xl h-12"
                            placeholder="Rate per kWh"
                        />
                    )}
                </div>
            </div>
          </TabsContent>
        </Tabs>

        {/* Advanced Options Accordion */}
        <Accordion type="single" collapsible className="w-full mt-4">
            <AccordionItem value="advanced" className="border-none">
                <AccordionTrigger className="hover:no-underline py-3 px-4 rounded-xl bg-white/5 border border-white/10">
                    <div className="flex items-center gap-2 text-sm font-bold">
                        <Settings2 className="w-4 h-4 text-primary" />
                        Execution Parameters
                    </div>
                </AccordionTrigger>
                <AccordionContent className="pt-4 px-4 space-y-6 bg-white/5 border-x border-b border-white/10 rounded-b-xl -mt-2 pb-6">
                    <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
                        <div className="space-y-2">
                            <Label className="flex items-center gap-2 text-xs font-bold text-muted-foreground uppercase tracking-widest">
                                <Tag className="w-3 h-3" />
                                Custom Run Name
                            </Label>
                            <Input 
                                placeholder="Optional name" 
                                value={runName} 
                                onChange={(e) => setRunName(e.target.value)}
                                className="bg-background/50 border-white/5 h-10 rounded-lg text-sm"
                            />
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
                            <Input 
                                type="number" 
                                min="0.1"
                                step="0.1" 
                                value={interval} 
                                onChange={(e) => setInterval(e.target.value)}
                                className="bg-background/50 border-white/5 h-10 rounded-lg text-sm"
                            />
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
                            <Input 
                                placeholder="e.g. ./results.csv" 
                                value={outputFile} 
                                onChange={(e) => setOutputFile(e.target.value)}
                                className="bg-background/50 border-white/5 h-10 rounded-lg text-sm"
                            />
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
                </AccordionContent>
            </AccordionItem>
        </Accordion>

        <div className="mt-6 p-4 rounded-2xl bg-white/5 border border-white/10 relative group overflow-hidden">
            <div className="absolute top-0 right-0 p-2">
                <div className="bg-primary/10 text-primary text-[10px] font-bold px-2 py-0.5 rounded-full border border-primary/20 uppercase tracking-tighter">
                    Generated Command
                </div>
            </div>
            <div className="font-mono text-[9px] md:text-xs text-foreground/90 break-all pt-4 leading-relaxed">
                <span className="text-primary font-bold mr-2">$</span>
                {generateCommand()}
            </div>
        </div>

        <DialogFooter className="mt-6 gap-2">
          <Button 
            variant="outline" 
            onClick={() => setOpen(false)}
            className="rounded-xl border-white/10 hover:bg-white/5"
          >
            Cancel
          </Button>
          {mode === 'new' ? (
            <Button 
                onClick={handleCreateAndCopy} 
                disabled={isCreating || !projectName || (useApiIntensity && !address)}
                className="rounded-xl bg-primary hover:bg-primary/90 font-bold px-8 shadow-lg shadow-primary/20"
            >
                {isCreating ? 'Creating...' : copied ? 'Copied!' : 'Create & Copy Command'}
                {copied ? <Check className="w-4 h-4 ml-2" /> : <Copy className="w-4 h-4 ml-2" />}
            </Button>
          ) : (
            <Button 
                onClick={handleCopy} 
                disabled={!selectedProjectId || (useApiIntensity && !address)}
                className="rounded-xl bg-primary hover:bg-primary/90 font-bold px-8 shadow-lg shadow-primary/20"
            >
                {copied ? 'Copied!' : 'Copy Command'}
                {copied ? <Check className="w-4 h-4 ml-2" /> : <Copy className="w-4 h-4 ml-2" />}
            </Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
