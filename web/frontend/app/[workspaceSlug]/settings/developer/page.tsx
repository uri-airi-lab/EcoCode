'use client'

import { useState, useEffect } from 'react'
import { Key, Plus, Trash2, Copy, Check, Loader2 } from 'lucide-react'
import { DashboardLayout } from '@/components/layout/DashboardLayout'
import { useWorkspace } from '@/components/layout/WorkspaceProvider'

interface ApiKey {
  id: number
  name: string
  key_hint: string
  created_at: string
}

export default function DeveloperSettingsPage() {
  const { currentWorkspace } = useWorkspace()
  const [keys, setKeys] = useState<ApiKey[]>([])
  const [loading, setLoading] = useState(true)
  const [generating, setGenerating] = useState(false)
  const [newKey, setNewKey] = useState<string | null>(null)
  const [copied, setCopied] = useState(false)
  const [keyName, setKeyName] = useState('')

  useEffect(() => {
    fetchKeys()
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
          workspace_id: currentWorkspace?.id
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
            <h2 className="text-lg font-semibold mb-2">Using Your API Key</h2>
            <p className="text-sm text-muted-foreground">
              There are three ways to provide your API key to the EcoCode CLI
            </p>
          </div>

          <div className="p-6 space-y-6">
            {/* Method 1: Saved Config */}
            <div className="space-y-2">
              <div className="flex items-start gap-3">
                <div className="bg-primary/15 text-primary rounded-md px-2.5 py-1 text-xs font-bold shrink-0 mt-0.5">
                  RECOMMENDED
                </div>
                <div className="flex-1">
                  <h3 className="font-semibold text-sm mb-1">1. Save Once (Persistent Config)</h3>
                  <p className="text-xs text-muted-foreground mb-3">
                    Save your API key once and never enter it again. The key is stored in{' '}
                    <code className="text-primary bg-background px-1.5 py-0.5 rounded font-mono">~/.ecocode/config.json</code>
                  </p>
                  <div className="bg-background rounded-md p-3 border border-border">
                    <code className="text-xs text-primary font-mono block">
                      # Save your API key once<br />
                      ecocode config set-api-key YOUR_API_KEY<br />
                      <br />
                      # Now use without --api-key flag<br />
                      ecocode run --output online -- python train.py
                    </code>
                  </div>
                </div>
              </div>
            </div>

            {/* Method 2: Environment Variable */}
            <div className="space-y-2">
              <div className="flex items-start gap-3">
                <div className="bg-chart-1/20 text-chart-1 rounded-md px-2.5 py-1 text-xs font-bold shrink-0 mt-0.5">
                  CI/CD
                </div>
                <div className="flex-1">
                  <h3 className="font-semibold text-sm mb-1">2. Environment Variable</h3>
                  <p className="text-xs text-muted-foreground mb-3">
                    Great for CI/CD pipelines, Docker containers, or shell profiles. Overrides saved config.
                  </p>
                  <div className="bg-background rounded-md p-3 border border-border">
                    <code className="text-xs text-chart-1 font-mono block">
                      # Set for current session<br />
                      export ECOCODE_API_KEY=YOUR_API_KEY<br />
                      <br />
                      # Or add to ~/.bashrc or ~/.zshrc<br />
                      echo &apos;export ECOCODE_API_KEY=YOUR_KEY&apos; &gt;&gt; ~/.bashrc<br />
                      <br />
                      # Use the CLI<br />
                      ecocode run --output online -- python train.py
                    </code>
                  </div>
                </div>
              </div>
            </div>

            {/* Method 3: Command Line */}
            <div className="space-y-2">
              <div className="flex items-start gap-3">
                <div className="bg-chart-2/20 text-chart-2 rounded-md px-2.5 py-1 text-xs font-bold shrink-0 mt-0.5">
                  OVERRIDE
                </div>
                <div className="flex-1">
                  <h3 className="font-semibold text-sm mb-1">3. Command-Line Flag</h3>
                  <p className="text-xs text-muted-foreground mb-3">
                    Provide the key directly in the command. Overrides both saved config and environment variable.
                  </p>
                  <div className="bg-background rounded-md p-3 border border-border">
                    <code className="text-xs text-chart-2 font-mono block">
                      ecocode run --output online --api-key YOUR_API_KEY -- python train.py
                    </code>
                  </div>
                </div>
              </div>
            </div>

            {/* Priority Order */}
            <div className="bg-background border border-border rounded-md p-4">
              <h4 className="text-xs font-semibold text-foreground mb-2 flex items-center gap-2">
                <svg className="w-4 h-4 text-primary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                </svg>
                Priority Order (Highest to Lowest)
              </h4>
              <ol className="text-xs text-muted-foreground space-y-1 ml-6">
                <li className="list-decimal">Command-line flag <code className="text-chart-2 bg-card px-1 py-0.5 rounded font-mono text-[10px]">--api-key</code></li>
                <li className="list-decimal">Environment variable <code className="text-chart-1 bg-card px-1 py-0.5 rounded font-mono text-[10px]">ECOCODE_API_KEY</code></li>
                <li className="list-decimal">Saved config file <code className="text-primary bg-card px-1 py-0.5 rounded font-mono text-[10px]">~/.ecocode/config.json</code></li>
              </ol>
            </div>

            {/* Additional Commands */}
            <div className="border-t border-border pt-4">
              <h4 className="text-xs font-semibold text-foreground mb-3">Additional Config Commands</h4>
              <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                <div className="bg-background rounded-md p-3 border border-border">
                  <code className="text-xs text-foreground font-mono block mb-1">
                    ecocode config show
                  </code>
                  <p className="text-[10px] text-muted-foreground">View current configuration</p>
                </div>
                <div className="bg-background rounded-md p-3 border border-border">
                  <code className="text-xs text-foreground font-mono block mb-1">
                    ecocode config clear-api-key
                  </code>
                  <p className="text-[10px] text-muted-foreground">Remove saved API key</p>
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
            <div className="flex items-center gap-2">
              <input
                type="text"
                placeholder="Key Name (e.g. GitHub Actions)"
                value={keyName}
                onChange={(e) => setKeyName(e.target.value)}
                className="bg-background border border-border rounded-md px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground/50 focus:outline-none focus:ring-2 focus:ring-primary/50 focus:border-primary w-full sm:w-64 transition-all"
              />
              <button
                onClick={handleGenerate}
                disabled={generating || !keyName.trim()}
                className="bg-primary hover:brightness-110 text-primary-foreground px-4 py-2 rounded-md font-medium text-sm flex items-center transition-all disabled:opacity-50 whitespace-nowrap"
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
                      <span className="font-mono text-[10px] bg-background px-2 py-0.5 rounded border border-border text-muted-foreground">
                        {key.key_hint}
                      </span>
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
