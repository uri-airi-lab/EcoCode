"""
vLLM inference server integration.

Tracks the prefill vs decode energy split, the most important input to
LLM inference cost modelling:

  Prefill: processes all prompt tokens in parallel (compute-bound).
           Energy is roughly proportional to prompt_length^2 (attention).
  Decode:  generates one token per autoregressive step (memory-bandwidth-bound).
           Energy per token is much lower but latency is higher.

Understanding this split lets you:
  - Optimise prompt length to reduce prefill cost
  - Choose batching strategies to amortise decode cost
  - Compare speculative decoding energy vs baseline

Usage (wrapping AsyncLLMEngine):
    import ecocode
    from ecocode.integrations.vllm import EcoCodeVLLMMiddleware

    engine = AsyncLLMEngine.from_engine_args(engine_args)

    with ecocode.run("vllm-serving", project_id=42, api_key="ecocode_...") as eco_run:
        eco_engine = EcoCodeVLLMMiddleware(engine, eco_run)

        async for output in eco_engine.generate(prompt, sampling_params, request_id):
            yield output

Usage (standalone inference loop without vLLM):
    with ecocode.run("inference", project_id=42, api_key="ecocode_...") as eco_run:
        tracker = InferencePhaseTracker(eco_run)
        tracker.on_prefill_begin(prompt_tokens=512)
        # ... run prefill ...
        tracker.on_decode_begin()
        for token in generated_tokens:
            # ... decode step ...
            pass
        tracker.on_generation_end(total_tokens=512 + len(generated_tokens))
"""
from __future__ import annotations

from typing import TYPE_CHECKING, Optional, AsyncIterator

if TYPE_CHECKING:
    from .. import EcoCodeRun


class EcoCodeVLLMMiddleware:
    """
    Async generator middleware for vLLM AsyncLLMEngine.

    Wraps the engine's generate() call to mark prefill and decode phases.
    Token counts accumulate over the course of a single request (prompt +
    tokens generated so far), resetting with each new call to generate().
    """

    def __init__(self, engine, run: "EcoCodeRun") -> None:
        self._engine = engine
        self._run = run
        self._total_tokens = 0

    async def generate(
        self,
        prompt: str,
        sampling_params,
        request_id: str,
        prompt_token_ids: Optional[list] = None,
    ) -> AsyncIterator:
        """
        Wraps AsyncLLMEngine.generate() with prefill/decode phase markers.
        """
        # Estimate prompt length for sequence_length logging
        if prompt_token_ids is not None:
            prompt_len = len(prompt_token_ids)
        else:
            # Rough approximation when token IDs are not available
            prompt_len = len(prompt.split()) * 4 // 3

        self._run.mark("phase", "prefill")
        self._run.log_sequence_length(prompt_len)

        first_token = True
        kwargs = {}
        if prompt_token_ids is not None:
            kwargs["prompt_token_ids"] = prompt_token_ids

        async for output in self._engine.generate(
            prompt, sampling_params, request_id, **kwargs
        ):
            if first_token and output.outputs:
                # First token generated: prefill is done, decode begins
                self._run.mark("phase", "decode")
                first_token = False

            if output.outputs:
                generated_len = len(output.outputs[0].token_ids)
                self._total_tokens = prompt_len + generated_len
                self._run.log_tokens(self._total_tokens)

            yield output


class InferencePhaseTracker:
    """
    Lightweight helper for tracking inference phases in any inference loop
    (not just vLLM). Use this when you have direct control over the loop.

    Example:
        tracker = InferencePhaseTracker(eco_run)
        tracker.on_model_load()
        # load model weights...
        tracker.on_prefill_begin(prompt_tokens=1024)
        # run prefill...
        tracker.on_decode_begin()
        for token in decode_loop():
            # generate one token
            pass
        tracker.on_generation_end(total_tokens=1024 + num_generated)
    """

    def __init__(self, run: "EcoCodeRun") -> None:
        self._run = run

    def on_model_load(self) -> None:
        """Call before loading model weights from disk to GPU."""
        self._run.mark("phase", "model_load")

    def on_prefill_begin(self, prompt_tokens: Optional[int] = None) -> None:
        """Call when prefill (prompt processing) starts."""
        self._run.mark("phase", "prefill")
        if prompt_tokens is not None:
            self._run.log_sequence_length(prompt_tokens)

    def on_decode_begin(self) -> None:
        """Call when autoregressive decode starts (after first token is generated)."""
        self._run.mark("phase", "decode")

    def on_generation_end(self, total_tokens: Optional[int] = None) -> None:
        """Call when a generation request is complete."""
        if total_tokens is not None:
            self._run.log_tokens(total_tokens)
