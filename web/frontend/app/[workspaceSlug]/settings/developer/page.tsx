'use client'

import { useState, useEffect } from 'react'
import { Key, Plus, Trash2, Copy, Check, Loader2 } from 'lucide-react'
import { DashboardLayout } from '@/components/DashboardLayout'
import { useWorkspace } from '@/components/WorkspaceProvider'

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
      const res = await fetch('/api/auth/api-keys')
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
        })
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
      const res = await fetch(`/api/auth/api-keys/${id}`, { method: 'DELETE' })
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
            <Key className="w-8 h-8 mr-3 text-emerald-500" />
            Developer Settings
          </h1>
          <p className="text-muted-foreground">
            Manage your API keys for CLI access and external integrations.
          </p>
        </div>

        {newKey && (
          <div className="mb-8 bg-emerald-500/10 border border-emerald-500/30 rounded-2xl p-6 relative overflow-hidden">
            <div className="absolute top-0 right-0 w-32 h-32 bg-emerald-500/20 blur-[50px] -mr-10 -mt-10 pointer-events-none" />
            <h3 className="text-lg font-semibold text-emerald-400 mb-2">New API Key Generated</h3>
            <p className="text-sm text-emerald-200/70 mb-4">
              Please copy this key now. For security reasons, it will never be shown again.
            </p>
            <div className="flex items-center gap-2">
              <code className="flex-1 bg-neutral-950 px-4 py-3 rounded-xl border border-emerald-500/20 text-emerald-300 font-mono text-sm break-all">
                {newKey}
              </code>
              <button
                onClick={copyToClipboard}
                className="bg-emerald-500 hover:bg-emerald-600 text-neutral-950 p-3 rounded-xl transition-colors shrink-0"
              >
                {copied ? <Check className="w-5 h-5" /> : <Copy className="w-5 h-5" />}
              </button>
            </div>
          </div>
        )}

        <div className="bg-neutral-900/40 border border-neutral-800 rounded-2xl overflow-hidden">
          <div className="p-6 border-b border-neutral-800 flex flex-col sm:flex-row sm:items-end justify-between gap-4">
            <div>
              <h2 className="text-lg font-semibold">API Keys</h2>
              <p className="text-sm text-neutral-400">Keys associated with your account</p>
            </div>
            <div className="flex items-center gap-2">
              <input
                type="text"
                placeholder="Key Name (e.g. GitHub Actions)"
                value={keyName}
                onChange={(e) => setKeyName(e.target.value)}
                className="bg-neutral-950 border border-neutral-800 rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-emerald-500/50 w-full sm:w-64"
              />
              <button
                onClick={handleGenerate}
                disabled={generating || !keyName.trim()}
                className="bg-neutral-100 hover:bg-white text-neutral-900 px-4 py-2 rounded-lg font-medium text-sm flex items-center transition-colors disabled:opacity-50 whitespace-nowrap"
              >
                {generating ? <Loader2 className="w-4 h-4 mr-2 animate-spin" /> : <Plus className="w-4 h-4 mr-2" />}
                Generate Key
              </button>
            </div>
          </div>

          <div className="divide-y divide-neutral-800/50">
            {loading ? (
              <div className="p-8 flex justify-center">
                <Loader2 className="w-6 h-6 text-emerald-500 animate-spin" />
              </div>
            ) : keys.length === 0 ? (
              <div className="p-8 text-center text-neutral-500 text-sm">
                No API keys found. Generate one to get started.
              </div>
            ) : (
              keys.map(key => (
                <div key={key.id} className="p-4 sm:px-6 flex items-center justify-between hover:bg-neutral-800/20 transition-colors">
                  <div>
                    <div className="flex items-center gap-3 mb-1">
                      <span className="font-semibold text-sm text-neutral-200">
                        {key.name}
                      </span>
                      <span className="font-mono text-[10px] bg-neutral-950 px-2 py-0.5 rounded border border-neutral-800 text-neutral-500">
                        {key.key_hint}
                      </span>
                    </div>
                    <p className="text-xs text-neutral-500">
                      Created on {new Date(key.created_at).toLocaleDateString()}
                    </p>
                  </div>
                  <button
                    onClick={() => handleRevoke(key.id)}
                    className="p-2 text-neutral-500 hover:text-red-400 hover:bg-red-400/10 rounded-lg transition-colors"
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
