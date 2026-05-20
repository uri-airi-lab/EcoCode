"""
Heavy vLLM Tracker Integration Test
- Model:    GPT-2 XL (1.5B) doing real autoregressive generation on GPU
- Tracker:  InferencePhaseTracker — model_load, prefill, decode, generation_end
- Scenario: 200 requests with varying prompt lengths (64→1024 tokens),
            256 decode tokens each, measuring prefill vs decode energy split
"""
import os, time, math
os.environ.setdefault("ECOCODE_API_URL", "http://localhost:3001")
os.environ["TOKENIZERS_PARALLELISM"] = "false"

import torch
from transformers import AutoTokenizer, AutoModelForCausalLM
import ecocode
from ecocode.integrations.vllm import InferencePhaseTracker

device = "cuda" if torch.cuda.is_available() else "cpu"
print(f"[vLLM Tracker heavy] device={device}")

MODEL = "gpt2-xl"
print(f"[vLLM Tracker heavy] Loading {MODEL}...")

api_key  = os.environ.get("ECOCODE_API_KEY")
base_url = os.environ.get("ECOCODE_API_URL", "http://localhost:3001")

with ecocode.run("vllm-gpt2xl-generation", api_key=api_key, base_url=base_url, interval=1.0) as eco_run:
    tracker = InferencePhaseTracker(eco_run)

    # ── Phase: model_load ─────────────────────────────────────
    tracker.on_model_load()
    t_load    = time.perf_counter()
    tokenizer = AutoTokenizer.from_pretrained(MODEL)
    tokenizer.pad_token = tokenizer.eos_token
    model = AutoModelForCausalLM.from_pretrained(MODEL, torch_dtype=torch.float16).to(device)
    model.eval()
    params   = sum(p.numel() for p in model.parameters()) / 1e6
    load_ms  = (time.perf_counter() - t_load) * 1000
    print(f"[vLLM Tracker heavy] {MODEL} loaded ({params:.1f}M params) in {load_ms:.0f}ms")
    if device == "cuda":
        print(f"  VRAM: {torch.cuda.memory_allocated()/1e9:.2f}GB")

    # Prompt templates — repeated to build varying-length prompts up to ~1024 tokens
    BASE_TEXTS = [
        "The transformer architecture, introduced in 'Attention Is All You Need', "
        "fundamentally changed how sequence-to-sequence tasks are approached. "
        "By replacing recurrence with multi-head self-attention, transformers "
        "can attend to all positions simultaneously and scale efficiently. ",

        "Large language models pre-trained on web-scale corpora exhibit emergent "
        "capabilities not seen in smaller models. These capabilities include "
        "few-shot learning, chain-of-thought reasoning, and tool use, making "
        "LLMs broadly applicable across many domains. ",

        "Reinforcement learning from human feedback aligns language models with "
        "human preferences by training a reward model on pairwise comparisons "
        "and then optimizing the language model policy via proximal policy "
        "optimization to maximize predicted reward. ",

        "The key challenge in efficient LLM inference is managing the KV cache, "
        "which stores key and value tensors from prior decode steps to avoid "
        "redundant computation. Memory-efficient attention and continuous batching "
        "are critical techniques for high-throughput serving. ",

        "Speculative decoding accelerates autoregressive generation by having a "
        "small draft model propose multiple candidate tokens that a larger target "
        "model then verifies in parallel, achieving meaningful speedups when the "
        "draft acceptance rate is high. ",
    ]

    N_REQUESTS   = 200
    DECODE_TOKENS = 256
    results_summary = []

    print(f"\n[vLLM Tracker heavy] Running {N_REQUESTS} requests, {DECODE_TOKENS} decode tokens each...")

    for req_idx in range(N_REQUESTS):
        base    = BASE_TEXTS[req_idx % len(BASE_TEXTS)]
        repeats = max(1, (req_idx % 16) + 1)          # 1→16 repeats → 64→1024 tokens
        prompt_text = (base * repeats).strip()

        enc        = tokenizer(prompt_text, return_tensors="pt", truncation=True, max_length=1024)
        input_ids  = enc["input_ids"].to(device)
        prompt_len = input_ids.shape[1]

        # ── Phase: prefill ───────────────────────────────────
        tracker.on_prefill_begin(prompt_tokens=prompt_len)
        t_prefill = time.perf_counter()

        with torch.no_grad():
            past = model(input_ids, use_cache=True).past_key_values

        prefill_ms = (time.perf_counter() - t_prefill) * 1000

        # ── Phase: decode ─────────────────────────────────────
        tracker.on_decode_begin()
        t_decode      = time.perf_counter()
        generated_ids = []
        current_ids   = input_ids

        with torch.no_grad():
            for _ in range(DECODE_TOKENS):
                out       = model(current_ids[:, -1:], past_key_values=past, use_cache=True)
                past      = out.past_key_values
                next_tok  = out.logits[:, -1, :].argmax(-1, keepdim=True)
                generated_ids.append(next_tok.item())
                current_ids = next_tok

        decode_ms  = (time.perf_counter() - t_decode) * 1000
        total_toks = prompt_len + len(generated_ids)

        tracker.on_generation_end(total_tokens=total_toks)

        results_summary.append({
            "req":            req_idx + 1,
            "prompt_len":     prompt_len,
            "decode_toks":    len(generated_ids),
            "prefill_ms":     prefill_ms,
            "decode_ms":      decode_ms,
            "ms_per_decode":  decode_ms / len(generated_ids),
        })

        if (req_idx + 1) % 20 == 0:
            r = results_summary[-1]
            print(f"  [{r['req']:3d}/{N_REQUESTS}]  prompt={r['prompt_len']:4d}tok  "
                  f"prefill={r['prefill_ms']:7.1f}ms  "
                  f"decode={r['decode_ms']:7.1f}ms ({r['ms_per_decode']:.1f}ms/tok)  "
                  f"total={r['prompt_len']+r['decode_toks']}tok")

    # Summary stats
    avg_prefill  = sum(r["prefill_ms"]     for r in results_summary) / N_REQUESTS
    avg_decode   = sum(r["decode_ms"]      for r in results_summary) / N_REQUESTS
    avg_mpt      = sum(r["ms_per_decode"]  for r in results_summary) / N_REQUESTS
    total_gen    = sum(r["decode_toks"]    for r in results_summary)
    total_time   = avg_prefill + avg_decode
    prefill_pct  = 100 * avg_prefill / total_time
    decode_pct   = 100 * avg_decode  / total_time

    print(f"\n[vLLM Tracker heavy] PASS  run_id={eco_run._run_id}")
    print(f"  requests={N_REQUESTS}  tokens_generated={total_gen}")
    print(f"  avg prefill latency : {avg_prefill:.1f}ms  ({prefill_pct:.1f}% of time)")
    print(f"  avg decode latency  : {avg_decode:.1f}ms  ({decode_pct:.1f}% of time)")
    print(f"  avg decode speed    : {avg_mpt:.2f}ms/token  ({1000/avg_mpt:.1f} tok/s)")
    if device == "cuda":
        print(f"  VRAM peak: {torch.cuda.max_memory_allocated()/1e9:.2f}GB")
