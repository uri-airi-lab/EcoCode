'use client'

import { useEffect, useRef, useState } from 'react'
import Link from 'next/link'
import { ArrowRight, Github, ChevronDown } from 'lucide-react'

const STATS = [
  { value: 7,   suffix: '',  label: 'export formats' },
  { value: 7,   suffix: '',  label: 'framework integrations' },
  { value: 50,  suffix: '+', label: 'country carbon table' },
  { value: 1,   suffix: 'µs', label: 'marker overhead' },
]

function useCountUp(target: number, duration = 1200, start = false) {
  const [count, setCount] = useState(0)
  useEffect(() => {
    if (!start) return
    let startTime: number | null = null
    const step = (ts: number) => {
      if (!startTime) startTime = ts
      const progress = Math.min((ts - startTime) / duration, 1)
      setCount(Math.floor(progress * target))
      if (progress < 1) requestAnimationFrame(step)
    }
    requestAnimationFrame(step)
  }, [start, target, duration])
  return count
}

const CODE_LINES = [
  { tokens: [{ text: '# Install the Python SDK', type: 'comment' }] },
  { tokens: [{ text: 'pip install ecocode', type: 'command' }] },
  { tokens: [] },
  { tokens: [{ text: 'import', type: 'keyword' }, { text: ' ecocode', type: 'plain' }] },
  { tokens: [] },
  { tokens: [{ text: 'with', type: 'keyword' }, { text: ' ecocode.', type: 'plain' }, { text: 'run', type: 'fn' }, { text: '(', type: 'plain' }, { text: '"llama3-finetune"', type: 'string' }, { text: ',', type: 'plain' }] },
  { tokens: [{ text: '         api_key=', type: 'plain' }, { text: '"eco_..."', type: 'string' }, { text: ') as run:', type: 'plain' }] },
  { tokens: [{ text: '    ', type: 'plain' }, { text: 'for', type: 'keyword' }, { text: ' epoch ', type: 'plain' }, { text: 'in', type: 'keyword' }, { text: ' range(epochs):', type: 'plain' }] },
  { tokens: [{ text: '        run.', type: 'plain' }, { text: 'log_epoch', type: 'fn' }, { text: '(epoch)', type: 'plain' }] },
  { tokens: [{ text: '        run.', type: 'plain' }, { text: 'log_loss', type: 'fn' }, { text: '(loss.item())', type: 'plain' }] },
]

const TOKEN_COLOURS: Record<string, string> = {
  comment: 'text-muted-foreground/70 italic',
  command:  'text-primary',
  keyword:  'text-chart-4',
  plain:    'text-foreground/90',
  fn:       'text-chart-2',
  string:   'text-chart-3',
}

