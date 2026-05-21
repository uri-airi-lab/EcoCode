'use client'

import { useState } from 'react'

interface Mode {
  id: string
  label: string
  tag: string
  lines: string[]
  description: string
}

const MODES: Mode[] = [
  {
    id: 'context',
    label: 'Context Manager',
    tag: 'Recommended',
    description: 'Cleanest API. Automatically starts monitoring when entering the block and stops it on exit, even if an exception is raised.',
    lines: [
      'import ecocode',
      '',
      'with ecocode.run("my-experiment", api_key="eco_...") as run:',
      '    for epoch in range(epochs):',
      '        run.log_epoch(epoch)',
      '        for batch in loader:',
      '            run.mark("phase", "forward")',
      '            loss = model(batch)',
      '            run.log_loss(loss.item())',
      '            run.log_step(global_step)',
    ],
  },
  {
    id: 'decorator',
    label: 'Decorator',
    tag: 'Minimal',
    description: 'One annotation above your training function. The entire function body is monitored automatically.',
    lines: [
      'import ecocode',
      '',
      '@ecocode.track("my-experiment", api_key="eco_...")',
      'def train():',
      '    for epoch in range(epochs):',
      '        # ... training loop ...',
      '        pass',
      '',
      'train()',
    ],
  },
  {
    id: 'explicit',
    label: 'Explicit Start / Stop',
    tag: 'Jupyter / servers',
    description: 'For Jupyter notebooks or long-lived servers where a context manager does not fit. Call start() and stop() manually across cells or request handlers.',
    lines: [
      'import ecocode',
      '',
      'run = ecocode.run("notebook-experiment", api_key="eco_...")',
      'run.start()',
      '',
      '# ... training across multiple notebook cells ...',
      '',
      'run.stop()',
    ],
  },
  {
    id: 'cli',
    label: 'CLI Mode',
    tag: 'Any language',
    description: 'Wrap any executable with zero code changes. The CLI creates the run, monitors energy, and publishes records to the dashboard.',
    lines: [
      '# Monitor any process — output to terminal',
      'ecocode run -- python train.py',
      '',
      '# Send results to the web dashboard',
      'ecocode run -o online -p "my-project" -- python train.py',
      '',
      '# Any executable — not just Python',
      'ecocode run -o online -- ./my_simulation',
      'ecocode run -o online -- node server.js',
      '',
      '# Save to CSV',
      'ecocode run -o csv -f results.csv -- python train.py',
    ],
  },
]

export function UsageModesSection() {
  const [active, setActive] = useState('context')
  const current = MODES.find((m) => m.id === active)!

  return (
    <section id="usage" className="relative py-24 px-4 overflow-hidden">
      <div className="absolute inset-0 landing-section-glow-left pointer-events-none" />

      <div className="relative z-10 max-w-5xl mx-auto">
        {/* Header */}
        <div className="text-center mb-12">
          <p className="text-sm font-medium text-primary tracking-widest uppercase mb-3">
            Usage patterns
          </p>
          <h2 className="text-3xl sm:text-4xl font-bold text-foreground mb-4">
            Three lines to get started
          </h2>
          <p className="text-muted-foreground max-w-2xl mx-auto text-lg leading-relaxed">
            Pick the pattern that fits your workflow. All three report to the same dashboard
            and use the same API key.
          </p>
        </div>

        {/* Mode selector */}
        <div className="flex flex-wrap gap-2 mb-6 justify-center" role="tablist" aria-label="Usage modes">
          {MODES.map(({ id, label, tag }) => (
            <button
              key={id}
              role="tab"
              aria-selected={active === id}
              aria-controls={`usage-panel-${id}`}
              onClick={() => setActive(id)}
              className={`flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium transition-all duration-150 border ${
                active === id
                  ? 'bg-primary/10 border-primary/40 text-primary'
                  : 'border-border text-muted-foreground hover:text-foreground hover:bg-muted/50'
              }`}
            >
              {label}
              <span className={`text-[10px] px-1.5 py-0.5 rounded font-medium ${
                active === id ? 'bg-primary/20 text-primary' : 'bg-muted/50 text-muted-foreground'
              }`}>
                {tag}
              </span>
            </button>
          ))}
        </div>

        {/* Description */}
        <p className="text-center text-muted-foreground text-sm mb-6 max-w-xl mx-auto min-h-[2.5rem]">
          {current.description}
        </p>

        {/* Code panel */}
        <div
          id={`usage-panel-${current.id}`}
          role="tabpanel"
          aria-label={`${current.label} usage example`}
          className="rounded-xl border border-border bg-neutral-950 overflow-hidden"
        >
          <div className="flex items-center gap-1.5 px-4 py-3 border-b border-border/60 bg-neutral-900/60">
            <span className="w-3 h-3 rounded-full bg-red-500/70" />
            <span className="w-3 h-3 rounded-full bg-yellow-500/70" />
            <span className="w-3 h-3 rounded-full bg-green-500/70" />
            <span className="ml-3 text-xs text-muted-foreground font-mono">
              {current.id === 'cli' ? 'shell' : 'train.py'}
            </span>
          </div>
          <pre className="p-5 font-mono text-sm leading-relaxed overflow-x-auto">
            {current.lines.map((line, i) => (
              <div key={i}>
                <span className={
                  line.startsWith('#')
                    ? 'text-muted-foreground/60 italic'
                    : line.startsWith('import') || line.startsWith('from') || line === 'def train():'
                      ? 'text-chart-4'
                      : line.startsWith('@')
                        ? 'text-chart-2'
                        : 'text-foreground/90'
                }>
                  {line || '\u00a0'}
                </span>
                {'\n'}
              </div>
            ))}
          </pre>
        </div>

        {/* SDK marker API quick reference */}
        <div className="mt-8 grid sm:grid-cols-2 lg:grid-cols-4 gap-3">
          {[
            { call: 'run.mark("phase", "forward")', note: 'Label a training phase' },
            { call: 'run.log_loss(1.45)',            note: 'Record loss value' },
            { call: 'run.log_step(global_step)',     note: 'Record training step' },
            { call: 'run.log_tokens(count)',         note: 'Record tokens processed' },
          ].map(({ call, note }) => (
            <div key={call} className="p-3 rounded-lg bg-card border border-border">
              <code className="text-xs text-primary block mb-1 font-mono">{call}</code>
              <p className="text-xs text-muted-foreground">{note}</p>
            </div>
          ))}
        </div>
      </div>
    </section>
  )
}
