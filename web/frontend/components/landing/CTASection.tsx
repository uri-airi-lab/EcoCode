import Link from 'next/link'
import { ArrowRight } from 'lucide-react'

export function CTASection() {
  return (
    <section id="get-started" className="relative py-24 px-4 overflow-hidden">
      {/* Full-width glow */}
      <div className="absolute inset-0 landing-hero-glow pointer-events-none opacity-60" />

      <div className="relative z-10 max-w-3xl mx-auto text-center">
        <div className="inline-flex items-center gap-2 px-3 py-1 rounded-full border border-primary/30 bg-primary/5 text-xs font-medium text-primary mb-6">
          <span className="w-1.5 h-1.5 rounded-full bg-primary animate-pulse" />
          Free to get started
        </div>

        <h2 className="text-4xl sm:text-5xl font-bold text-foreground mb-5 leading-tight">
          Start measuring your impact today
        </h2>

        <p className="text-lg text-muted-foreground leading-relaxed mb-10 max-w-xl mx-auto">
          Three lines of code. No infrastructure to manage.
          Real hardware readings from day one.
        </p>

        {/* Code teaser */}
        <div className="mb-10 rounded-xl border border-border bg-neutral-950 text-left overflow-hidden shadow-2xl shadow-black/50 max-w-lg mx-auto">
          <div className="flex items-center gap-1.5 px-4 py-2.5 border-b border-border/60 bg-neutral-900/60">
            <span className="w-2.5 h-2.5 rounded-full bg-red-500/70" />
            <span className="w-2.5 h-2.5 rounded-full bg-yellow-500/70" />
            <span className="w-2.5 h-2.5 rounded-full bg-green-500/70" />
            <span className="ml-3 text-xs text-muted-foreground font-mono">train.py</span>
          </div>
          <pre className="p-4 font-mono text-sm leading-relaxed">
            <span className="text-chart-4">import</span>
            <span className="text-foreground/90"> ecocode{'\n'}</span>
            <span className="text-foreground/90">{'\n'}</span>
            <span className="text-foreground/50"># That&apos;s it. Wrap your existing training loop:{'\n'}</span>
            <span className="text-chart-4">with </span>
            <span className="text-foreground/90">ecocode.</span>
            <span className="text-chart-2">run</span>
            <span className="text-foreground/90">(</span>
            <span className="text-chart-3">&quot;my-experiment&quot;</span>
            <span className="text-foreground/90">, api_key=</span>
            <span className="text-chart-3">&quot;eco_...&quot;</span>
            <span className="text-foreground/90">) as run:{'\n'}</span>
            <span className="text-foreground/90">    ...</span>
          </pre>
        </div>

        {/* CTA buttons */}
        <div className="flex flex-wrap items-center justify-center gap-4">
          <Link
            href="/register"
            className="flex items-center gap-2 px-6 py-3 bg-primary text-primary-foreground font-semibold rounded-lg hover:brightness-110 transition-all shadow-xl shadow-primary/30 group text-base"
          >
            Create an Account
            <ArrowRight className="w-4 h-4 group-hover:translate-x-0.5 transition-transform" />
          </Link>
          <Link
            href="/login"
            target="_blank"
            rel="noopener noreferrer"
            className="flex items-center gap-2 px-6 py-3 border border-border text-foreground font-semibold rounded-lg hover:bg-muted/50 transition-all text-base"
          >
            Open Dashboard
          </Link>
          <Link
            href="/login"
            className="text-sm text-muted-foreground hover:text-foreground transition-colors"
          >
            Already have an account? Sign in
          </Link>
        </div>
      </div>
    </section>
  )
}
