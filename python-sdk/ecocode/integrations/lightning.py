"""
PyTorch Lightning integration.

Usage:
    import ecocode
    import pytorch_lightning as pl
    from ecocode.integrations.lightning import EcoCodeCallback

    with ecocode.run("pretrain", project_id=42, api_key="eco_...") as eco_run:
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
      on_train_epoch_start     → log epoch (no phase change)
      on_train_batch_start     → phase = "forward_backward"
      on_before_optimizer_step → phase = "optimizer_step", log step
      on_validation_epoch_start → phase = "eval"
      on_save_checkpoint       → phase = "checkpoint_save"
      on_train_end              → phase = "done"

    The phase is marked once per step via ``on_train_batch_start`` rather than
    once per epoch, so that every step's forward and backward pass is
    correctly attributed to "forward_backward" instead of inheriting the
    previous step's "optimizer_step" marker. Lightning exposes no separate
    backward-pass hook, hence the combined name (see the HuggingFace
    integration for the same convention).
    """

    def __init__(self, run: "EcoCodeRun") -> None:
        self._run = run

    def on_train_start(self, trainer, pl_module):
        self._run.mark("phase", "data_loading")

    def on_train_epoch_start(self, trainer, pl_module):
        self._run.log_epoch(trainer.current_epoch)

    def on_train_batch_start(self, trainer, pl_module, batch, batch_idx):
        self._run.mark("phase", "forward_backward")

    def on_before_optimizer_step(self, trainer, pl_module, optimizer):
        self._run.mark("phase", "optimizer_step")
        self._run.log_step(trainer.global_step)
        # Pull loss from the callback metrics if available
        logged = trainer.callback_metrics
        if "train_loss" in logged:
            self._run.log_loss(float(logged["train_loss"]))
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
