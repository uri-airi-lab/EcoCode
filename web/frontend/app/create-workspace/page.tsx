'use client'

import { useState, useRef } from 'react'
import { useRouter } from 'next/navigation'
import { FolderPlus, Loader2, ArrowRight, ArrowLeft } from 'lucide-react'
import { useWorkspace } from '@/components/layout/WorkspaceProvider'

export default function CreateWorkspacePage() {
  const router = useRouter()
  const { workspaces, refresh } = useWorkspace()
  const [creating, setCreating] = useState(false)
  const [name, setName] = useState('')
  const [slug, setSlug] = useState('')
  const [error, setError] = useState('')
  const submittingRef = useRef(false)

  const handleCreate = async (e: React.FormEvent) => {
    e.preventDefault()
    if (submittingRef.current) return
    submittingRef.current = true
    setCreating(true)
    setError('')

    try {
      const res = await fetch('/api/workspaces', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ name, slug }),
        credentials: 'include',
      })

      if (res.ok) {
        await refresh()
        router.push(`/${slug}`)
      } else if (res.status === 409) {
        setError('You already have a workspace with this slug. Please choose a different one.')
      } else if (res.status === 401) {
        router.push('/login')
      } else {
        setError('Failed to create workspace. Please try again.')
      }
    } catch {
      setError('An error occurred. Please try again.')
    } finally {
      submittingRef.current = false
      setCreating(false)
    }
  }

  const handleNameChange = (val: string) => {
    setName(val)
    if (!slug || slug === name.toLowerCase().replace(/[^a-z0-9]/g, '-')) {
      setSlug(val.toLowerCase().replace(/[^a-z0-9]/g, '-'))
    }
  }

  return (
    <div className="min-h-screen bg-neutral-950 flex flex-col items-center justify-center relative overflow-hidden text-white">
      <div className="absolute top-0 left-0 w-full h-full overflow-hidden z-0 pointer-events-none">
        <div className="absolute top-[-10%] left-[-10%] w-[40%] h-[40%] rounded-full bg-emerald-500/10 blur-[120px]" />
      </div>

      <div className="z-10 w-full max-w-md px-6">
        {workspaces.length > 0 && (
          <button
            onClick={() => router.back()}
            className="flex items-center gap-2 text-neutral-400 hover:text-white text-sm mb-6 transition-colors"
          >
            <ArrowLeft className="w-4 h-4" />
            Back
          </button>
        )}

        <div className="flex flex-col items-center mb-8 text-center">
          <div className="w-16 h-16 bg-neutral-900 border border-neutral-800 rounded-2xl flex items-center justify-center mb-6">
            <FolderPlus className="w-8 h-8 text-emerald-400" />
          </div>
          <h1 className="text-3xl font-bold mb-2">Create a workspace</h1>
          <p className="text-neutral-400">
            A workspace contains all your EcoCode projects, runs, and team members.
          </p>
        </div>

        <div className="bg-neutral-900/60 backdrop-blur-xl border border-neutral-800 rounded-3xl p-8 shadow-2xl">
          <form onSubmit={handleCreate} className="space-y-5">
            {error && (
              <p className="text-sm text-red-400 bg-red-500/10 border border-red-500/20 rounded-lg px-4 py-3">{error}</p>
            )}

            <div>
              <label className="block text-sm font-medium text-neutral-300 mb-2">
                Workspace Name
              </label>
              <input
                type="text"
                required
                value={name}
                onChange={(e) => handleNameChange(e.target.value)}
                className="w-full px-4 py-3 bg-neutral-950/50 border border-neutral-800 rounded-xl text-white placeholder:text-neutral-600 focus:outline-none focus:ring-2 focus:ring-emerald-500/50"
                placeholder="Acme Corp"
              />
            </div>

            <div>
              <label className="block text-sm font-medium text-neutral-300 mb-2">
                Workspace URL
              </label>
              <div className="flex items-center">
                <span className="px-4 py-3 bg-neutral-900 border border-neutral-800 border-r-0 rounded-l-xl text-neutral-500 text-sm whitespace-nowrap">
                  ecocode.com/
                </span>
                <input
                  type="text"
                  required
                  value={slug}
                  onChange={(e) => setSlug(e.target.value.toLowerCase().replace(/[^a-z0-9-]/g, ''))}
                  className="w-full px-4 py-3 bg-neutral-950/50 border border-neutral-800 rounded-r-xl text-white focus:outline-none focus:ring-2 focus:ring-emerald-500/50"
                  placeholder="acme-corp"
                />
              </div>
            </div>

            <button
              type="submit"
              disabled={creating}
              className="w-full mt-2 bg-gradient-to-r from-emerald-500 to-emerald-600 hover:from-emerald-400 hover:to-emerald-500 text-white font-medium py-3 px-4 rounded-xl flex items-center justify-center transition-all disabled:opacity-50"
            >
              {creating ? (
                <Loader2 className="w-5 h-5 animate-spin" />
              ) : (
                <>
                  Create Workspace
                  <ArrowRight className="w-5 h-5 ml-2" />
                </>
              )}
            </button>
          </form>
        </div>
      </div>
    </div>
  )
}
