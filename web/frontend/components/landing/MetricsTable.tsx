const METRICS = [
  { metric: 'CPU Usage (%)',     source: 'sysinfo',        granularity: 'Per-process' },
  { metric: 'CPU Energy (J)',    source: 'Hardware power counters (CPU, DRAM, iGPU)', granularity: 'Per-socket, per-domain' },
  { metric: 'GPU Usage (%)',     source: 'Per-process GPU utilisation sampling',       granularity: 'Per-GPU, per-process' },
  { metric: 'GPU Energy (J)',    source: 'Per-process GPU energy counter',             granularity: 'Per-GPU' },
  { metric: 'Memory Usage (%)', source: 'Process memory sampling (PSS)',              granularity: 'Per-process' },
  { metric: 'Memory Energy (J)',source: 'Hardware DRAM power counter',                granularity: 'Per-socket' },
  { metric: 'iGPU Energy (J)',  source: 'Hardware uncore power counter',              granularity: 'Per-socket' },
]

export function MetricsTable() {
  return (
    <section id="metrics" className="relative py-24 px-4 overflow-hidden">
      <div className="absolute inset-0 landing-section-glow-left pointer-events-none" />

      <div className="relative z-10 max-w-4xl mx-auto">
        <div className="text-center mb-12">
          <p className="text-sm font-medium text-primary tracking-widest uppercase mb-3">
            Measurement coverage
          </p>
          <h2 className="text-3xl sm:text-4xl font-bold text-foreground mb-4">
            What gets monitored
          </h2>
          <p className="text-muted-foreground max-w-2xl mx-auto text-lg leading-relaxed">
            Every metric comes directly from a kernel or hardware interface.
            Energy is stored in Joules (1 Wh = 3600 J).
          </p>
        </div>

        <div className="overflow-x-auto rounded-xl border border-border">
          <table className="w-full text-sm" aria-label="EcoCode monitored metrics">
            <thead>
              <tr className="bg-card/80 border-b border-border text-left">
                <th className="px-5 py-3.5 font-medium text-muted-foreground">Metric</th>
                <th className="px-5 py-3.5 font-medium text-muted-foreground">Source</th>
                <th className="px-5 py-3.5 font-medium text-muted-foreground">Granularity</th>
              </tr>
            </thead>
            <tbody>
              {METRICS.map((row, i) => (
                <tr
                  key={row.metric}
                  className={`border-b border-border/50 hover:bg-muted/20 transition-colors ${
                    i % 2 === 0 ? 'bg-background' : 'bg-card/30'
                  }`}
                >
                  <td className="px-5 py-3.5 font-semibold text-foreground">{row.metric}</td>
                  <td className="px-5 py-3.5 font-mono text-xs text-primary/80 leading-relaxed">{row.source}</td>
                  <td className="px-5 py-3.5 text-muted-foreground text-sm">{row.granularity}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>

        {/* Counter wrap-around note */}
        <div className="mt-5 px-5 py-4 rounded-xl border border-border bg-card/50 text-sm text-muted-foreground leading-relaxed">
          <strong className="text-foreground">Counter wrap-around correction:</strong>{' '}
          Hardware energy counters are monotonically increasing integers that reset to zero on certain hardware events
          (driver reload, GPU reset). EcoCode tracks the running maximum for each counter and applies a correction
          when a backward jump is detected, ensuring no energy is lost across counter resets.
        </div>
      </div>
    </section>
  )
}
