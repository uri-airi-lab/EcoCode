import { ArrowRight } from 'lucide-react'

const LAYERS = [
  {
    id: 'cli-sdk',
    label: 'CLI / Python SDK',
    sublabel: 'Your training script',
    color: 'bg-chart-4/10 border-chart-4/30 text-chart-4',
    dot: 'bg-chart-4',
    items: ['ecocode run -- python train.py', 'import ecocode; ecocode.run(...)'],
  },
  {
    id: 'monitor',
    label: 'Energy Monitor',
    sublabel: 'Hardware sensor layer',
    color: 'bg-chart-2/10 border-chart-2/30 text-chart-2',
    dot: 'bg-chart-2',
    items: ['CPU, DRAM & iGPU power sampling', 'Per-process GPU power', 'Process CPU & memory usage'],
  },
  {
    id: 'backend',
    label: 'API Server',
    sublabel: 'REST & real-time event stream',
    color: 'bg-primary/10 border-primary/30 text-primary',
    dot: 'bg-primary',
    items: ['Run lifecycle management', 'Persistent record storage', 'Live event broadcast to clients'],
  },
  {
    id: 'frontend',
    label: 'Dashboard',
    sublabel: 'Real-time visualisation',
    color: 'bg-chart-3/10 border-chart-3/30 text-chart-3',
    dot: 'bg-chart-3',
    items: ['Energy timeline charts', 'Per-GPU / per-socket breakdown', 'Carbon & cost summary'],
  },
]

export function ArchitectureDiagram() {
  return (
    <section id="architecture" className="relative py-24 px-4 overflow-hidden">
      <div className="absolute inset-0 landing-section-glow-left pointer-events-none" />

      <div className="relative z-10 max-w-6xl mx-auto">
        {/* Header */}
        <div className="text-center mb-14">
          <p className="text-sm font-medium text-primary tracking-widest uppercase mb-3">
            System architecture
          </p>
          <h2 className="text-3xl sm:text-4xl font-bold text-foreground mb-4">
            Four independent layers
          </h2>
          <p className="text-muted-foreground max-w-2xl mx-auto text-lg leading-relaxed">
            A CLI agent, a Python SDK, an API server, and a real-time dashboard — each
            independently usable, designed to compose cleanly.
          </p>
        </div>

        {/* Flow diagram */}
        <div className="flex flex-col lg:flex-row items-stretch gap-0">
          {LAYERS.map((layer, i) => (
            <div key={layer.id} className="flex flex-col lg:flex-row items-stretch flex-1">
              {/* Layer card */}
              <div className={`flex-1 border rounded-xl p-5 flex flex-col gap-3 ${layer.color} backdrop-blur-sm`}>
                <div className="flex items-center gap-2">
                  <span className={`w-2 h-2 rounded-full ${layer.dot} flex-shrink-0`} />
                  <span className="font-semibold text-sm">{layer.label}</span>
                </div>
                <p className="text-xs opacity-70">{layer.sublabel}</p>
                <ul className="space-y-1.5 mt-1">
                  {layer.items.map((item) => (
                    <li key={item} className="text-xs opacity-80 font-mono leading-snug">
                      {item}
                    </li>
                  ))}
                </ul>
              </div>

              {/* Arrow connector */}
              {i < LAYERS.length - 1 && (
                <div className="flex items-center justify-center py-2 lg:py-0 lg:px-2 text-muted-foreground/40">
                  <ArrowRight className="w-5 h-5 rotate-90 lg:rotate-0" />
                </div>
              )}
            </div>
          ))}
        </div>

        {/* Data path annotations */}
        <div className="mt-10 grid sm:grid-cols-2 lg:grid-cols-4 gap-4">
          {[
            { num: '①', label: 'Run lifecycle', detail: 'CLI → API server via HTTP (create, annotate, complete)' },
            { num: '②', label: 'Semantic markers', detail: 'CLI → Monitor via local IPC (~1 µs latency)' },
            { num: '③', label: 'Energy records', detail: 'Monitor → API server via HTTP every N seconds' },
            { num: '④', label: 'Live visualisation', detail: 'API server → Dashboard via real-time event stream' },
          ].map(({ num, label, detail }) => (
            <div key={num} className="flex gap-3 p-4 rounded-lg bg-card border border-border">
              <span className="text-primary font-bold text-lg leading-none flex-shrink-0">{num}</span>
              <div>
                <p className="text-sm font-medium text-foreground mb-0.5">{label}</p>
                <p className="text-xs text-muted-foreground leading-snug">{detail}</p>
              </div>
            </div>
          ))}
        </div>
      </div>
    </section>
  )
}
