"""
Heavy vLLM Integration Test
- Tracker:    InferencePhaseTracker around real GPT-2 autoregressive generation
              (transformers, KV cache), prompts 64→1024 tokens
- Middleware: EcoCodeVLLMMiddleware around a real vllm.AsyncLLMEngine (CUDA only)
"""
import asyncio
import importlib.util
import time

import pytest

from ecocode.integrations.vllm import EcoCodeVLLMMiddleware, InferencePhaseTracker

torch = pytest.importorskip("torch")
pytest.importorskip("transformers")

from transformers import AutoModelForCausalLM, AutoTokenizer  # noqa: E402

BASE_TEXTS = [
    "The transformer architecture, introduced in 'Attention Is All You Need', fundamentally changed how "
    "sequence-to-sequence tasks are approached. By replacing recurrence with multi-head self-attention, "
    "transformers can attend to all positions simultaneously and scale efficiently. ",
    "Large language models pre-trained on web-scale corpora exhibit emergent capabilities not seen in "
    "smaller models. These capabilities include few-shot learning, chain-of-thought reasoning, and tool "
    "use, making LLMs broadly applicable across many domains. ",
    "The key challenge in efficient LLM inference is managing the KV cache, which stores key and value "
    "tensors from prior decode steps to avoid redundant computation. Memory-efficient attention and "
    "continuous batching are critical techniques for high-throughput serving. ",
]


def test_heavy_vllm_tracker(device, scaled, heavy_run, marker_tap):
    n_requests = scaled(200, 6)
    decode_tokens = scaled(256, 16)
    expected_phases, expected_seq, expected_tokens = ["model_load"], [], []
    with heavy_run("vllm-gpt2-generation") as eco_run:
        tracker = InferencePhaseTracker(eco_run)
        tracker.on_model_load()
        tokenizer = AutoTokenizer.from_pretrained("gpt2")
        model = AutoModelForCausalLM.from_pretrained(
            "gpt2", torch_dtype=torch.float16 if device == "cuda" else torch.float32
        ).to(device).eval()

        for i in range(n_requests):
            prompt = (BASE_TEXTS[i % len(BASE_TEXTS)] * max(1, (i % 16) + 1)).strip()
            ids = tokenizer(prompt, return_tensors="pt", truncation=True, max_length=1024)["input_ids"].to(device)
            prompt_len = ids.shape[1]
            tracker.on_prefill_begin(prompt_tokens=prompt_len)
            t0 = time.perf_counter()
            with torch.no_grad():
                out = model(ids, use_cache=True)
                past, cur = out.past_key_values, out.logits[:, -1:].argmax(-1)
            tracker.on_decode_begin()
            with torch.no_grad():
                for _ in range(decode_tokens - 1):
                    out = model(cur, past_key_values=past, use_cache=True)
                    past, cur = out.past_key_values, out.logits[:, -1:].argmax(-1)
            total = prompt_len + decode_tokens
            tracker.on_generation_end(total_tokens=total)
            expected_phases += ["prefill", "decode"]
            expected_seq.append(prompt_len)
            expected_tokens.append(total)
            if (i + 1) % 20 == 0:
                print(f"  [{i + 1}/{n_requests}] prompt={prompt_len} tok, {(time.perf_counter() - t0) * 1000:.0f} ms")

    assert marker_tap.phases() == expected_phases
    assert marker_tap.values("seq_len") == expected_seq
    assert marker_tap.values("tokens") == expected_tokens
    if not heavy_run.online:
        assert heavy_run.last()["phase"] == "decode"
        assert heavy_run.last()["tokens_processed"] == expected_tokens[-1]


@pytest.mark.cuda
def test_heavy_vllm_middleware(device, scaled, heavy_run, marker_tap):
    if importlib.util.find_spec("vllm") is None:
        pytest.skip("vllm not installed")
    if device != "cuda":
        pytest.skip("vLLM engine needs CUDA")
    from vllm import AsyncEngineArgs, AsyncLLMEngine, SamplingParams

    engine = AsyncLLMEngine.from_engine_args(
        AsyncEngineArgs(model="gpt2", enforce_eager=True, gpu_memory_utilization=0.4)
    )
    n_requests, max_tokens = scaled(50, 2), 64
    params = SamplingParams(max_tokens=max_tokens, temperature=0.0)

    async def collect(gen):
        return [o async for o in gen]

    finals = []
    with heavy_run("vllm-engine-gpt2") as eco_run:
        wrapped = EcoCodeVLLMMiddleware(engine, eco_run)
        for i in range(n_requests):
            prompt = BASE_TEXTS[i % len(BASE_TEXTS)].strip()
            outputs = asyncio.run(collect(wrapped.generate(prompt, params, request_id=f"r{i}")))
            finals.append((len(prompt.split()) * 4 // 3, len(outputs[-1].outputs[0].token_ids)))

    assert marker_tap.phases() == ["prefill", "decode"] * n_requests
    assert marker_tap.values("seq_len") == [p for p, _ in finals]
    tokens = marker_tap.values("tokens")
    assert tokens[-1] == finals[-1][0] + finals[-1][1]
