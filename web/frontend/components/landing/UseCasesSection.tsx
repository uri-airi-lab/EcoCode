import { FlaskConical, GitCommitHorizontal, Zap } from 'lucide-react'
import type { LucideIcon } from 'lucide-react'

interface UseCase {
  Icon: LucideIcon
  title: string
  subtitle: string
  description: string
  snippet: string[]
  isShell?: boolean
}

const USE_CASES: UseCase[] = [
  {
    Icon: FlaskConical,
    title: 'Research',
    subtitle: 'Compare experiment energy cost',
    description:
      'Run precision variants back-to-back and use the dashboard\'s run comparison panel to see side-by-side energy and CO₂ figures. Ideal for ablation studies and efficiency benchmarks.',
    snippet: [
      '# Baseline vs optimised — same project, different runs',
      'ecocode run -o online \\',
      '    --project "efficiency-study" \\',
      '    --name "fp32-baseline" \\',
      '    -- python train.py --precision fp32',
      '',
      'ecocode run -o online \\',
      '    --project "efficiency-study" \\',
      '    --name "bf16-amp" \\',
      '    -- python train.py --precision bf16',
    ],
    isShell: true,
  },
  {
    Icon: GitCommitHorizontal,
    title: 'CI / CD',
    subtitle: 'Per-commit energy regression tracking',
    description:
      'Add EcoCode to your GitHub Actions workflow to track energy per commit. Catch regressions before they reach production — a green CI pipeline that is also energy-efficient.',
    snippet: [
      '# .github/workflows/energy.yml',
      'ecocode run -o online \\',
      '    --project "ci-energy" \\',
      '    --name "commit-${GITHUB_SHA:0:8}" \\',
      '    -- python run_benchmarks.py',
    ],
    isShell: true,
  },
  {
    Icon: Zap,
    title: 'Inference Profiling',
    subtitle: 'vLLM prefill vs decode energy',
    description:
      'Track exactly how much energy goes into prefill (compute-bound) vs decode (memory-bandwidth-bound). Tune prompt length and batching strategies with real data.',
    snippet: [
      'from ecocode.integrations.vllm import EcoCodeVLLMMiddleware',
      '',
      'with ecocode.run("llama3-serving", api_key="eco_...") as r:',
      '    engine = EcoCodeVLLMMiddleware(',
      '        AsyncLLMEngine.from_engine_args(args), r',
      '    )',
      '    async for output in engine.generate(prompt, params, id):',
      '        yield output  # prefill/decode phases marked automatically',
    ],
  },
]

export function UseCasesSection() {
  return (
    <section id="use-cases" className="relative py-24 px-4 overflow-hidden">
      <div className="absolute inset-0 landing-section-glow-right pointer-events-none" />

      <div className="relative z-10 max-w-6xl mx-auto">
        {/* Header */}
        <div className="text-center mb-14">
          <p className="text-sm font-medium text-primary tracking-widest uppercase mb-3">
            Use cases
          </p>
          <h2 className="text-3xl sm:text-4xl font-bold text-foreground mb-4">
            Built for researchers and engineers
          </h2>
          <p className="text-muted-foreground max-w-2xl mx-auto text-lg leading-relaxed">
            Whether you are running ablation studies, shipping to production, or profiling LLM inference —
            EcoCode has a workflow for it.
          </p>
        </div>

        {/* Use case cards */}
        <div className="grid lg:grid-cols-3 gap-6">
          {USE_CASES.map(({ Icon, title, subtitle, description, snippet, isShell }) => (
            <article key={title} className="landing-card flex flex-col gap-4">
              {/* Icon + label */}
              <div className="flex items-center gap-3">
                <div className="w-9 h-9 rounded-lg bg-primary/10 border border-primary/20 flex items-center justify-center flex-shrink-0">
                  <Icon className="w-4.5 h-4.5 text-primary" aria-hidden />
                </div>
                <div>
                  <p className="text-xs font-medium text-primary">{title}</p>
                  <h3 className="text-sm font-semibold text-foreground leading-tight">{subtitle}</h3>
                </div>
              </div>

              <p className="text-sm text-muted-foreground leading-relaxed">{description}</p>

              {/* Code block */}
              <div className="mt-auto rounded-lg border border-border bg-neutral-950 overflow-hidden">
                <div className="px-3 py-1.5 border-b border-border/60 bg-neutral-900/60 flex items-center gap-1.5">
                  <span className="w-2 h-2 rounded-full bg-red-500/50" />
                  <span className="w-2 h-2 rounded-full bg-yellow-500/50" />
                  <span className="w-2 h-2 rounded-full bg-green-500/50" />
                  <span className="ml-2 text-[10px] text-muted-foreground font-mono">
                    {isShell ? 'shell' : 'train.py'}
                  </span>
                </div>
                <pre className="p-3 font-mono text-xs leading-relaxed overflow-x-auto">
                  {snippet.map((line, i) => (
                    <div key={i}>
                      <span className={line.startsWith('#') ? 'text-muted-foreground/60 italic' : 'text-foreground/85'}>
                        {line || '\u00a0'}
                      </span>
                      {'\n'}
                    </div>
                  ))}
                </pre>
              </div>
            </article>
          ))}
        </div>
      </div>
    </section>
  )
}
