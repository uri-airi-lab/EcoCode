"""
EcoCode Python SDK

Track energy consumption of ML training and inference runs: phases, epochs,
token counts, optimizer steps, loss, learning rate, and sequence length.

Three equivalent usage patterns:

  Context manager:
      with ecocode.run("experiment", exporter="terminal") as r:
          r.mark("phase", "forward")
          r.log_loss(1.45)

  Decorator:
      @ecocode.track("experiment", exporter="terminal")
      def train(): ...

  Explicit start/stop:
      r = ecocode.run("experiment", exporter="terminal")
      r.start()
      train()
      r.stop()

Exporter options:
  exporter="terminal" (default)   Print live metrics to stdout.
  exporter="csv"                  Write to a CSV file (requires output_file=).
  exporter="json"                 Write to a JSON file (requires output_file=).
  exporter="online"               Send to the EcoCode dashboard
                                  (requires api_key= or ECOCODE_API_KEY).

Attached mode (CLI wrapping) is auto-detected from environment variables.

ML Phase labels (use these consistently for cross-run comparisons):
  Pretraining / SFT : data_loading, tokenization, forward, backward,
                      optimizer_step, eval, checkpoint_save, done
  RLHF              : reward_model, policy_rollout, reference_forward,
                      ppo_update, dpo_update, grpo_update, kl_penalty
  Inference         : model_load, prefill, decode, kv_cache_ops, done

Additional methods:
  run.log_custom(key, value)      Log arbitrary numeric metrics (gradient norm,
                                  perplexity, BLEU, reward score, batch size, etc.)

Framework integrations:
  ecocode.integrations.huggingface    HuggingFace Trainer
  ecocode.integrations.trl            TRL (SFT, DPO, GRPO)
  ecocode.integrations.lightning      PyTorch Lightning
  ecocode.integrations.deepspeed      DeepSpeed
  ecocode.integrations.vllm           vLLM (async inference)
  ecocode.integrations.keras          Keras 3 / tf.keras
  ecocode.integrations.onnxruntime    ONNX Runtime inference
"""
from __future__ import annotations

from functools import wraps
from typing import Callable, Optional, TypeVar

from ._run import EcoCodeRun

__version__ = "1.2.0"
__all__ = ["run", "track", "EcoCodeRun"]

F = TypeVar("F", bound=Callable)


def run(
    name: str = "run",
    *,
    exporter: str = "terminal",
    output_file: Optional[str] = None,
    db_path: Optional[str] = None,
    api_key: Optional[str] = None,
    project_id: Optional[int] = None,
    base_url: Optional[str] = None,
    interval: float = 1.0,
) -> EcoCodeRun:
    """
    Return an EcoCodeRun context manager.

    Parameters
    ----------
    name : str
        Human-readable label for this run.
    exporter : str
        Where to store results:
        - "terminal" (default): print metrics to stdout.
        - "csv": CSV file (requires output_file=).
        - "json": JSON file (requires output_file=).
        - "online": EcoCode platform (requires api_key=).
    output_file : str, optional
        Output path for csv/json exporters.
    db_path : str, optional
        Ignored, with a warning. Accepted only so older call sites keep working.
    api_key : str, optional
        API key for the online exporter. Also read from ECOCODE_API_KEY.
    project_id : int, optional
        Project ID for the online exporter. Inferred from API key if omitted.
    base_url : str, optional
        Ignored (with a warning). The EcoCode backend URL is fixed:
        https://ecocode.harmonyhpc.io
    interval : float
        Sampling interval in seconds (default: 1.0).

    In attached mode (under `ecocode run`), most parameters are ignored: the
    run was already created by the CLI.
    """
    return EcoCodeRun(
        name,
        exporter=exporter,
        output_file=output_file,
        db_path=db_path,
        api_key=api_key,
        project_id=project_id,
        base_url=base_url,
        interval=interval,
    )


def track(
    name: Optional[str] = None,
    *,
    exporter: str = "terminal",
    output_file: Optional[str] = None,
    db_path: Optional[str] = None,
    api_key: Optional[str] = None,
    project_id: Optional[int] = None,
    base_url: Optional[str] = None,
    interval: float = 1.0,
) -> Callable[[F], F]:
    """
    Decorator that wraps a function in an EcoCodeRun context.

    Usage:
        @ecocode.track(exporter="terminal")
        def train():
            ...

        @ecocode.track(exporter="online", api_key="ecocode_...")
        def train():
            ...
    """
    def decorator(fn: F) -> F:
        run_name = name or fn.__name__

        @wraps(fn)
        def wrapper(*args, **kwargs):
            with EcoCodeRun(
                run_name,
                exporter=exporter,
                output_file=output_file,
                db_path=db_path,
                api_key=api_key,
                project_id=project_id,
                base_url=base_url,
                interval=interval,
            ):
                return fn(*args, **kwargs)

        return wrapper  # type: ignore[return-value]

    return decorator
