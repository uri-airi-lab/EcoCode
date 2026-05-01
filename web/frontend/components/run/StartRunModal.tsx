'use client'

import React, { useState, useEffect } from 'react'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Check, Copy, Rocket, Zap } from 'lucide-react'
import { ProjectSummary, createProject, fetchCarbonConfig } from '@/lib/api'
import { useWorkspace } from '@/components/layout/WorkspaceProvider'
import { toast } from 'sonner'
import { StartRunForm } from './StartRunForm'

interface StartRunModalProps {
  projects: ProjectSummary[]
  onProjectCreated?: () => void
  defaultProjectId?: string
  /** When true, only the "Existing Project" tab is shown. */
  existingOnly?: boolean
}

export function StartRunModal({
  projects,
  onProjectCreated,
  defaultProjectId,
  existingOnly = false,
}: StartRunModalProps) {
  const { currentWorkspace } = useWorkspace()
  const [open, setOpen] = useState(false)

  // Form state
  const [mode, setMode] = useState<'existing' | 'new'>('existing')
  const [selectedProjectId, setSelectedProjectId] = useState<string>('')
  const [projectName, setProjectName] = useState('')
  const [intensity, setIntensity] = useState('445')
  const [cost, setCost] = useState('0.15')
  const [currency, setCurrency] = useState('USD')
  const [address, setAddress] = useState('')
  const [useApiCost, setUseApiCost] = useState(false)
  const [useApiIntensity, setUseApiIntensity] = useState(false)
  const [outputFormat, setOutputFormat] = useState('online')
  const [outputFile, setOutputFile] = useState('')
  const [interval, setInterval] = useState('1.0')
  const [collaborator, setCollaborator] = useState('')
  const [runName, setRunName] = useState('')

  const [isCreating, setIsCreating] = useState(false)
  const [copied, setCopied] = useState(false)
  const [missingConfig, setMissingConfig] = useState(false)

  useEffect(() => {
    if (defaultProjectId) {
      setSelectedProjectId(defaultProjectId)
      setMode('existing')
    }
  }, [defaultProjectId])

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

    let cmd = `ecocode run --project "${project}"`
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

    cmd += ` --api-key "ecocode_your_key_here"`
    cmd += ` "<your-command>"`
    return cmd
  }

  const validate = (): boolean => {
    if (!useApiIntensity && parseFloat(intensity) < 0) { toast.error('Intensity cannot be negative'); return false }
    if (!useApiCost && parseFloat(cost) < 0) { toast.error('Cost cannot be negative'); return false }
    if (parseFloat(interval) <= 0) { toast.error('Interval must be greater than 0'); return false }
    return true
  }

  const handleCopy = () => {
    const project = mode === 'new' ? projectName : projects.find(p => p.id === selectedProjectId)?.name
    if (!project || !validate()) return
    const cmd = generateCommand()
    if (cmd === '...') return
    navigator.clipboard.writeText(cmd)
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
    toast.success('Command copied to clipboard!')
  }

  const handleCreateAndCopy = async () => {
    if (!projectName) { toast.error('Project name is required'); return }
    if (useApiIntensity && !address) { toast.error('Address is required for API intensity'); return }
    if (!validate()) return
    if (!currentWorkspace?.id) { toast.error('Workspace not identified'); return }

    try {
      setIsCreating(true)
      await createProject(projectName, currentWorkspace.id, {
        intensity: useApiIntensity ? undefined : parseFloat(intensity),
        cost: useApiCost ? undefined : parseFloat(cost),
        currency,
        address: address || undefined,
        use_api_for_cost: useApiCost,
      })
      handleCopy()
      if (onProjectCreated) onProjectCreated()
      setOpen(false)
    } catch (err) {
      toast.error(`Failed to create project: ${err instanceof Error ? err.message : 'An error occurred'}`)
    } finally {
      setIsCreating(false)
    }
  }

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

        <StartRunForm
          mode={mode} setMode={setMode}
          selectedProjectId={selectedProjectId} setSelectedProjectId={setSelectedProjectId}
          projectName={projectName} setProjectName={setProjectName}
          intensity={intensity} setIntensity={setIntensity}
          cost={cost} setCost={setCost}
          currency={currency} setCurrency={setCurrency}
          address={address} setAddress={setAddress}
          useApiCost={useApiCost} setUseApiCost={setUseApiCost}
          useApiIntensity={useApiIntensity} setUseApiIntensity={setUseApiIntensity}
          outputFormat={outputFormat} setOutputFormat={setOutputFormat}
          outputFile={outputFile} setOutputFile={setOutputFile}
          interval={interval} setInterval={setInterval}
          collaborator={collaborator} setCollaborator={setCollaborator}
          runName={runName} setRunName={setRunName}
          missingConfig={missingConfig}
          existingOnly={existingOnly}
          projects={projects}
        />

        {/* Generated command preview */}
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
          <Button variant="outline" onClick={() => setOpen(false)} className="rounded-xl border-white/10 hover:bg-white/5">
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
