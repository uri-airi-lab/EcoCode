'use client'

import Link from 'next/link'
import { usePathname } from 'next/navigation'
import {
  History,
  Leaf,
  Folder,
  Shield
} from 'lucide-react'
import { useDataSource } from './DataSourceProvider'

const navItems = [
  { href: '/admin', label: 'Admin', icon: Shield },
  { href: '/projects', label: 'Projects', icon: Folder },
  { href: '/runs', label: 'Run History', icon: History },

]

export function Sidebar() {
  const pathname = usePathname()
  const { source, setSource, availableSources, loadingSources } = useDataSource()

  return (
    <aside className="sidebar-glass h-screen w-64 fixed left-0 top-0 flex flex-col border-r border-white/10 lg:relative lg:w-64 z-50">
      {/* Header */}
      <div className="p-6 border-b border-white/10">
        <div className="flex items-center gap-3">
          <div className="p-2 rounded-lg bg-primary">
            <Leaf className="w-5 h-5 text-white" />
          </div>
          <div>
            <h1 className="font-bold text-lg text-foreground">EcoCode</h1>
            <p className="text-xs text-muted-foreground">Energy Monitor</p>
          </div>
        </div>
      </div>

      <div className="px-4 py-4 border-b border-white/10">
        <p className="text-[11px] uppercase tracking-[0.18em] text-muted-foreground mb-3">
          Data Source
        </p>
        <div className="grid grid-cols-2 gap-2">
          {(['local', 'online'] as const).map((candidate) => {
            const available = availableSources.includes(candidate)
            const active = source === candidate

            return (
              <button
                key={candidate}
                type="button"
                onClick={() => setSource(candidate)}
                disabled={loadingSources || !available}
                className={`rounded-lg border px-3 py-2 text-sm font-medium transition-colors ${
                  active
                    ? 'border-primary/40 bg-primary/20 text-primary'
                    : available
                      ? 'border-white/10 bg-white/5 text-foreground hover:bg-white/10'
                      : 'border-white/5 bg-white/[0.03] text-muted-foreground/60 cursor-not-allowed'
                }`}
              >
                {candidate === 'local' ? 'Local' : 'Online'}
              </button>
            )
          })}
        </div>
      </div>

      {/* Navigation */}
      <nav className="flex-1 p-4 space-y-2">
        {navItems.map(({ href, label, icon: Icon }) => {
          const isActive = pathname === href
          return (
            <Link
              key={href}
              href={href}
              className={`flex items-center gap-3 px-4 py-3 rounded-lg transition-all duration-200 ${isActive
                  ? 'bg-primary/20 text-primary border border-primary/30'
                  : 'text-muted-foreground hover:text-foreground hover:bg-white/5'
                }`}
            >
              <Icon className="w-5 h-5" />
              <span className="font-medium">{label}</span>
            </Link>
          )
        })}
      </nav>

      {/* Footer */}
      <div className="p-4 border-t border-white/10 space-y-2 text-xs text-muted-foreground">
        <p>Dashboard v0.1</p>
        <p>Real-time energy monitoring</p>
      </div>
    </aside>
  )
}
