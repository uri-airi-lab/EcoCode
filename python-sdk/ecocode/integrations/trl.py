"""
TRL (Transformer Reinforcement Learning) integration.

Provides separate callbacks for each alignment training paradigm because
their energy profiles differ fundamentally:

  SFTTrainer   — standard supervised fine-tuning (forward + backward + step)
  DPOTrainer   — direct preference optimisation (policy forward +
                 reference forward + backward, no reward model)
  GRPOTrainer  — group relative policy optimisation (policy rollout via
                 generation + grpo gradient update, no reference model)

Usage:
    import ecocode
    from ecocode.integrations.trl import EcoCodeSFTCallback

    with ecocode.run("sft-run", project_id=42, api_key="eco_...") as eco_run:
        trainer = SFTTrainer(
            model=model,
            args=args,
            callbacks=[EcoCodeSFTCallback(eco_run)],
        )
        trainer.train()
"""
from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from .. import EcoCodeRun

try:
    from transformers import TrainerCallback as _Base
except ImportError:
    _Base = object


class EcoCodeSFTCallback(_Base):
    """
    Callback for trl.SFTTrainer.

    SFT energy profile: data loading → forward → backward → optimizer step → eval
    """

    def __init__(self, run: "EcoCodeRun") -> None:
        self._run = run

    def on_train_begin(self, args, state, control, **kwargs):
        self._run.mark("phase", "data_loading")
        # max_seq_length was renamed to max_length in TRL 0.14+; check both
        seq_len = getattr(args, "max_length", None) or getattr(args, "max_seq_length", None)
        if seq_len:
            self._run.log_sequence_length(int(seq_len))

    def on_epoch_begin(self, args, state, control, **kwargs):
        self._run.log_epoch(int(state.epoch or 0))

    def on_step_begin(self, args, state, control, **kwargs):
        self._run.mark("phase", "forward")

    def on_optimizer_step(self, args, state, control, **kwargs):
        self._run.mark("phase", "optimizer_step")
        self._run.log_step(state.global_step)
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
        tokens = getattr(state, "num_input_tokens_seen", None)
        if tokens is not None:
            self._run.log_tokens(int(tokens))
        self._run.mark("phase", "done")


class EcoCodeDPOCallback(_Base):
    """
    Callback for trl.DPOTrainer.

    DPO energy profile: each step runs a policy forward pass AND a reference
    model forward pass (for KL regularisation), then one backward + optimizer step.
    Energy per step is roughly 2× SFT forward + 1× backward.
    No reward model inference is needed (unlike PPO).
    """

    def __init__(self, run: "EcoCodeRun") -> None:
        self._run = run

    def on_train_begin(self, args, state, control, **kwargs):
        self._run.mark("phase", "data_loading")
        # max_seq_length was renamed to max_length in TRL 0.14+; check both
        seq_len = getattr(args, "max_length", None) or getattr(args, "max_seq_length", None)
        if seq_len:
            self._run.log_sequence_length(int(seq_len))

    def on_epoch_begin(self, args, state, control, **kwargs):
        self._run.log_epoch(int(state.epoch or 0))

    def on_step_begin(self, args, state, control, **kwargs):
        # DPO step: policy forward + reference forward happen together
        self._run.mark("phase", "forward")

    def on_optimizer_step(self, args, state, control, **kwargs):
        self._run.mark("phase", "dpo_update")
        self._run.log_step(state.global_step)
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
        self._run.mark("phase", "done")


class EcoCodeGRPOCallback(_Base):
    """
    Callback for trl.GRPOTrainer (Group Relative Policy Optimisation).

    GRPO energy profile: each step alternates between:
      1. policy_rollout — autoregressive generation for a group of prompts
                          (memory-bandwidth-bound, like decode in inference)
      2. grpo_update    — gradient update using group-relative rewards
                          (compute-bound, like a standard backward pass)

    No separate reward model, no reference model forward pass.
    Used in DeepSeek-R1, Qwen reasoning models.
    """

    def __init__(self, run: "EcoCodeRun") -> None:
        self._run = run

    def on_train_begin(self, args, state, control, **kwargs):
        self._run.mark("phase", "data_loading")
        # max_seq_length was renamed to max_length in TRL 0.14+; also check max_completion_length for GRPO
        seq_len = (getattr(args, "max_length", None)
                   or getattr(args, "max_seq_length", None)
                   or getattr(args, "max_completion_length", None))
        if seq_len:
            self._run.log_sequence_length(int(seq_len))

    def on_epoch_begin(self, args, state, control, **kwargs):
        self._run.log_epoch(int(state.epoch or 0))

    def on_step_begin(self, args, state, control, **kwargs):
        # GRPO step begins with rollout (generation) before the gradient update
        self._run.mark("phase", "policy_rollout")

    def on_optimizer_step(self, args, state, control, **kwargs):
        self._run.mark("phase", "grpo_update")
        self._run.log_step(state.global_step)
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
        self._run.mark("phase", "done")
