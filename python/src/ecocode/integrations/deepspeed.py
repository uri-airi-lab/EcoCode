"""
DeepSpeed integration.

DeepSpeed ZeRO stages have very different energy profiles:
  Stage 0: no sharding, standard DDP energy
  Stage 1: optimizer state sharding; reduces memory, minimal comms overhead
  Stage 2: optimizer + gradient sharding; some all-reduce communication
  Stage 3: full parameter sharding; highest communication overhead per step

Use EcoCodeDeepSpeedCallback as a manual wrapper around your DeepSpeed
training loop. Call the hook methods at the appropriate points.

Usage:
    import ecocode
    from ecocode.integrations.deepspeed import EcoCodeDeepSpeedCallback

    with ecocode.run("pretrain-70b", project_id=42, api_key="ecocode_...") as eco_run:
        ds_cb = EcoCodeDeepSpeedCallback(eco_run, zero_stage=3)

        for step, batch in enumerate(loader):
            ds_cb.on_step_begin(step)

            outputs = engine(batch)
            loss = outputs.loss

            ds_cb.on_backward_begin()
            engine.backward(loss)
            ds_cb.on_backward_end()

            engine.step()
            ds_cb.on_optimizer_step(step, loss.item())

        ds_cb.on_train_end()
"""
from __future__ import annotations

from typing import TYPE_CHECKING, Optional

if TYPE_CHECKING:
    from .. import EcoCodeRun


class EcoCodeDeepSpeedCallback:
    """
    Manual hook-based callback for DeepSpeed training loops.

    Records phase transitions and ZeRO stage metadata so the dashboard
    can show the communication overhead cost across stages.
    """

    def __init__(self, run: "EcoCodeRun", zero_stage: int = 0) -> None:
        self._run = run
        self._zero_stage = zero_stage
        # Log ZeRO stage as a one-time marker at run start
        run.mark("phase", f"deepspeed_zero{zero_stage}_init")

    def on_data_loading_begin(self) -> None:
        self._run.mark("phase", "data_loading")

    def on_step_begin(self, step: int) -> None:
        # Default to forward phase at step start
        self._run.mark("phase", "forward")

    def on_forward_begin(self) -> None:
        self._run.mark("phase", "forward")

    def on_forward_end(self) -> None:
        # no-op; phase stays "forward" until backward starts
        pass

    def on_backward_begin(self) -> None:
        self._run.mark("phase", "backward")

    def on_backward_end(self) -> None:
        # DeepSpeed runs allreduce/allgather after backward depending on ZeRO
        # stage; phase stays "backward" until the optimizer step
        pass

    def on_optimizer_step(self, step: int, loss: Optional[float] = None) -> None:
        self._run.mark("phase", "optimizer_step")
        self._run.log_step(step)
        if loss is not None:
            self._run.log_loss(loss)

    def on_eval_begin(self) -> None:
        self._run.mark("phase", "eval")

    def on_checkpoint_save(self) -> None:
        self._run.mark("phase", "checkpoint_save")

    def on_train_end(self) -> None:
        self._run.mark("phase", "done")
