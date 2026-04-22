'use client'

import Link from 'next/link'
import { usePathname } from 'next/navigation'
import {
  History,
  Leaf,
  Folder,
  Shield,
  LayoutDashboard
} from 'lucide-react'
import { useDataSource } from './DataSourceProvider'

const navItems = [
  { href: '/', label: 'Dashboard', icon: LayoutDashboard },
  { href: '/admin', label: 'Admin', icon: Shield },
  { href: '/projects', label: 'Projects', icon: Folder },
  { href: '/runs', label: 'Run History', icon: History },

]

export function Sidebar() {
  const pathname = usePathname()
  const { source, setSource, availableSources, loadingSources } = useDataSource()

  return (
    <aside className="sidebar-glass h-screen w-64 fixed left-0 top-0 flex flex-col lg:relative lg:w-64 z-50">
      {/* Header */}
      <div className="p-6">
        <div className="flex items-center gap-3">
          <div className="p-2 rounded bg-primary">
            <Leaf className="w-5 h-5 text-black" />
          </div>
          <div>
            <h1 className="font-bold text-lg text-foreground tracking-tight">EcoCode</h1>
            <p className="text-[10px] uppercase tracking-widest text-muted-foreground/60">Energy Monitor</p>
          </div>
        </div>
      </div>

      <div className="px-4 py-4">
        <p className="text-[11px] uppercase tracking-[0.2em] text-muted-foreground/50 font-semibold mb-3 px-2">
          Data Source
        </p>
        <div className="grid grid-cols-2 gap-2 px-2">
          {(['local', 'online'] as const).map((candidate) => {
            const available = availableSources.includes(candidate)
            const active = source === candidate

            return (
              <button
                key={candidate}
                type="button"
                onClick={() => setSource(candidate)}
                disabled={loadingSources || !available}
                className={`rounded border px-3 py-1.5 text-xs font-medium transition-all ${
                  active
                    ? 'border-primary bg-primary text-black'
                    : available
                      ? 'border-border bg-transparent text-muted-foreground hover:text-foreground hover:border-muted-foreground/30'
                      : 'border-border/30 bg-transparent text-muted-foreground/30 cursor-not-allowed'
                }`}
              >
                {candidate === 'local' ? 'Local' : 'Online'}
              </button>
            )
          })}
        </div>
      </div>

      {/* Navigation */}
      <nav className="flex-1 p-4 space-y-1">
        <p className="text-[11px] uppercase tracking-[0.2em] text-muted-foreground/50 font-semibold mb-3 px-2">
          Project
        </p>
        {navItems.map(({ href, label, icon: Icon }) => {
          const isActive = pathname === href
          return (
            <Link
              key={href}
              href={href}
              className={`flex items-center gap-3 px-3 py-2 rounded transition-all duration-200 ${isActive
                  ? 'bg-muted text-primary'
                  : 'text-muted-foreground hover:text-foreground hover:bg-muted/50'
                }`}
            >
              <Icon className={`w-4 h-4 ${isActive ? 'text-primary' : ''}`} />
              <span className="text-sm font-medium">{label}</span>
            </Link>
          )
        })}
      </nav>

      {/* Footer */}
      <div className="p-6 border-t border-border space-y-1 text-[10px] text-muted-foreground/60 uppercase tracking-wider font-medium">
        <p>Dashboard v0.1</p>
        <p>Real-time energy monitoring</p>
      </div>
    </aside>
  )
}
