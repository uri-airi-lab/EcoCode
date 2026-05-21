'use client'

import { Monitor, BarChart3, Cpu, Leaf, ImageIcon } from 'lucide-react'

// ─── Screenshot slot types ───────────────────────────────────────────────────

interface ScreenshotSlot {
  id: string
  /** Path to the actual screenshot once taken — leave as undefined for placeholder */
  src?: string
  alt: string
  /** Short label shown inside the placeholder */
  label: string
  /** What page/view to screenshot */
  instructions: string
  Icon: React.FC<{ className?: string }>
  /** Aspect ratio class */
  aspect: string
}

// ─── Screenshot definitions ──────────────────────────────────────────────────

export const SCREENSHOTS: ScreenshotSlot[] = [
  {
    id: 'live-run-dashboard',
    src: '/screenshots/live-run-dashboard.png',
    alt: 'EcoCode live run dashboard showing real-time energy charts and phase timeline',
    label: 'Live Run Dashboard',
    instructions:
      'Navigate to a run that is in progress (or a completed run with many records). ' +
      'Capture the full-width view showing the energy timeline chart + phase timeline together. ' +
      'Ideal width: 1400 px. Save as /public/screenshots/live-run-dashboard.png',
    Icon: Monitor,
    aspect: 'aspect-[16/9]',
  },
  {
    id: 'runs-list',
    src: '/screenshots/runs-list.png',
    alt: 'EcoCode runs list showing multiple experiments with energy, CO₂ and duration columns',
    label: 'Runs List',
    instructions:
      'Navigate to /{workspace}/runs. Populate at least 4–5 runs so the table looks full. ' +
      'Capture the full runs table including the summary row (total energy, CO₂, duration). ' +
      'Save as /public/screenshots/runs-list.png',
    Icon: BarChart3,
    aspect: 'aspect-[16/9]',
  },
  {
    id: 'gpu-socket-breakdown',
    // src: '/screenshots/gpu-socket-breakdown.png',
    alt: 'EcoCode per-GPU and per-socket energy breakdown charts',
    label: 'Multi-GPU & Socket Breakdown',
    instructions:
      'Navigate to a run with multiple GPUs or sockets. ' +
      'Capture the lower section of the run detail page showing the per-GPU energy bar chart ' +
      'and the per-socket RAPL breakdown side by side. ' +
      'Save as /public/screenshots/gpu-socket-breakdown.png',
    Icon: Cpu,
    aspect: 'aspect-[16/9]',
  },
  {
    id: 'carbon-impact',
    src: '/screenshots/carbon-impact.png',
    alt: 'EcoCode environmental impact panel showing CO₂ equivalents in car distance, smartphone charges and trees',
    label: 'Carbon & Environmental Impact',
    instructions:
      'Navigate to any completed run. Scroll to the "Environmental Impact" panel. ' +
      'Capture the panel showing gCO₂, car km, smartphone charges, and tree-days equivalents. ' +
      'If possible, include the carbon intensity source (Electricity Maps / static table) label. ' +
      'Save as /public/screenshots/carbon-impact.png',
    Icon: Leaf,
    aspect: 'aspect-[4/3]',
  },
]

// ─── Placeholder component ────────────────────────────────────────────────────

function ScreenshotPlaceholder({ slot, className = '' }: { slot: ScreenshotSlot; className?: string }) {
  const { Icon, label, instructions, alt, aspect } = slot

  if (slot.src) {
    return (
      // Once you drop the real screenshot in, this renders it directly
      // eslint-disable-next-line @next/next/no-img-element
      <img
        src={slot.src}
        alt={alt}
        className={`w-full h-full object-cover object-top rounded-xl ${className}`}
      />
    )
  }

  return (
    <div
      className={`
        relative w-full ${aspect} rounded-xl border-2 border-dashed border-primary/30
        bg-card/50 backdrop-blur-sm overflow-hidden flex flex-col items-center justify-center
        gap-4 group hover:border-primary/50 transition-colors duration-300 ${className}
      `}
      role="img"
      aria-label={`Screenshot placeholder: ${label}`}
    >
      {/* Subtle grid background */}
      <div className="absolute inset-0 landing-grid opacity-40 pointer-events-none" />

      {/* Animated shimmer */}
      <div className="absolute inset-0 bg-gradient-to-r from-transparent via-primary/[0.03] to-transparent -translate-x-full group-hover:translate-x-full transition-transform duration-[1500ms] ease-in-out pointer-events-none" />

      {/* Icon */}
      <div className="relative z-10 flex flex-col items-center gap-3 px-6 text-center">
        <div className="w-14 h-14 rounded-xl bg-primary/10 border border-primary/20 flex items-center justify-center">
          <Icon className="w-7 h-7 text-primary/60" />
        </div>

        <div>
          <p className="font-semibold text-foreground/70 text-sm mb-1">{label}</p>
          <div className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-md bg-primary/10 border border-primary/20 text-[10px] font-mono text-primary/70">
            <ImageIcon className="w-3 h-3" />
            screenshot placeholder
          </div>
        </div>

        {/* Instructions tooltip on hover */}
        <p className="text-xs text-muted-foreground/60 leading-relaxed max-w-xs mt-1 opacity-0 group-hover:opacity-100 transition-opacity duration-300">
          {instructions}
        </p>
      </div>

      {/* Corner badge */}
      <div className="absolute top-3 right-3 text-[10px] px-2 py-0.5 rounded-full bg-primary/10 border border-primary/20 text-primary/60 font-mono">
        {slot.id}
      </div>
    </div>
  )
}

