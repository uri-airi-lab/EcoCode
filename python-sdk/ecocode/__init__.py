"""
EcoCode Python SDK

Track energy consumption of ML training and inference runs — phases, epochs,
token counts, optimizer steps, loss, learning rate, and sequence length.

Three equivalent usage patterns:

  Context manager:
      with ecocode.run("experiment", api_key="eco_...") as r:
          r.mark("phase", "forward")
          r.log_loss(1.45)

  Decorator:
      @ecocode.track("experiment", api_key="eco_...")
      def train(): ...

  Explicit start/stop:
      r = ecocode.run("experiment", api_key="eco_...")
      r.start()
      train()
      r.stop()

Attached mode (CLI wrapping) is auto-detected from environment variables.

ML Phase labels (use these consistently for cross-run comparisons):
  Pretraining / SFT : data_loading, tokenization, forward, backward,
                      optimizer_step, eval, checkpoint_save, done
  RLHF              : reward_model, policy_rollout, reference_forward,
                      ppo_update, dpo_update, grpo_update, kl_penalty
  Inference         : model_load, prefill, decode, kv_cache_ops, done

Additional methods:
  run.log_custom(key, value)    — Log arbitrary numeric metrics (gradient norm,
                                  perplexity, BLEU, reward score, batch size, etc.)

Framework integrations:
  ecocode.integrations.huggingface  — HuggingFace Trainer
  ecocode.integrations.trl          — TRL (SFT, DPO, GRPO)
  ecocode.integrations.lightning    — PyTorch Lightning
  ecocode.integrations.deepspeed    — DeepSpeed
  ecocode.integrations.vllm         — vLLM (async inference)
  ecocode.integrations.keras        — Keras 3 / tf.keras
  ecocode.integrations.onnxruntime  — ONNX Runtime inference
"""
from __future__ import annotations

from functools import wraps
from typing import Callable, Optional, TypeVar

from ._run import EcoCodeRun

__version__ = "0.2.0"
__all__ = ["run", "track", "EcoCodeRun"]

F = TypeVar("F", bound=Callable)


def run(
    name: str = "run",
    *,
    api_key: Optional[str] = None,
    project_id: Optional[int] = None,
    base_url: Optional[str] = None,
    interval: float = 1.0,
) -> EcoCodeRun:
    """
    Return an EcoCodeRun context manager.

    In attached mode (CLI running), name/api_key/project_id are ignored —
    the run was already created by the CLI.

    When using a project-scoped API key (generated from the dashboard with a
    project selected), project_id can be omitted — the backend infers it from
    the key automatically.
    """
    return EcoCodeRun(
        name,
        api_key=api_key,
        project_id=project_id,
        base_url=base_url,
        interval=interval,
    )


def track(
    name: Optional[str] = None,
    *,
    api_key: Optional[str] = None,
    project_id: Optional[int] = None,
    base_url: Optional[str] = None,
    interval: float = 1.0,
) -> Callable[[F], F]:
    """
    Decorator that wraps a function in an EcoCodeRun context.

    Usage:
        @ecocode.track(project_id=42, api_key="eco_...")
        def train():
            ...
    """
    def decorator(fn: F) -> F:
        run_name = name or fn.__name__

        @wraps(fn)
        def wrapper(*args, **kwargs):
            with EcoCodeRun(
                run_name,
                api_key=api_key,
                project_id=project_id,
                base_url=base_url,
                interval=interval,
            ):
                return fn(*args, **kwargs)

        return wrapper  # type: ignore[return-value]

    return decorator
