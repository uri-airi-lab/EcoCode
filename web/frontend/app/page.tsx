'use client'

import { useState, useEffect } from 'react'
import { useRouter } from 'next/navigation'
import { FolderPlus, Loader2, ArrowRight } from 'lucide-react'
import { useWorkspace } from '@/components/layout/WorkspaceProvider'

export default function RootPage() {
  const router = useRouter()
  const { workspaces, loading, refresh } = useWorkspace()
  const [creating, setCreating] = useState(false)
  
  const [name, setName] = useState('')
  const [slug, setSlug] = useState('')

  useEffect(() => {
    if (loading) return
    if (workspaces.length > 0) {
      router.push(`/${workspaces[0].slug}`)
    } else {
      router.push('/create-workspace')
    }
  }, [loading, workspaces, router])

  const handleCreate = async (e: React.FormEvent) => {
    e.preventDefault()
    setCreating(true)

    try {
      const res = await fetch('/api/workspaces', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ name, slug }),
        credentials: 'include'
      })

      if (res.ok) {
        // Refresh the global workspace context before navigating
        await refresh()
        router.push(`/${slug}`)
      } else if (res.status === 409) {
        alert('Failed to create workspace. Slug might be taken.')
      } else if (res.status === 401) {
        router.push('/login')
      } else {
        const errorData = await res.json().catch(() => ({}))
        alert(`Failed to create workspace: ${errorData.message || 'Server error'}`)
      }
    } catch (err) {
      alert('An error occurred')
    } finally {
      setCreating(false)
    }
  }

  // Auto-generate slug from name
  const handleNameChange = (val: string) => {
    setName(val)
    if (!slug || slug === name.toLowerCase().replace(/[^a-z0-9]/g, '-')) {
      setSlug(val.toLowerCase().replace(/[^a-z0-9]/g, '-'))
    }
  }

  if (loading) {
    return (
      <div className="min-h-screen bg-neutral-950 flex items-center justify-center">
        <Loader2 className="w-8 h-8 text-emerald-500 animate-spin" />
      </div>
    )
  }

  return (
    <div className="min-h-screen bg-neutral-950 flex flex-col items-center justify-center relative overflow-hidden text-white">
      <div className="absolute top-0 left-0 w-full h-full overflow-hidden z-0 pointer-events-none">
        <div className="absolute top-[-10%] left-[-10%] w-[40%] h-[40%] rounded-full bg-emerald-500/10 blur-[120px]" />
      </div>

      <div className="z-10 w-full max-w-md px-6">
        <div className="flex flex-col items-center mb-8 text-center">
          <div className="w-16 h-16 bg-neutral-900 border border-neutral-800 rounded-2xl flex items-center justify-center mb-6">
            <FolderPlus className="w-8 h-8 text-emerald-400" />
          </div>
          <h1 className="text-3xl font-bold mb-2">Create your workspace</h1>
          <p className="text-neutral-400">
            A workspace contains all your EcoCode projects, runs, and team members.
          </p>
        </div>

        <div className="bg-neutral-900/60 backdrop-blur-xl border border-neutral-800 rounded-3xl p-8 shadow-2xl">
          <form onSubmit={handleCreate} className="space-y-5">
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
                <span className="px-4 py-3 bg-neutral-900 border border-neutral-800 border-r-0 rounded-l-xl text-neutral-500 text-sm">
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
