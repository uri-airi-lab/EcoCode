"""
PyTorch Lightning integration.

Usage:
    import ecocode
    import pytorch_lightning as pl
    from ecocode.integrations.lightning import EcoCodeCallback

    with ecocode.run("pretrain", project_id=42, api_key="ecocode_...") as eco_run:
        trainer = pl.Trainer(
            callbacks=[EcoCodeCallback(eco_run)],
        )
        trainer.fit(model, datamodule=dm)
"""
from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from .. import EcoCodeRun

try:
    from pytorch_lightning.callbacks import Callback as _Base
except ImportError:
    _Base = object


class EcoCodeCallback(_Base):
    """
    PyTorch Lightning Callback that marks training phases and logs metrics.

    Hooks:
      on_train_start             → phase = "data_loading"
      on_train_epoch_start       → log epoch (no phase change)
      on_train_batch_start       → phase = "forward"
      on_before_backward         → phase = "backward"
      on_before_optimizer_step   → phase = "optimizer_step", log step, loss, lr
      on_validation_epoch_start  → phase = "eval"
      on_save_checkpoint         → phase = "checkpoint_save"
      on_train_end               → phase = "done"

    Unlike HuggingFace/TRL/Keras, Lightning exposes a real hook between the
    forward and backward pass (``on_before_backward``), fired in this order:
    on_train_batch_start -> on_before_backward -> on_after_backward ->
    on_before_optimizer_step. This lets the forward and backward phases be
    marked separately instead of merged into a single "forward_backward"
    interval.

    Both ``on_before_backward`` and ``on_before_optimizer_step`` are part of
    Lightning's automatic-optimization training loop and are not reliably
    invoked when a LightningModule sets ``self.automatic_optimization =
    False`` and calls ``self.manual_backward(loss)`` itself; this callback
    assumes automatic optimization, the default and common case.

    The loss logged is the module's ``train_loss`` metric when it logs one,
    otherwise the loss returned by ``training_step``.
    """

    def __init__(self, run: "EcoCodeRun") -> None:
        self._run = run
        self._last_loss = None

    def on_train_start(self, trainer, pl_module):
        self._run.mark("phase", "data_loading")

    def on_train_epoch_start(self, trainer, pl_module):
        self._run.log_epoch(trainer.current_epoch)

    def on_train_batch_start(self, trainer, pl_module, batch, batch_idx):
        self._run.mark("phase", "forward")

    def on_before_backward(self, trainer, pl_module, loss):
        self._run.mark("phase", "backward")
        # Kept as a tensor: converting here would sync the GPU on every micro-batch.
        self._last_loss = loss.detach()

    def on_before_optimizer_step(self, trainer, pl_module, optimizer):
        self._run.mark("phase", "optimizer_step")
        self._run.log_step(trainer.global_step)
        logged = trainer.callback_metrics
        if "train_loss" in logged:
            self._run.log_loss(float(logged["train_loss"]))
        elif self._last_loss is not None:
            # Lightning divides the loss by accumulate_grad_batches before backward;
            # undo it so the value matches what training_step returned.
            self._run.log_loss(float(self._last_loss) * trainer.accumulate_grad_batches)
            self._last_loss = None
        # Pull lr from the optimizer
        for pg in optimizer.param_groups:
            if "lr" in pg:
                self._run.log_lr(float(pg["lr"]))
                break

    def on_validation_epoch_start(self, trainer, pl_module):
        self._run.mark("phase", "eval")

    def on_save_checkpoint(self, trainer, pl_module, checkpoint):
        self._run.mark("phase", "checkpoint_save")

    def on_train_end(self, trainer, pl_module):
        self._run.mark("phase", "done")
