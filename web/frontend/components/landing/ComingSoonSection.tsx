import { PackageOpen, Map, Clock } from 'lucide-react'

const ITEMS = [
  {
    Icon: PackageOpen,
    title: 'Python SDK on PyPI',
    description:
      'pip install ecocode — platform-specific wheels for the bundled ecocode-monitor binary are under development. The SDK is currently installable from source.',
    status: 'In progress',
    snippet: [
      '# Coming soon — install from source in the meantime:',
      'git clone https://github.com/MohamedAziz-Khezami/EcoCode.git',
      'cd EcoCode/python-sdk',
      'pip install -e ".[all]"',
    ],
  },
  {
    Icon: Map,
    title: 'Global Run Map',
    description:
      'A world map view of all runs, with circles centred at their recorded location. Public runs visible without login; authenticated users see their full history. Click a circle for run details.',
    status: 'Planned',
    snippet: null,
  },
]

export function ComingSoonSection() {
  return (
    <section id="roadmap" className="relative py-24 px-4 overflow-hidden">
      <div className="absolute inset-0 landing-section-glow-right pointer-events-none" />

      <div className="relative z-10 max-w-4xl mx-auto">
        <div className="text-center mb-12">
          <div className="inline-flex items-center gap-2 px-3 py-1 rounded-full border border-primary/30 bg-primary/5 text-xs font-medium text-primary mb-4">
            <Clock className="w-3.5 h-3.5" />
            Roadmap
          </div>
          <h2 className="text-3xl sm:text-4xl font-bold text-foreground mb-4">
            Coming soon
          </h2>
          <p className="text-muted-foreground max-w-xl mx-auto text-lg leading-relaxed">
            Features under active development. Watch the repository for updates.
          </p>
        </div>

        <div className="grid sm:grid-cols-2 gap-6">
          {ITEMS.map(({ Icon, title, description, status, snippet }) => (
            <div key={title} className="landing-card flex flex-col gap-4">
              <div className="flex items-start justify-between gap-3">
                <div className="flex items-center gap-3">
                  <div className="w-9 h-9 rounded-lg bg-primary/10 border border-primary/20 flex items-center justify-center flex-shrink-0">
                    <Icon className="w-4.5 h-4.5 text-primary" aria-hidden />
                  </div>
                  <h3 className="font-semibold text-foreground text-sm leading-tight">{title}</h3>
                </div>
                <span className="flex-shrink-0 text-[10px] font-semibold px-2 py-0.5 rounded-full border border-primary/30 bg-primary/5 text-primary">
                  {status}
                </span>
              </div>

              <p className="text-sm text-muted-foreground leading-relaxed">{description}</p>

              {snippet && (
                <div className="rounded-lg border border-border bg-neutral-950 overflow-hidden">
                  <div className="px-3 py-1.5 border-b border-border/60 bg-neutral-900/60 flex items-center gap-1.5">
                    <span className="w-2 h-2 rounded-full bg-red-500/50" />
                    <span className="w-2 h-2 rounded-full bg-yellow-500/50" />
                    <span className="w-2 h-2 rounded-full bg-green-500/50" />
                    <span className="ml-2 text-[10px] text-muted-foreground font-mono">shell</span>
                  </div>
                  <pre className="p-3 font-mono text-xs leading-relaxed overflow-x-auto">
                    {snippet.map((line, i) => (
                      <div key={i}>
                        <span className={line.startsWith('#') ? 'text-muted-foreground/60 italic' : 'text-foreground/85'}>
                          {line}
                        </span>
                        {'\n'}
                      </div>
                    ))}
                  </pre>
                </div>
              )}
            </div>
          ))}
        </div>
      </div>
    </section>
  )
}
