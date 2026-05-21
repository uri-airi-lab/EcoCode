'use client'

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from 'react'
import { usePathname } from 'next/navigation'

export interface Workspace {
  id: number
  name: string
  slug: string
  role: string
}

interface WorkspaceContextValue {
  workspaces: Workspace[]
  currentWorkspace: Workspace | null
  loading: boolean
  dropdownOpen: boolean
  setDropdownOpen: (open: boolean) => void
  refresh: () => void
}

const WorkspaceContext = createContext<WorkspaceContextValue | null>(null)

export function WorkspaceProvider({ children }: { children: React.ReactNode }) {
  const pathname = usePathname()
  const [workspaces, setWorkspaces] = useState<Workspace[]>([])
  const [loading, setLoading] = useState(true)
  const [dropdownOpen, setDropdownOpen] = useState(false)

  const load = useCallback(async () => {
    // The root path is the public landing page — skip auth entirely
    if (typeof window !== 'undefined' && window.location.pathname === '/') {
      setLoading(false)
      return
    }
    try {
      const res = await fetch('/api/workspaces', { 
        credentials: 'include',
        cache: 'no-store'
      })
      if (res.ok) {
        const data: Workspace[] = await res.json()
        setWorkspaces(data)
        
        // If the user has no workspaces, redirect to workspace creation page
        const path = window.location.pathname
        const isAuthOrCreate = path === '/' || path.startsWith('/login') || path.startsWith('/register') || path.startsWith('/create-workspace')
        if (data.length === 0 && !isAuthOrCreate) {
          window.location.href = '/create-workspace'
        }
      } else if (res.status === 401) {
        // Token might be invalid or expired (e.g., after DB reset)
        // Clear the auth_token cookie to prevent the middleware redirect loop
        try {
          await fetch('/api/auth/logout', { method: 'POST', credentials: 'include' })
        } catch {
          // ignore
        }
        // Only redirect if not already on an auth page
        if (!window.location.pathname.startsWith('/login') && !window.location.pathname.startsWith('/register')) {
          window.location.href = '/login'
        }
      }
    } catch {
      // ignore — backend may not be up yet
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    load()
  }, [load])

  const slug = pathname.split('/').filter(Boolean)[0] ?? ''
  const currentWorkspace = useMemo(
    () => workspaces.find(w => w.slug === slug) ?? workspaces[0] ?? null,
    [workspaces, slug],
  )

  const value = useMemo(
    () => ({ workspaces, currentWorkspace, loading, dropdownOpen, setDropdownOpen, refresh: load }),
    [workspaces, currentWorkspace, loading, dropdownOpen, load],
  )

  return <WorkspaceContext.Provider value={value}>{children}</WorkspaceContext.Provider>
}

export function useWorkspace() {
  const ctx = useContext(WorkspaceContext)
  if (!ctx) throw new Error('useWorkspace must be used within WorkspaceProvider')
  return ctx
}
