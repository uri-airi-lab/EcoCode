'use client'

import { useState } from 'react'

interface Integration {
  id: string
  label: string
  import: string
  snippet: string[]
}

const INTEGRATIONS: Integration[] = [
  {
    id: 'hf',
    label: 'HuggingFace',
    import: 'from ecocode.integrations.huggingface import EcoCodeCallback',
    snippet: [
      'with ecocode.run("sft-llama3", api_key="eco_...") as eco_run:',
      '    trainer = Trainer(',
      '        model=model,',
      '        args=TrainingArguments(...),',
      '        callbacks=[EcoCodeCallback(eco_run)],  # ← one line',
      '    )',
      '    trainer.train()',
    ],
  },
  {
    id: 'trl',
    label: 'TRL',
    import: 'from ecocode.integrations.trl import EcoCodeSFTCallback, EcoCodeGRPOCallback',
    snippet: [
      '# SFT / DPO / GRPO — each has a dedicated callback',
      '# that maps the unique energy profile of each paradigm',
      'with ecocode.run("grpo-qwen", api_key="eco_...") as eco_run:',
      '    trainer = GRPOTrainer(',
      '        model=model,',
      '        callbacks=[EcoCodeGRPOCallback(eco_run)],',
      '    )',
      '    trainer.train()',
    ],
  },
  {
    id: 'lightning',
    label: 'PyTorch Lightning',
    import: 'from ecocode.integrations.lightning import EcoCodeCallback',
    snippet: [
      'with ecocode.run("pretrain", api_key="eco_...") as eco_run:',
      '    trainer = pl.Trainer(',
      '        callbacks=[EcoCodeCallback(eco_run)],',
      '    )',
      '    trainer.fit(model, datamodule=dm)',
    ],
  },
  {
    id: 'deepspeed',
    label: 'DeepSpeed',
    import: 'from ecocode.integrations.deepspeed import EcoCodeDeepSpeedCallback',
    snippet: [
      'with ecocode.run("pretrain-70b", api_key="eco_...") as eco_run:',
      '    ds_cb = EcoCodeDeepSpeedCallback(eco_run, zero_stage=3)',
      '    for step, batch in enumerate(loader):',
      '        ds_cb.on_step_begin(step)',
      '        engine.backward(outputs.loss)',
      '        ds_cb.on_optimizer_step(step, loss)',
    ],
  },
  {
    id: 'vllm',
    label: 'vLLM',
    import: 'from ecocode.integrations.vllm import EcoCodeVLLMMiddleware',
    snippet: [
      'with ecocode.run("llama3-serving", api_key="eco_...") as eco_run:',
      '    engine = EcoCodeVLLMMiddleware(',
      '        AsyncLLMEngine.from_engine_args(args), eco_run',
      '    )',
      '    # Marks prefill / decode phases automatically',
      '    async for out in engine.generate(prompt, params, req_id):',
      '        yield out',
    ],
  },
  {
    id: 'keras',
    label: 'Keras',
    import: 'from ecocode.integrations.keras import EcoCodeCallback',
    snippet: [
      'with ecocode.run("keras-train", api_key="eco_...") as eco_run:',
      '    model.fit(',
      '        x, y,',
      '        callbacks=[EcoCodeCallback(eco_run)],',
      '    )',
      '    # Custom Keras metrics forwarded automatically',
    ],
  },
  {
    id: 'onnx',
    label: 'ONNX Runtime',
    import: 'from ecocode.integrations.onnxruntime import EcoCodeORTWrapper',
    snippet: [
      'session = ort.InferenceSession("model.onnx")',
      'with ecocode.run("ort-inference", api_key="eco_...") as eco_run:',
      '    eco_session = EcoCodeORTWrapper(session, eco_run)',
      '    result = eco_session.run(None, {"input": data})',
      '    # Logs batch_size, latency_ms, step count per call',
    ],
  },
]

export function IntegrationsShowcase() {
  const [active, setActive] = useState('hf')
  const current = INTEGRATIONS.find((i) => i.id === active)!

  return (
    <section id="integrations" className="relative py-24 px-4 overflow-hidden">
      <div className="absolute inset-0 landing-section-glow-right pointer-events-none" />

      <div className="relative z-10 max-w-5xl mx-auto">
        {/* Header */}
        <div className="text-center mb-12">
          <p className="text-sm font-medium text-primary tracking-widest uppercase mb-3">
            Framework integrations
          </p>
          <h2 className="text-3xl sm:text-4xl font-bold text-foreground mb-4">
            Works with your stack
          </h2>
          <p className="text-muted-foreground max-w-2xl mx-auto text-lg leading-relaxed">
            Dedicated callbacks for every major ML framework — one extra line in your existing code.
          </p>
        </div>

        {/* Tab strip */}
        <div className="flex flex-wrap gap-2 mb-6 justify-center" role="tablist" aria-label="Framework integrations">
          {INTEGRATIONS.map(({ id, label }) => (
            <button
              key={id}
              role="tab"
              aria-selected={active === id}
              aria-controls={`panel-${id}`}
              onClick={() => setActive(id)}
              className={`px-4 py-1.5 rounded-full text-sm font-medium transition-all duration-150 ${
                active === id
                  ? 'bg-primary text-primary-foreground shadow-md shadow-primary/20'
                  : 'border border-border text-muted-foreground hover:text-foreground hover:bg-muted/50'
              }`}
            >
              {label}
            </button>
          ))}
        </div>

        {/* Code panel */}
        <div
          id={`panel-${current.id}`}
          role="tabpanel"
          aria-label={`${current.label} integration example`}
          className="rounded-xl border border-border bg-neutral-950 overflow-hidden"
        >
          {/* Chrome */}
          <div className="flex items-center gap-1.5 px-4 py-3 border-b border-border/60 bg-neutral-900/60">
            <span className="w-3 h-3 rounded-full bg-red-500/70" />
            <span className="w-3 h-3 rounded-full bg-yellow-500/70" />
            <span className="w-3 h-3 rounded-full bg-green-500/70" />
            <span className="ml-3 text-xs text-muted-foreground font-mono">{current.label} integration</span>
          </div>

          <pre className="p-5 font-mono text-sm leading-relaxed overflow-x-auto" aria-label={`${current.label} code example`}>
            {/* Import line */}
            <div className="mb-3">
              <span className="text-chart-4">import</span>
              <span className="text-foreground/80"> ecocode</span>
              {'\n'}
              <span className="text-muted-foreground/60">{current.import}</span>
            </div>
            {/* Snippet lines */}
            {current.snippet.map((line, i) => (
              <div key={i}>
                <span className={line.startsWith('#') ? 'text-muted-foreground/60 italic' : 'text-foreground/90'}>
                  {line}
                </span>
                {'\n'}
              </div>
            ))}
          </pre>
        </div>
      </div>
    </section>
  )
}
