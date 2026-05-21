import { Check, X, Minus } from 'lucide-react'

type CellValue = 'yes' | 'no' | 'partial'

interface Row {
  capability: string
  ecocode:      CellValue
  codecarbon:   CellValue
  scaphandre:   CellValue
  carbontracker: CellValue
  kepler:       CellValue
}

const ROWS: Row[] = [
  { capability: 'Real hardware energy (RAPL)',        ecocode: 'yes', codecarbon: 'yes',     scaphandre: 'yes',     carbontracker: 'yes',     kepler: 'yes'     },
  { capability: 'Real GPU energy (NVML energy ctr.)', ecocode: 'yes', codecarbon: 'yes',     scaphandre: 'no',      carbontracker: 'partial', kepler: 'partial' },
  { capability: 'Per-process GPU attribution',        ecocode: 'yes', codecarbon: 'no',      scaphandre: 'no',      carbontracker: 'no',      kepler: 'partial' },
  { capability: 'Multi-GPU per-device breakdown',     ecocode: 'yes', codecarbon: 'yes',     scaphandre: 'no',      carbontracker: 'yes',     kepler: 'yes'     },
  { capability: 'Multi-socket per-domain breakdown',  ecocode: 'yes', codecarbon: 'partial', scaphandre: 'partial', carbontracker: 'partial', kepler: 'partial' },
  { capability: 'Any language / any process',         ecocode: 'yes', codecarbon: 'no',      scaphandre: 'yes',     carbontracker: 'no',      kepler: 'yes'     },
  { capability: 'Live real-time dashboard (SSE)',     ecocode: 'yes', codecarbon: 'no',      scaphandre: 'no',      carbontracker: 'no',      kepler: 'no'      },
  { capability: 'Team workspaces & collaboration',    ecocode: 'yes', codecarbon: 'no',      scaphandre: 'no',      carbontracker: 'no',      kepler: 'no'      },
  { capability: 'Prometheus export',                  ecocode: 'yes', codecarbon: 'partial', scaphandre: 'yes',     carbontracker: 'no',      kepler: 'yes'     },
  { capability: 'Live grid carbon intensity (API)',   ecocode: 'yes', codecarbon: 'yes',     scaphandre: 'no',      carbontracker: 'yes',     kepler: 'no'      },
  { capability: 'Counter wrap-around correction',     ecocode: 'yes', codecarbon: 'yes',     scaphandre: 'no',      carbontracker: 'no',      kepler: 'yes'     },
]

function Cell({ value, highlight = false }: { value: CellValue; highlight?: boolean }) {
  if (value === 'yes') return (
    <span className={`inline-flex items-center justify-center w-6 h-6 rounded-full ${highlight ? 'bg-primary/15' : 'bg-muted/40'}`}>
      <Check className={`w-3.5 h-3.5 ${highlight ? 'text-primary' : 'text-muted-foreground'}`} strokeWidth={2.5} />
    </span>
  )
  if (value === 'partial') return (
    <span className="inline-flex items-center justify-center w-6 h-6 rounded-full bg-chart-2/10">
      <Minus className="w-3.5 h-3.5 text-chart-2" strokeWidth={2.5} />
    </span>
  )
  return (
    <span className="inline-flex items-center justify-center w-6 h-6 rounded-full bg-muted/20">
      <X className="w-3.5 h-3.5 text-muted-foreground/40" strokeWidth={2} />
    </span>
  )
}

const COLS = [
  { key: 'ecocode' as const,       label: 'EcoCode',       highlight: true  },
  { key: 'codecarbon' as const,    label: 'CodeCarbon',    highlight: false },
  { key: 'scaphandre' as const,    label: 'Scaphandre',    highlight: false },
  { key: 'carbontracker' as const, label: 'CarbonTracker', highlight: false },
  { key: 'kepler' as const,        label: 'Kepler',        highlight: false },
]

export function ComparisonTable() {
  return (
    <section id="comparison" className="relative py-24 px-4 overflow-hidden">
      <div className="absolute inset-0 landing-section-glow-right pointer-events-none" />

      <div className="relative z-10 max-w-5xl mx-auto">
        {/* Header */}
        <div className="text-center mb-12">
          <p className="text-sm font-medium text-primary tracking-widest uppercase mb-3">
            Tool comparison
          </p>
          <h2 className="text-3xl sm:text-4xl font-bold text-foreground mb-4">
            How EcoCode compares
          </h2>
          <p className="text-muted-foreground max-w-2xl mx-auto text-lg leading-relaxed">
            A capability-by-capability comparison against the most widely used energy measurement tools
            in the ML and systems research community.
          </p>
        </div>

        {/* Table */}
        <div className="overflow-x-auto rounded-xl border border-border">
          <table className="w-full text-sm" aria-label="EcoCode feature comparison table">
            <thead>
              <tr className="bg-card/80 border-b border-border">
                <th className="text-left px-5 py-4 font-medium text-muted-foreground w-auto">
                  Capability
                </th>
                {COLS.map(({ label, highlight }) => (
                  <th
                    key={label}
                    className={`px-5 py-4 font-semibold text-center whitespace-nowrap ${
                      highlight ? 'text-primary bg-primary/5' : 'text-muted-foreground'
                    }`}
                  >
                    {label}
                    {highlight && (
                      <span className="ml-1.5 inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-bold bg-primary/20 text-primary">
                        YOU
                      </span>
                    )}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {ROWS.map((row, i) => (
                <tr
                  key={row.capability}
                  className={`border-b border-border/50 transition-colors hover:bg-muted/20 ${
                    i % 2 === 0 ? 'bg-background' : 'bg-card/30'
                  }`}
                >
                  <td className="px-5 py-3.5 text-foreground/90 font-medium">{row.capability}</td>
                  {COLS.map(({ key, highlight }) => (
                    <td key={key} className={`px-5 py-3.5 text-center ${highlight ? 'bg-primary/[0.03]' : ''}`}>
                      <Cell value={row[key]} highlight={highlight} />
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>

        {/* Footnote */}
        <p className="mt-5 text-xs text-muted-foreground/70 leading-relaxed max-w-3xl">
          Partial (–) indicates limited support. <strong className="text-muted-foreground">GPU energy:</strong> Kepler
          and CarbonTracker derive per-process GPU energy from instantaneous power sampling rather than the NVML
          cumulative energy counter, introducing integration error over short intervals; Kepler additionally uses
          SM-utilization proportional attribution for per-process GPU (hardware-isolated counters not available outside
          MIG). <strong className="text-muted-foreground">Multi-socket:</strong> CodeCarbon and CarbonTracker expose
          package and DRAM per socket but omit uncore; Scaphandre and Kepler aggregate same-named domains across
          sockets instead of exposing per-socket series. <strong className="text-muted-foreground">Prometheus:</strong>{' '}
          CodeCarbon uses push-only (Pushgateway); Scaphandre and Kepler expose a native pull endpoint.
          Scaphandre has no GPU support; Kepler is primarily designed for Kubernetes and container workloads.
        </p>
      </div>
    </section>
  )
}
