"""
HuggingFace Transformers integration.

Works with Trainer and SFTTrainer. Automatically marks training phases
(forward, optimizer_step, eval, checkpoint_save) and logs token counts,
step number, loss, and learning rate from the trainer state.

Usage:
    import ecocode
    from ecocode.integrations.huggingface import EcoCodeCallback

    with ecocode.run("sft-llama3", project_id=42, api_key="eco_...") as eco_run:
        trainer = Trainer(
            model=model,
            args=training_args,
            callbacks=[EcoCodeCallback(eco_run)],
        )
        trainer.train()
"""
from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from .. import EcoCodeRun

try:
    from transformers import TrainerCallback as _TrainerCallbackBase
except ImportError:
    _TrainerCallbackBase = object  # graceful degradation


class EcoCodeCallback(_TrainerCallbackBase):
    """
    TrainerCallback that tracks ML training phases and metrics for EcoCode.

    Hooks fired per training step:
      on_step_begin      → phase = "forward_backward" (fires once per optimizer
                           update; HF exposes no separate backward-pass hook, so
                           this marker covers every micro-batch's forward and
                           backward pass under gradient accumulation)
      on_optimizer_step  → phase = "optimizer_step", logs step + loss + lr
      on_evaluate        → phase = "eval"
      on_save            → phase = "checkpoint_save"
      on_epoch_begin     → logs epoch number
      on_train_end       → logs total token count, phase = "done"
    """

    def __init__(self, run: "EcoCodeRun") -> None:
        self._run = run

    def on_train_begin(self, args, state, control, **kwargs):
        self._run.mark("phase", "data_loading")
        # Log sequence length if available in training args
        seq_len = getattr(args, "max_seq_length", None) or getattr(args, "max_length", None)
        if seq_len:
            self._run.log_sequence_length(int(seq_len))

    def on_epoch_begin(self, args, state, control, **kwargs):
        epoch = int(state.epoch or 0)
        self._run.log_epoch(epoch)

    def on_step_begin(self, args, state, control, **kwargs):
        self._run.mark("phase", "forward_backward")

    def on_optimizer_step(self, args, state, control, **kwargs):
        self._run.mark("phase", "optimizer_step")
        self._run.log_step(state.global_step)
        # Pull the most recent loss and lr from the trainer's log history
        if state.log_history:
            latest = state.log_history[-1]
            if "loss" in latest:
                self._run.log_loss(float(latest["loss"]))
            if "learning_rate" in latest:
                self._run.log_lr(float(latest["learning_rate"]))

    def on_evaluate(self, args, state, control, **kwargs):
        self._run.mark("phase", "eval")

    def on_save(self, args, state, control, **kwargs):
        self._run.mark("phase", "checkpoint_save")

    def on_train_end(self, args, state, control, **kwargs):
        # num_input_tokens_seen is available in recent transformers versions
        tokens = getattr(state, "num_input_tokens_seen", None)
        if tokens is not None:
            self._run.log_tokens(int(tokens))
        self._run.mark("phase", "done")
