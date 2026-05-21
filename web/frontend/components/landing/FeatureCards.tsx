import {
  Cpu,
  GitBranch,
  Terminal,
  Activity,
  Users,
  Leaf,
} from 'lucide-react'
import type { LucideIcon } from 'lucide-react'

interface Feature {
  Icon: LucideIcon
  title: string
  description: string
  detail: string
}

const FEATURES: Feature[] = [
  {
    Icon: Cpu,
    title: 'Real Hardware Readings',
    description:
      'Reads hardware power counters directly — the same interfaces used by power-management firmware and hardware performance engineers.',
    detail: 'Not TDP × utilisation estimates.',
  },
  {
    Icon: GitBranch,
    title: 'Per-Process GPU Attribution',
    description:
      'Attributes GPU energy to your specific process across every detected GPU independently, with per-device breakdown — not whole-device averaging.',
    detail: 'No whole-device averaging.',
  },
  {
    Icon: Terminal,
    title: 'Language & Runtime Agnostic',
    description:
      'Wrap any executable with the CLI — python, ./simulation, node server.js — without touching the target program. Or instrument from inside Python with the SDK.',
    detail: 'Zero changes to target code.',
  },
  {
    Icon: Activity,
    title: 'Live Real-Time Dashboard',
    description:
      'The server streams each energy record to the dashboard as it arrives. Charts update every second while your run is still in progress.',
    detail: 'No polling. No refresh.',
  },
  {
    Icon: Users,
    title: 'Team Workspaces',
    description:
      'Shared workspaces with admin and member roles, invitation by email, and workspace-scoped API keys. Designed for collaborative ML research teams.',
    detail: 'Project-level run grouping.',
  },
  {
    Icon: Leaf,
    title: 'Carbon Footprint',
    description:
      'CO₂ estimation using a 50-country IEA 2025 carbon intensity table or live Electricity Maps API. Outputs car distance, smartphone charges, and tree equivalents.',
    detail: 'PUE-corrected, per-run.',
  },
]

export function FeatureCards() {
  return (
    <section id="features" className="relative py-24 px-4 overflow-hidden">
      <div className="absolute inset-0 landing-section-glow-left pointer-events-none" />

      <div className="relative z-10 max-w-7xl mx-auto">
        {/* Section header */}
        <div className="text-center mb-16">
          <p className="text-sm font-medium text-primary tracking-widest uppercase mb-3">
            Core capabilities
          </p>
          <h2 className="text-3xl sm:text-4xl font-bold text-foreground mb-4">
            Why EcoCode is different
          </h2>
          <p className="text-muted-foreground max-w-2xl mx-auto text-lg leading-relaxed">
            Most energy-monitoring tools estimate power from manufacturer TDP figures.
            EcoCode reads what the hardware actually reports.
          </p>
        </div>

        {/* 2 × 3 grid */}
        <div className="grid sm:grid-cols-2 lg:grid-cols-3 gap-6">
          {FEATURES.map(({ Icon, title, description, detail }) => (
            <article key={title} className="landing-card group flex flex-col gap-4">
              <div className="w-10 h-10 rounded-lg bg-primary/10 border border-primary/20 flex items-center justify-center flex-shrink-0 group-hover:bg-primary/15 transition-colors duration-300">
                <Icon className="w-5 h-5 text-primary" aria-hidden />
              </div>

              <div>
                <h3 className="font-semibold text-foreground mb-2">{title}</h3>
                <p className="text-sm text-muted-foreground leading-relaxed">{description}</p>
              </div>

              <div className="mt-auto pt-3 border-t border-border/50">
                <span className="text-xs font-medium text-primary/80">{detail}</span>
              </div>
            </article>
          ))}
        </div>
      </div>
    </section>
  )
}
