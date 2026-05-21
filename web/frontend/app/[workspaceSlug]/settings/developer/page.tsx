'use client'

import { useState, useEffect } from 'react'
import { Key, Plus, Trash2, Copy, Check, Loader2 } from 'lucide-react'
import { DashboardLayout } from '@/components/layout/DashboardLayout'
import { useWorkspace } from '@/components/layout/WorkspaceProvider'

interface ApiKey {
  id: number
  name: string
  key_hint: string | null
  project_id: number | null
  created_at: string
}

interface Project {
  id: number
  name: string
}

export default function DeveloperSettingsPage() {
  const { currentWorkspace } = useWorkspace()
  const [keys, setKeys] = useState<ApiKey[]>([])
  const [projects, setProjects] = useState<Project[]>([])
  const [loading, setLoading] = useState(true)
  const [generating, setGenerating] = useState(false)
  const [newKey, setNewKey] = useState<string | null>(null)
  const [copied, setCopied] = useState(false)
  const [keyName, setKeyName] = useState('')
  const [selectedProjectId, setSelectedProjectId] = useState<number | ''>('')

  useEffect(() => {
    fetchKeys()
    fetchProjects()
  }, [])

  const fetchKeys = async () => {
    try {
      const res = await fetch('/api/auth/api-keys', { credentials: 'include' })
      if (res.ok) {
        setKeys(await res.json())
      }
    } finally {
      setLoading(false)
    }
  }

  const fetchProjects = async () => {
    try {
      const res = await fetch('/api/projects', { credentials: 'include' })
      if (res.ok) {
        setProjects(await res.json())
      }
    } catch (e) {
      // non-critical
    }
  }

  const handleGenerate = async () => {
    if (!keyName.trim()) return
    setGenerating(true)
    setNewKey(null)
    try {
      const res = await fetch('/api/auth/api-keys', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          name: keyName,
          workspace_id: currentWorkspace?.id,
          project_id: selectedProjectId !== '' ? selectedProjectId : null,
        }),
        credentials: 'include'
      })
      if (res.ok) {
        const data = await res.json()
        setNewKey(data.key)
        setKeyName('')
        await fetchKeys()
      }
    } finally {
      setGenerating(false)
    }
  }

  const handleRevoke = async (id: number) => {
    if (!confirm('Are you sure you want to revoke this key?')) return
    try {
      const res = await fetch(`/api/auth/api-keys/${id}`, {
        method: 'DELETE',
        credentials: 'include'
      })
      if (res.ok) {
        await fetchKeys()
      }
    } catch (e) {
      console.error(e)
    }
  }

  const copyToClipboard = () => {
    if (newKey) {
      navigator.clipboard.writeText(newKey)
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    }
  }

  return (
    <DashboardLayout>
      <div className="p-4 md:p-8 max-w-4xl mx-auto w-full">
        <div className="mb-8">
          <h1 className="text-2xl md:text-3xl font-bold text-foreground mb-2 flex items-center">
            <Key className="w-8 h-8 mr-3 text-primary" />
            Developer Settings
          </h1>
          <p className="text-muted-foreground">
            Manage your API keys for CLI access and external integrations.
          </p>
        </div>

        {/* Usage Guide Section */}
        <div className="mb-8 bg-card border border-border rounded-md overflow-hidden">
          <div className="p-6 border-b border-border">
            <h2 className="text-lg font-semibold mb-1">Using Your API Key</h2>
            <p className="text-sm text-muted-foreground">
              Three ways to provide your API key to the EcoCode CLI
            </p>
          </div>

          <div className="divide-y divide-border">
            {/* Method 1 */}
            <div className="p-6">
              <div className="flex items-center gap-3 mb-3">
                <span className="w-6 h-6 rounded-full bg-primary/15 text-primary text-xs font-bold flex items-center justify-center shrink-0">1</span>
                <h3 className="font-semibold text-sm">Save Once (Persistent Config)</h3>
                <span className="ml-auto text-[10px] font-bold bg-primary/15 text-primary px-2 py-0.5 rounded">RECOMMENDED</span>
              </div>
              <p className="text-xs text-muted-foreground mb-3 ml-9">
                Save your API key once and never enter it again. Stored in{' '}
                <code className="text-primary bg-background px-1.5 py-0.5 rounded font-mono">~/.ecocode/config.json</code>
              </p>
              <div className="ml-9 bg-background rounded-md p-3 border border-border">
                <code className="text-xs text-primary font-mono block leading-relaxed">
                  # Save your API key once<br />
                  ecocode config set-api-key YOUR_API_KEY<br />
                  <br />
                  # Now run without --api-key<br />
                  ecocode run --output online -- python train.py
                </code>
              </div>
            </div>

            {/* Method 2 */}
            <div className="p-6">
              <div className="flex items-center gap-3 mb-3">
                <span className="w-6 h-6 rounded-full bg-chart-1/20 text-chart-1 text-xs font-bold flex items-center justify-center shrink-0">2</span>
                <h3 className="font-semibold text-sm">Environment Variable</h3>
                <span className="ml-auto text-[10px] font-bold bg-chart-1/20 text-chart-1 px-2 py-0.5 rounded">CI/CD</span>
              </div>
              <p className="text-xs text-muted-foreground mb-3 ml-9">
                Ideal for CI/CD pipelines, Docker containers, or shell profiles. Overrides saved config.
              </p>
              <div className="ml-9 bg-background rounded-md p-3 border border-border">
                <code className="text-xs text-chart-1 font-mono block leading-relaxed">
                  # Set for current session<br />
                  export ECOCODE_API_KEY=YOUR_API_KEY<br />
                  <br />
                  # Or add to ~/.bashrc / ~/.zshrc<br />
                  echo &apos;export ECOCODE_API_KEY=YOUR_KEY&apos; &gt;&gt; ~/.bashrc<br />
                  <br />
                  ecocode run --output online -- python train.py
                </code>
              </div>
            </div>

            {/* Method 3 */}
            <div className="p-6">
              <div className="flex items-center gap-3 mb-3">
                <span className="w-6 h-6 rounded-full bg-chart-2/20 text-chart-2 text-xs font-bold flex items-center justify-center shrink-0">3</span>
                <h3 className="font-semibold text-sm">Command-Line Flag</h3>
                <span className="ml-auto text-[10px] font-bold bg-chart-2/20 text-chart-2 px-2 py-0.5 rounded">OVERRIDE</span>
              </div>
              <p className="text-xs text-muted-foreground mb-3 ml-9">
                Pass the key inline. Overrides both saved config and environment variable.
              </p>
              <div className="ml-9 bg-background rounded-md p-3 border border-border">
                <code className="text-xs text-chart-2 font-mono block leading-relaxed">
                  ecocode run --output online --api-key YOUR_API_KEY -- python train.py
                </code>
              </div>
            </div>

            {/* Priority + Extra commands */}
            <div className="p-6 grid grid-cols-1 sm:grid-cols-2 gap-6">
              {/* Priority order */}
              <div className="bg-background border border-border rounded-md p-4">
                <h4 className="text-xs font-bold text-foreground uppercase tracking-widest mb-3">Priority Order</h4>
                <ol className="space-y-2">
                  {[
                    { label: '--api-key flag',      color: 'text-chart-2', badge: 'bg-chart-2/15' },
                    { label: 'ECOCODE_API_KEY env', color: 'text-chart-1', badge: 'bg-chart-1/15' },
                    { label: '~/.ecocode/config',   color: 'text-primary',  badge: 'bg-primary/15' },
                  ].map((item, i) => (
                    <li key={i} className="flex items-center gap-2 text-xs text-muted-foreground">
                      <span className={`w-4 h-4 rounded-full ${item.badge} ${item.color} text-[10px] font-bold flex items-center justify-center shrink-0`}>{i + 1}</span>
                      <code className={`${item.color} font-mono`}>{item.label}</code>
                    </li>
                  ))}
                </ol>
              </div>

              {/* Additional commands */}
              <div className="bg-background border border-border rounded-md p-4">
                <h4 className="text-xs font-bold text-foreground uppercase tracking-widest mb-3">Config Commands</h4>
                <div className="space-y-3">
                  <div>
                    <code className="text-xs text-foreground font-mono">ecocode config show</code>
                    <p className="text-[10px] text-muted-foreground mt-0.5">View current configuration</p>
                  </div>
                  <div>
                    <code className="text-xs text-foreground font-mono">ecocode config clear-api-key</code>
                    <p className="text-[10px] text-muted-foreground mt-0.5">Remove saved API key</p>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>

        {newKey && (
          <div className="mb-8 bg-primary/10 border border-primary/30 rounded-md p-6">
            <h3 className="text-lg font-semibold text-primary mb-2">New API Key Generated</h3>
            <p className="text-sm text-foreground/70 mb-4">
              Please copy this key now. For security reasons, it will never be shown again.
            </p>
            <div className="flex items-center gap-2 mb-4">
              <code className="flex-1 bg-background px-4 py-3 rounded-md border border-primary/20 text-primary font-mono text-sm break-all">
                {newKey}
              </code>
              <button
                onClick={copyToClipboard}
                className="bg-primary hover:brightness-110 text-primary-foreground p-3 rounded-md transition-colors shrink-0"
              >
                {copied ? <Check className="w-5 h-5" /> : <Copy className="w-5 h-5" />}
              </button>
            </div>

            {/* Quick Start Guide */}
            <div className="bg-background/50 rounded-md p-4 border border-primary/20">
              <h4 className="text-sm font-semibold text-primary mb-2 flex items-center gap-2">
                <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 10V3L4 14h7v7l9-11h-7z" />
                </svg>
                Quick Start
              </h4>
              <p className="text-xs text-muted-foreground mb-3">
                Save your API key once and use it automatically:
              </p>
              <div className="bg-background rounded-md p-3 border border-primary/10">
                <code className="text-xs text-primary font-mono block">
                  ecocode config set-api-key {newKey}
                </code>
              </div>
              <p className="text-[10px] text-muted-foreground mt-2">
                See the usage guide below for more options
              </p>
            </div>
          </div>
        )}

        <div className="bg-card border border-border rounded-md overflow-hidden">
          <div className="p-6 border-b border-border flex flex-col sm:flex-row sm:items-end justify-between gap-4">
            <div>
              <h2 className="text-lg font-semibold">API Keys</h2>
              <p className="text-sm text-muted-foreground">Keys associated with your account</p>
            </div>
            <div className="flex flex-col sm:flex-row items-stretch sm:items-center gap-2">
              <input
                type="text"
                placeholder="Key name (e.g. training-server)"
                value={keyName}
                onChange={(e) => setKeyName(e.target.value)}
                className="bg-background border border-border rounded-md px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground/50 focus:outline-none focus:ring-2 focus:ring-primary/50 focus:border-primary w-full sm:w-48 transition-all"
              />
              <select
                value={selectedProjectId}
                onChange={(e) => setSelectedProjectId(e.target.value === '' ? '' : Number(e.target.value))}
                className="bg-background border border-border rounded-md px-3 py-2 text-sm text-foreground focus:outline-none focus:ring-2 focus:ring-primary/50 focus:border-primary w-full sm:w-44 transition-all"
              >
                <option value="">Select project</option>
                {projects.map(p => (
                  <option key={p.id} value={p.id}>{p.name}</option>
                ))}
              </select>
              <button
                onClick={handleGenerate}
                disabled={generating || !keyName.trim() || selectedProjectId === ''}
                className="bg-primary hover:brightness-110 text-primary-foreground px-4 py-2 rounded-md font-medium text-sm flex items-center justify-center transition-all disabled:opacity-50 whitespace-nowrap"
              >
                {generating ? <Loader2 className="w-4 h-4 mr-2 animate-spin" /> : <Plus className="w-4 h-4 mr-2" />}
                Generate Key
              </button>
            </div>
          </div>

          <div className="divide-y divide-border">
            {loading ? (
              <div className="p-8 flex justify-center">
                <Loader2 className="w-6 h-6 text-primary animate-spin" />
              </div>
            ) : keys.length === 0 ? (
              <div className="p-8 text-center text-muted-foreground text-sm">
                No API keys found. Generate one to get started.
              </div>
            ) : (
              keys.map(key => (
                <div key={key.id} className="p-4 sm:px-6 flex items-center justify-between hover:bg-muted/30 transition-colors">
                  <div>
                    <div className="flex items-center gap-3 mb-1">
                      <span className="font-semibold text-sm text-foreground">
                        {key.name}
                      </span>
                      {key.key_hint && (
                        <span className="font-mono text-[10px] bg-background px-2 py-0.5 rounded border border-border text-muted-foreground">
                          {key.key_hint}
                        </span>
                      )}
                      {key.project_id && (
                        <span className="text-[10px] bg-primary/10 text-primary px-2 py-0.5 rounded border border-primary/20">
                          {projects.find(p => p.id === key.project_id)?.name ?? `project ${key.project_id}`}
                        </span>
                      )}
                    </div>
                    <p className="text-xs text-muted-foreground">
                      Created on {new Date(key.created_at).toLocaleDateString()}
                    </p>
                  </div>
                  <button
                    onClick={() => handleRevoke(key.id)}
                    className="p-2 text-muted-foreground hover:text-destructive hover:bg-destructive/10 rounded-md transition-colors"
                    title="Revoke Key"
                  >
                    <Trash2 className="w-4 h-4" />
                  </button>
                </div>
              ))
            )}
          </div>
        </div>
      </div>
    </DashboardLayout>
  )
}