export function HeroSection() {
  const [visible, setVisible] = useState(false)
  const ref = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const observer = new IntersectionObserver(([e]) => {
      if (e.isIntersecting) setVisible(true)
    }, { threshold: 0.1 })
    if (ref.current) observer.observe(ref.current)
    return () => observer.disconnect()
  }, [])

  const c0 = useCountUp(STATS[0].value, 800, visible)
  const c1 = useCountUp(STATS[1].value, 900, visible)
  const c2 = useCountUp(STATS[2].value, 1000, visible)

  return (
    <section
      ref={ref}
      className="relative min-h-screen flex flex-col items-center justify-center pt-24 pb-16 px-4 overflow-hidden landing-grid"
    >
      {/* Glow layers */}
      <div className="absolute inset-0 landing-hero-glow pointer-events-none" />
      <div className="absolute top-1/4 left-1/4 w-96 h-96 bg-primary/5 rounded-full blur-[120px] pointer-events-none" />
      <div className="absolute bottom-1/4 right-1/4 w-64 h-64 bg-primary/8 rounded-full blur-[80px] pointer-events-none" />

      <div className="relative z-10 max-w-7xl w-full mx-auto">
        <div className="grid lg:grid-cols-2 gap-12 items-center">

          {/* Left — copy */}
          <div className="flex flex-col items-start animate-fade-in">
            {/* Badge */}
            <div className="mb-6 inline-flex items-center gap-2 px-3 py-1.5 rounded-full border border-primary/30 bg-primary/5 text-xs font-medium text-primary tracking-wide">
              <span className="w-1.5 h-1.5 rounded-full bg-primary animate-pulse" />
              High-precision energy monitoring for Linux
            </div>

            <h1 className="text-4xl sm:text-5xl lg:text-6xl font-bold text-foreground leading-[1.1] mb-6 tracking-tight">
              Real energy.{' '}
              <span className="text-primary">Real hardware.</span>{' '}
              Real science.
            </h1>

            <p className="text-lg text-muted-foreground leading-relaxed mb-8 max-w-xl">
              EcoCode reads hardware power sensors directly — the same interfaces used by power-management firmware.
              Get accurate per-process CPU, GPU, and RAM energy measurements, a live collaborative dashboard, and carbon
              footprint estimation across 50+ countries. No TDP guessing.
            </p>

            {/* CTAs */}
            <div className="flex flex-wrap items-center gap-3 mb-10">
              <Link
                href="/register"
                className="flex items-center gap-2 px-5 py-2.5 bg-primary text-primary-foreground font-medium rounded-lg hover:brightness-110 transition-all shadow-lg shadow-primary/25 group"
              >
                Get Started Free
                <ArrowRight className="w-4 h-4 group-hover:translate-x-0.5 transition-transform" />
              </Link>
              <Link
                href="/login"
                className="flex items-center gap-2 px-5 py-2.5 border border-border text-foreground font-medium rounded-lg hover:bg-muted/50 transition-all"
              >
                Sign In
              </Link>
              <a
                href="#"
                aria-label="View on GitHub"
                className="flex items-center gap-2 px-5 py-2.5 border border-border text-muted-foreground font-medium rounded-lg hover:bg-muted/50 hover:text-foreground transition-all"
              >
                <Github className="w-4 h-4" />
                GitHub
              </a>
            </div>

            {/* Stat pills */}
            <div className="grid grid-cols-2 sm:grid-cols-4 gap-3 w-full max-w-xl">
              {[
                { count: c0, suffix: STATS[0].suffix, label: STATS[0].label },
                { count: c1, suffix: STATS[1].suffix, label: STATS[1].label },
                { count: c2, suffix: STATS[2].suffix, label: STATS[2].label },
                { count: null, suffix: STATS[3].suffix, label: STATS[3].label, raw: `<${STATS[3].value}` },
              ].map(({ count, suffix, label, raw }) => (
                <div key={label} className="flex flex-col items-center text-center px-3 py-3 rounded-xl bg-card border border-border">
                  <span className="text-xl font-bold text-primary tabular-nums">
                    {raw ?? `${count}${suffix}`}
                  </span>
                  <span className="text-xs text-muted-foreground mt-0.5 leading-tight">{label}</span>
                </div>
              ))}
            </div>
          </div>

          {/* Right — code block */}
          <div className="relative animate-fade-in" style={{ animationDelay: '150ms' }}>
            {/* Terminal window chrome */}
            <div className="rounded-xl border border-border bg-neutral-950 shadow-2xl shadow-black/60 overflow-hidden">
              <div className="flex items-center gap-1.5 px-4 py-3 border-b border-border/60 bg-neutral-900/60">
                <span className="w-3 h-3 rounded-full bg-red-500/70" />
                <span className="w-3 h-3 rounded-full bg-yellow-500/70" />
                <span className="w-3 h-3 rounded-full bg-green-500/70" />
                <span className="ml-3 text-xs text-muted-foreground font-mono">train.py — ecocode</span>
              </div>
              <pre className="p-5 font-mono text-sm leading-relaxed overflow-x-auto" aria-label="EcoCode usage example">
                {CODE_LINES.map((line, li) => (
                  <div key={li} className="flex">
                    <span className="select-none mr-5 text-muted-foreground/40 w-5 text-right shrink-0 text-xs leading-relaxed pt-px">
                      {li + 1}
                    </span>
                    <span>
                      {line.tokens.length === 0 ? '\u00a0' : line.tokens.map((tok, ti) => (
                        <span key={ti} className={TOKEN_COLOURS[tok.type] || 'text-foreground/90'}>
                          {tok.text}
                        </span>
                      ))}
                    </span>
                  </div>
                ))}
              </pre>
              {/* Animated cursor */}
              <div className="px-5 pb-4 flex items-center gap-2">
                <span className="font-mono text-sm text-primary/60">$</span>
                <span className="font-mono text-sm text-muted-foreground/50">python train.py</span>
                <span className="w-2 h-4 bg-primary/50 animate-pulse rounded-sm" />
              </div>
            </div>

            {/* Floating badge */}
            <div className="absolute -bottom-4 -left-4 px-3 py-2 rounded-lg bg-card border border-border shadow-lg text-xs font-mono">
              <span className="text-chart-3">✓</span>
              <span className="text-muted-foreground ml-1.5">Run published to dashboard</span>
            </div>
          </div>
        </div>

        {/* Scroll indicator */}
        <div className="mt-16 flex flex-col items-center gap-2 text-muted-foreground/50 animate-bounce">
          <ChevronDown className="w-5 h-5" />
        </div>
      </div>
    </section>
  )
}
