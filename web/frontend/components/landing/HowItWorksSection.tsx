'use client'

import { useState, useEffect } from 'react'
import { KeyRound, Play, BarChart3, ChevronRight, Copy, Check, Terminal, Cpu } from 'lucide-react'

export function HowItWorksSection() {
  const [activeStep, setActiveStep] = useState(0)
  const [apiKey, setApiKey] = useState('eco_************************')
  const [isGenerated, setIsGenerated] = useState(false)
  const [copiedKey, setCopiedKey] = useState(false)
  const [sdkTab, setSdkTab] = useState<'cli' | 'sdk'>('cli')

  // Real-time chart simulator for Step 3
  const [chartData, setChartData] = useState<number[]>([12, 19, 15, 24, 28, 33])
  useEffect(() => {
    if (activeStep !== 2) return
    const interval = setInterval(() => {
      setChartData((prev) => {
        const next = [...prev.slice(1)]
        const last = prev[prev.length - 1]
        // Add random walk power value in Watts
        const delta = Math.floor((Math.random() - 0.45) * 8)
        const val = Math.max(10, Math.min(60, last + delta))
        return [...next, val]
      });
    }, 1000)
    return () => clearInterval(interval)
  }, [activeStep])

  const handleGenerateKey = () => {
    setIsGenerated(true)
    setApiKey('eco_live_8a9f2c3b0d1e5f8a92')
  }

  const handleCopyKey = () => {
    navigator.clipboard.writeText(apiKey)
    setCopiedKey(true)
    setTimeout(() => setCopiedKey(false), 2000)
  }

  const steps = [
    {
      title: 'Get Workspace Key',
      description: 'Create an account on the dashboard, navigate to Settings, and generate a workspace API key to authenticate your runs.',
      Icon: KeyRound,
    },
    {
      title: 'Monitor Your Code',
      description: 'Use the CLI binary wrapper to monitor any Linux program out-of-the-box, or integrate the Python SDK into your ML scripts.',
      Icon: Play,
    },
    {
      title: 'Stream Live Charts',
      description: 'Open your web dashboard and see high-precision metrics (CPU, multiple GPUs, RAM power draws) streaming live per-second.',
      Icon: BarChart3,
    },
  ]

  return (
    <section id="how-it-works" className="relative py-24 px-4 overflow-hidden border-t border-border/50">
      <div className="absolute inset-0 landing-section-glow-right pointer-events-none" />

      <div className="relative z-10 max-w-6xl mx-auto">
        {/* Section Header */}
        <div className="text-center mb-16">
          <p className="text-sm font-medium text-primary tracking-widest uppercase mb-3">
            Simple Onboarding
          </p>
          <h2 className="text-3xl sm:text-4xl font-bold text-foreground mb-4">
            How EcoCode Works
          </h2>
          <p className="text-muted-foreground max-w-2xl mx-auto text-lg leading-relaxed">
            Get from zero to real-time software carbon tracking in three intuitive steps.
          </p>
        </div>

        {/* Step Navigation Cards */}
        <div className="grid md:grid-cols-3 gap-6 mb-12">
          {steps.map((step, index) => {
            const StepIcon = step.Icon
            const isActive = activeStep === index
            return (
              <button
                key={index}
                onClick={() => setActiveStep(index)}
                className={`landing-card text-left flex flex-col items-start gap-4 transition-all duration-300 relative ${
                  isActive
                    ? 'border-primary/50 bg-primary/[0.02] shadow-lg shadow-primary/5 ring-1 ring-primary/20'
                    : 'opacity-70 hover:opacity-100'
                }`}
              >
                {/* Step number badge */}
                <div className={`absolute top-4 right-4 flex items-center justify-center w-6 h-6 rounded-full text-xs font-bold font-mono border ${
                  isActive
                    ? 'bg-primary/20 border-primary text-primary'
                    : 'bg-muted border-border text-muted-foreground'
                }`}>
                  {index + 1}
                </div>

                <div className={`w-10 h-10 rounded-lg flex items-center justify-center border transition-colors ${
                  isActive
                    ? 'bg-primary/20 border-primary/30 text-primary'
                    : 'bg-muted border-border text-muted-foreground'
                }`}>
                  <StepIcon className="w-5 h-5" />
                </div>

                <div>
                  <h3 className="font-semibold text-foreground text-base mb-1.5">{step.title}</h3>
                  <p className="text-xs text-muted-foreground leading-relaxed">{step.description}</p>
                </div>
              </button>
            )
          })}
        </div>

        {/* Dynamic Display Panel for Active Step */}
        <div className="rounded-xl border border-border bg-neutral-950/80 backdrop-blur-sm overflow-hidden shadow-2xl animate-fade-in">
          {/* Header */}
          <div className="flex items-center justify-between px-6 py-4 border-b border-border bg-neutral-950">
            <div className="flex items-center gap-1.5">
              <span className="w-3 h-3 rounded-full bg-red-500/70" />
              <span className="w-3 h-3 rounded-full bg-yellow-500/70" />
              <span className="w-3 h-3 rounded-full bg-green-500/70" />
              <span className="ml-3 text-xs text-muted-foreground font-mono">
                {activeStep === 0 && 'developer-settings // api-keys'}
                {activeStep === 1 && (sdkTab === 'cli' ? 'shell -- run-cli' : 'train.py -- integrate-sdk')}
                {activeStep === 2 && 'run-dashboard // sse-stream-live'}
              </span>
            </div>

            {/* Sub-navigation for Step 2 */}
            {activeStep === 1 && (
              <div className="flex bg-neutral-900 border border-border rounded-lg p-0.5" role="tablist" aria-label="Step 2 integrations">
                <button
                  role="tab"
                  aria-selected={sdkTab === 'cli'}
                  onClick={() => setSdkTab('cli')}
                  className={`flex items-center gap-1.5 px-3 py-1 rounded-md text-xs font-mono transition-all ${
                    sdkTab === 'cli' ? 'bg-primary/10 text-primary font-bold border border-primary/20' : 'text-muted-foreground hover:text-foreground'
                  }`}
                >
                  <Terminal className="w-3.5 h-3.5" />
                  CLI Mode
                </button>
                <button
                  role="tab"
                  aria-selected={sdkTab === 'sdk'}
                  onClick={() => setSdkTab('sdk')}
                  className={`flex items-center gap-1.5 px-3 py-1 rounded-md text-xs font-mono transition-all ${
                    sdkTab === 'sdk' ? 'bg-primary/10 text-primary font-bold border border-primary/20' : 'text-muted-foreground hover:text-foreground'
                  }`}
                >
                  <Cpu className="w-3.5 h-3.5" />
                  Python SDK
                </button>
              </div>
            )}
          </div>

          {/* Body Content */}
          <div className="p-6 md:p-8 min-h-[260px] flex items-center justify-center">
            {activeStep === 0 && (
              <div className="w-full max-w-xl flex flex-col gap-6 items-center text-center">
                <div className="w-full bg-neutral-900 border border-border rounded-xl p-5 flex items-center justify-between gap-4 font-mono text-sm overflow-hidden">
                  <div className="flex flex-col items-start gap-1 w-full overflow-hidden text-left">
                    <span className="text-[10px] uppercase tracking-wider text-muted-foreground">API Token Prefix</span>
                    <span className="text-foreground truncate w-full">{apiKey}</span>
                  </div>
                  {isGenerated ? (
                    <button
                      onClick={handleCopyKey}
                      className="px-4 py-2 border border-border hover:border-primary/50 text-foreground hover:text-primary rounded-lg flex items-center gap-2 transition-all font-sans font-medium"
                    >
                      {copiedKey ? (
                        <>
                          <Check className="w-4 h-4 text-primary animate-scale-in" />
                          Copied
                        </>
                      ) : (
                        <>
                          <Copy className="w-4 h-4" />
                          Copy Key
                        </>
                      )}
                    </button>
                  ) : (
                    <button
                      onClick={handleGenerateKey}
                      className="px-4 py-2 bg-primary text-primary-foreground font-sans font-medium rounded-lg hover:brightness-110 transition-all whitespace-nowrap shadow-md shadow-primary/10"
                    >
                      Generate Key
                    </button>
                  )}
                </div>
                <p className="text-xs text-muted-foreground leading-relaxed max-w-md">
                  Keep this key secret. You can set it globally in your terminal with `ecocode config set-api-key &lt;key&gt;` or assign it via environment variable `ECOCODE_API_KEY`.
                </p>
              </div>
            )}

            {activeStep === 1 && (
              <div className="w-full font-mono text-sm leading-relaxed">
                {sdkTab === 'cli' ? (
                  <div className="flex flex-col gap-4">
                    <div className="flex flex-col gap-1.5">
                      <span className="text-muted-foreground text-xs font-sans">// Step 1: Install the Linux agent wrapper</span>
                      <div className="bg-neutral-900 border border-border rounded-lg p-3 flex justify-between items-center">
                        <span className="text-primary">curl -sSf https://ecocode.dev/install.sh | sh</span>
                      </div>
                    </div>
                    <div className="flex flex-col gap-1.5">
                      <span className="text-muted-foreground text-xs font-sans">// Step 2: Prefix any execution shell command</span>
                      <div className="bg-neutral-900 border border-border rounded-lg p-3 flex justify-between items-center overflow-x-auto">
                        <span className="text-foreground">ecocode run -o online --project "image-classification" -- python train.py</span>
                      </div>
                    </div>
                  </div>
                ) : (
                  <div className="flex flex-col gap-4">
                    <div className="flex flex-col gap-1.5">
                      <span className="text-muted-foreground text-xs font-sans">// Step 1: Install via pip package manager</span>
                      <div className="bg-neutral-900 border border-border rounded-lg p-3 flex justify-between items-center">
                        <span className="text-primary">pip install ecocode</span>
                      </div>
                    </div>
                    <div className="flex flex-col gap-1.5">
                      <span className="text-muted-foreground text-xs font-sans">// Step 2: Wrap training blocks in context manager</span>
                      <div className="bg-neutral-900 border border-border rounded-lg p-4 overflow-x-auto text-foreground/90">
                        <span className="text-chart-4">import</span> ecocode<br />
                        <br />
                        <span className="text-chart-4">with</span> ecocode.run(<span className="text-chart-3">"llama3-finetune"</span>) <span className="text-chart-4">as</span> run:<br />
                        &nbsp;&nbsp;&nbsp;&nbsp;model.fit(dataset)<br />
                        &nbsp;&nbsp;&nbsp;&nbsp;run.log_loss(loss.item())
                      </div>
                    </div>
                  </div>
                )}
              </div>
            )}

            {activeStep === 2 && (
              <div className="w-full flex flex-col md:flex-row gap-8 items-center">
                {/* Simulated Chart Rendering */}
                <div className="w-full md:w-2/3 bg-neutral-900/60 border border-border rounded-xl p-5 flex flex-col gap-4">
                  <div className="flex justify-between items-center">
                    <div className="flex flex-col">
                      <span className="text-xs font-bold text-foreground">Active Run Power (Watts)</span>
                      <span className="text-[10px] text-muted-foreground">Streaming live via Server-Sent Events (SSE)</span>
                    </div>
                    <div className="flex items-center gap-1 text-[10px] text-primary border border-primary/30 bg-primary/5 px-2 py-0.5 rounded-full font-mono font-bold animate-pulse">
                      <span className="w-1.5 h-1.5 rounded-full bg-primary" />
                      LIVE
                    </div>
                  </div>

                  {/* Horizontal Bar Visualizer */}
                  <div className="flex items-end justify-between gap-2.5 h-32 pt-2 px-1">
                    {chartData.map((val, idx) => (
                      <div key={idx} className="flex-1 flex flex-col items-center gap-1.5">
                        <div
                          className="w-full rounded-t bg-gradient-to-t from-primary/40 to-primary transition-all duration-700 ease-out relative group"
                          style={{ height: `${(val / 60) * 100}px` }}
                        >
                          {/* Power Watts Tooltip */}
                          <div className="absolute -top-7 left-1/2 -translate-x-1/2 bg-neutral-800 text-white font-mono text-[9px] px-1 py-0.5 rounded opacity-0 group-hover:opacity-100 transition-opacity">
                            {val}W
                          </div>
                        </div>
                        <span className="text-[9px] font-mono text-muted-foreground/60">{idx}s ago</span>
                      </div>
                    ))}
                  </div>
                </div>

                {/* Status metrics side panel */}
                <div className="w-full md:w-1/3 flex flex-col gap-4">
                  <div className="flex items-center justify-between border-b border-border/50 pb-2">
                    <span className="text-xs text-muted-foreground">Monitored PID</span>
                    <span className="text-xs font-mono font-bold text-foreground">30842</span>
                  </div>
                  <div className="flex items-center justify-between border-b border-border/50 pb-2">
                    <span className="text-xs text-muted-foreground">Elapsed Duration</span>
                    <span className="text-xs font-mono font-bold text-foreground">00:04:12</span>
                  </div>
                  <div className="flex items-center justify-between border-b border-border/50 pb-2">
                    <span className="text-xs text-muted-foreground">Total Energy</span>
                    <span className="text-xs font-mono font-bold text-primary">12.82 Wh</span>
                  </div>
                  <div className="flex items-center justify-between">
                    <span className="text-xs text-muted-foreground">CO₂ Equivalent</span>
                    <span className="text-xs font-mono font-bold text-chart-2">5.71 gCO₂</span>
                  </div>
                </div>
              </div>
            )}
          </div>
        </div>

        {/* Navigation buttons */}
        <div className="mt-6 flex justify-between items-center text-sm">
          <button
            onClick={() => setActiveStep((prev) => Math.max(0, prev - 1))}
            disabled={activeStep === 0}
            className="px-4 py-2 border border-border hover:border-foreground/20 text-muted-foreground hover:text-foreground rounded-lg disabled:opacity-30 disabled:pointer-events-none transition-all"
          >
            Previous Step
          </button>
          <button
            onClick={() => setActiveStep((prev) => Math.min(2, prev + 1))}
            disabled={activeStep === 2}
            className="px-4 py-2 bg-neutral-900 border border-border hover:border-primary/50 text-foreground hover:text-primary rounded-lg flex items-center gap-1 disabled:opacity-30 disabled:pointer-events-none transition-all"
          >
            Next Step
            <ChevronRight className="w-4 h-4" />
          </button>
        </div>
      </div>
    </section>
  )
}
