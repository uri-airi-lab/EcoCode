'use client'

import Link from 'next/link'
import { usePathname, useRouter } from 'next/navigation'
import {
  History,
  Leaf,
  Folder,
  Shield,
  LayoutDashboard,
  Users,
  ChevronDown,
  LogOut,
  Plus
} from 'lucide-react'
import { useWorkspace } from './WorkspaceProvider'
import { toast } from 'sonner'

export function Sidebar() {
  const pathname = usePathname()
  const router = useRouter()
  const { workspaces, currentWorkspace, dropdownOpen, setDropdownOpen } = useWorkspace()

  const workspaceSlug = currentWorkspace?.slug ?? pathname.split('/').filter(Boolean)[0] ?? ''

  const handleLogout = async () => {
    try {
      const res = await fetch('/api/auth/logout', { 
        method: 'POST',
        credentials: 'include'
      })
      if (res.ok) {
        toast.success('Logged out successfully')
        router.push('/login')
      } else {
        toast.error('Failed to logout')
      }
    } catch (error) {
      toast.error('An error occurred during logout')
    }
  }

  const navItems = [
    { href: `/${workspaceSlug}`, label: 'Dashboard', icon: LayoutDashboard, exact: true },
    { href: `/${workspaceSlug}/projects`, label: 'Projects', icon: Folder },
    { href: `/${workspaceSlug}/runs`, label: 'Run History', icon: History },
    { href: `/${workspaceSlug}/settings/developer`, label: 'Developer API', icon: Shield },
    { href: `/${workspaceSlug}/settings/members`, label: 'Members', icon: Users },
  ]

  return (
    <aside className="sidebar-glass h-screen w-64 fixed left-0 top-0 flex flex-col lg:relative lg:w-64 z-50">
      {/* Header / Workspace Switcher */}
      <div className="p-4 border-b border-border relative">
        <div
          className="flex items-center justify-between p-2 rounded-lg hover:bg-neutral-800/50 cursor-pointer transition-colors"
          onClick={() => setDropdownOpen(!dropdownOpen)}
        >
          <div className="flex items-center gap-3 overflow-hidden">
            <div className="p-1.5 rounded-md bg-primary/15 flex-shrink-0">
              <Leaf className="w-4 h-4 text-primary-foreground" />
            </div>
            <div className="truncate">
              <h1 className="font-semibold text-sm text-foreground truncate">
                {currentWorkspace ? currentWorkspace.name : 'Loading...'}
              </h1>
              <p className="text-[10px] uppercase tracking-widest text-primary font-medium">Workspace</p>
            </div>
          </div>
          <ChevronDown className={`w-4 h-4 text-neutral-400 transition-transform ${dropdownOpen ? 'rotate-180' : ''}`} />
        </div>

        {dropdownOpen && (
          <div className="absolute top-[100%] left-4 right-4 mt-2 bg-card border border-border rounded-xl shadow-xl overflow-hidden z-50">
            <div className="p-2 space-y-1">
              <p className="px-2 py-1 text-[10px] uppercase tracking-wider text-neutral-500 font-semibold">Your Workspaces</p>
              {workspaces.map(ws => (
                <button
                  key={ws.id}
                  onClick={() => {
                    setDropdownOpen(false)
                    router.push(`/${ws.slug}`)
                  }}
                  className={`w-full text-left px-3 py-2 rounded-lg text-sm transition-colors ${ws.slug === workspaceSlug ? 'bg-primary/10 text-primary font-medium' : 'text-foreground hover:bg-muted'}`}
                >
                  {ws.name}
                </button>
              ))}
              <div className="h-px bg-neutral-800 my-1" />
              <button
                onClick={() => {
                  setDropdownOpen(false)
                  router.push('/')
                }}
                className="w-full text-left px-3 py-2 rounded-lg text-sm text-foreground hover:bg-muted flex items-center"
              >
                <Plus className="w-4 h-4 mr-2" />
                Create Workspace
              </button>
            </div>
          </div>
        )}
      </div>

      {/* Navigation */}
      <nav className="flex-1 p-4 flex flex-col space-y-1 overflow-y-auto">
        <p className="text-[11px] uppercase tracking-[0.2em] text-muted-foreground/50 font-semibold mb-3 px-2">
          Menu
        </p>
        {navItems.map(({ href, label, icon: Icon, exact }) => {
          const isActive = exact ? pathname === href : pathname.startsWith(href)
          return (
            <Link
              key={href}
              href={href}
              className={`flex items-center gap-3 px-3 py-2.5 rounded-md transition-all duration-200 ${isActive
                  ? 'border-l-2 border-primary bg-primary/10 text-primary font-medium'
                  : 'border-l-2 border-transparent text-muted-foreground hover:bg-muted hover:text-foreground'
                }`}
            >
              <Icon className={`w-4 h-4 ${isActive ? 'text-primary' : ''}`} />
              <span className="text-sm">{label}</span>
            </Link>
          )
        })}

        <div className="flex-1" />

        <button
          onClick={handleLogout}
          className="w-full flex items-center gap-3 px-3 py-2.5 rounded-lg text-neutral-400 hover:text-destructive hover:bg-destructive/10 transition-all duration-200 border border-transparent mt-4"
        >
          <LogOut className="w-4 h-4" />
          <span className="text-sm">Logout</span>
        </button>
      </nav>

      {/* Footer */}
      <div className="p-6 border-t border-border space-y-1 font-mono text-[11px] text-muted-foreground/50 uppercase tracking-wider font-medium bg-background">
        <p>EcoCode Platform v1.0</p>
        <p>Sustainable Engineering</p>
      </div>
    </aside>
  )
}