// ─── Exported section components ─────────────────────────────────────────────

/**
 * § A — Full-width product preview placed just below the Hero.
 * Screenshot: Live Run Dashboard (#1)
 */
export function HeroScreenshot() {
  const slot = SCREENSHOTS[0]
  return (
    <section className="relative px-4 pb-8 -mt-4">
      <div className="max-w-6xl mx-auto">
        <div className="rounded-2xl border border-border overflow-hidden shadow-2xl shadow-black/40 ring-1 ring-primary/10">
          {/* Fake browser chrome */}
          <div className="flex items-center gap-1.5 px-4 py-3 bg-neutral-900 border-b border-border">
            <span className="w-3 h-3 rounded-full bg-red-500/70" />
            <span className="w-3 h-3 rounded-full bg-yellow-500/70" />
            <span className="w-3 h-3 rounded-full bg-green-500/70" />
            <span className="ml-4 flex-1 bg-neutral-800 rounded-md px-3 py-1 text-xs text-muted-foreground/50 font-mono">
              app.ecocode.dev/{'{workspace}'}/runs/{'{run-id}'}
            </span>
          </div>
          <ScreenshotPlaceholder slot={slot} />
        </div>
        <p className="text-center text-xs text-muted-foreground/50 mt-3">
          Real-time energy dashboard — charts update every second via SSE
        </p>
      </div>
    </section>
  )
}

/**
 * § B — 2-up screenshots placed after the Comparison Table.
 * Left: Runs List (#2) · Right: Multi-GPU breakdown (#3)
 */
export function DashboardScreenshots() {
  const [runsSlot, gpuSlot] = [SCREENSHOTS[1], SCREENSHOTS[2]]
  return (
    <section className="relative py-16 px-4 overflow-hidden">
      <div className="absolute inset-0 landing-section-glow-left pointer-events-none" />
      <div className="relative z-10 max-w-6xl mx-auto">
        <div className="text-center mb-10">
          <p className="text-sm font-medium text-primary tracking-widest uppercase mb-3">
            Product preview
          </p>
          <h2 className="text-2xl sm:text-3xl font-bold text-foreground mb-3">
            A workspace built for ML researchers
          </h2>
          <p className="text-muted-foreground max-w-xl mx-auto">
            Track every run across your team — energy, CO₂, training metrics, and hardware
            breakdown in one place.
          </p>
        </div>

        <div className="grid md:grid-cols-2 gap-6">
          <div>
            <ScreenshotPlaceholder slot={runsSlot} />
            <p className="mt-2 text-center text-xs text-muted-foreground/60">
              Runs list — sortable by energy, CO₂, duration, or project
            </p>
          </div>
          <div>
            <ScreenshotPlaceholder slot={gpuSlot} />
            <p className="mt-2 text-center text-xs text-muted-foreground/60">
              Per-GPU & per-socket RAPL energy breakdown
            </p>
          </div>
        </div>
      </div>
    </section>
  )
}

/**
 * § C — Carbon impact screenshot placed before the CTA.
 * Full-width with descriptive copy alongside.
 */
export function CarbonScreenshot() {
  const slot = SCREENSHOTS[3]
  return (
    <section className="relative py-16 px-4 overflow-hidden">
      <div className="absolute inset-0 landing-section-glow-right pointer-events-none" />
      <div className="relative z-10 max-w-6xl mx-auto">
        <div className="grid lg:grid-cols-2 gap-10 items-center">
          {/* Copy */}
          <div>
            <p className="text-sm font-medium text-primary tracking-widest uppercase mb-3">
              Environmental impact
            </p>
            <h2 className="text-3xl sm:text-4xl font-bold text-foreground mb-5 leading-tight">
              From joules to real-world equivalents
            </h2>
            <p className="text-muted-foreground leading-relaxed mb-6">
              Every completed run automatically calculates its CO₂ footprint using the
              IEA 2025 carbon intensity table (or live Electricity Maps data if configured).
              Results are expressed in units everyone understands:
            </p>
            <ul className="space-y-3">
              {[
                { icon: '🚗', label: 'Car distance (km)', detail: '170 gCO₂/km — IEA global passenger fleet average' },
                { icon: '📱', label: 'Smartphone charges', detail: '~16 Wh battery at world-average grid intensity' },
                { icon: '🌳', label: 'Tree-days absorbed', detail: '22 kg CO₂/year per tree — EcoTree species average' },
              ].map(({ icon, label, detail }) => (
                <li key={label} className="flex items-start gap-3">
                  <span className="text-lg leading-none mt-0.5" aria-hidden>{icon}</span>
                  <div>
                    <p className="text-sm font-medium text-foreground">{label}</p>
                    <p className="text-xs text-muted-foreground">{detail}</p>
                  </div>
                </li>
              ))}
            </ul>
          </div>

          {/* Screenshot */}
          <div>
            <ScreenshotPlaceholder slot={slot} />
            <p className="mt-2 text-center text-xs text-muted-foreground/60">
              Environmental impact panel — shown on every completed run
            </p>
          </div>
        </div>
      </div>
    </section>
  )
}
