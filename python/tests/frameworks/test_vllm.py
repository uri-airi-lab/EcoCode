"""vLLM integration: InferencePhaseTracker around a real transformers generate
loop, and EcoCodeVLLMMiddleware around a real vllm.AsyncLLMEngine (CUDA only)."""
import asyncio
import importlib.util

import pytest

from ecocode.integrations.vllm import EcoCodeVLLMMiddleware, InferencePhaseTracker

MAX_NEW = 8


def test_inference_phase_tracker_real_generate(torch, tokenizer, tiny_gpt2, marker_tap, json_run, burn):
    prompt_ids = tokenizer("the model is training on green compute", return_tensors="pt").input_ids
    prompt_len = prompt_ids.shape[1]
    with json_run("vllm-tracker") as run:
        tracker = InferencePhaseTracker(run)
        tracker.on_model_load()
        model = tiny_gpt2.eval()
        tracker.on_prefill_begin(prompt_tokens=prompt_len)
        with torch.no_grad():
            # Prefill: one forward pass over the prompt with a KV cache.
            out = model(prompt_ids, use_cache=True)
            past = out.past_key_values
            next_id = out.logits[:, -1].argmax(-1, keepdim=True)
            tracker.on_decode_begin()
            generated = [next_id]
            for _ in range(MAX_NEW - 1):
                out = model(next_id, past_key_values=past, use_cache=True)
                past = out.past_key_values
                next_id = out.logits[:, -1].argmax(-1, keepdim=True)
                generated.append(next_id)
        total = prompt_len + len(generated)
        tracker.on_generation_end(total_tokens=total)
        burn(0.3)

    assert marker_tap.phases() == ["model_load", "prefill", "decode"]
    assert marker_tap.values("seq_len") == [prompt_len]
    assert marker_tap.values("tokens") == [prompt_len + MAX_NEW]
    assert json_run.last()["tokens_processed"] == prompt_len + MAX_NEW
    assert json_run.last()["sequence_length"] == prompt_len
    assert json_run.last()["phase"] == "decode"


@pytest.mark.cuda
def test_vllm_middleware_real_async_engine(marker_tap, json_run):
    if importlib.util.find_spec("vllm") is None:
        pytest.skip("vllm not installed")
    torch = pytest.importorskip("torch")
    if not torch.cuda.is_available():
        pytest.skip("vLLM engine needs CUDA")
    from vllm import AsyncEngineArgs, AsyncLLMEngine, SamplingParams

    engine = AsyncLLMEngine.from_engine_args(
        AsyncEngineArgs(model="facebook/opt-125m", enforce_eager=True, gpu_memory_utilization=0.3)
    )
    prompt = "The transformer architecture"
    params = SamplingParams(max_tokens=MAX_NEW, temperature=0.0)

    async def collect(gen):
        outs = []
        async for o in gen:
            outs.append(o)
        return outs

    with json_run("vllm-engine") as run:
        wrapped = EcoCodeVLLMMiddleware(engine, run)
        outputs = asyncio.run(collect(wrapped.generate(prompt, params, request_id="r1")))

    final = outputs[-1]
    n_generated = len(final.outputs[0].token_ids)
    assert n_generated == MAX_NEW
    prompt_len = len(prompt.split()) * 4 // 3  # the middleware's estimate without token ids
    assert marker_tap.phases() == ["prefill", "decode"]
    assert marker_tap.values("seq_len") == [prompt_len]
    tokens = marker_tap.values("tokens")
    assert tokens == sorted(tokens) and tokens[-1] == prompt_len + n_generated
    assert json_run.last()["tokens_processed"] == prompt_len + n_generated
