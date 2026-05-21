"""
EcoCode — Keras / TensorFlow Integration.

Provides an ``EcoCodeCallback`` for Keras 3+ (or tf.keras) that automatically
tracks training phases, epoch boundaries, and key metrics.

Usage::

    import ecocode
    from ecocode.integrations.keras import EcoCodeCallback

    with ecocode.run("keras-train", project_id=42, api_key="eco_...") as eco_run:
        model.fit(x, y, callbacks=[EcoCodeCallback(eco_run)])
"""
from __future__ import annotations
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from .. import EcoCodeRun

from .._validation import validate_metric_key as _validate_metric_key

try:
    from keras.callbacks import Callback as _Base
except ImportError:
    try:
        from tensorflow.keras.callbacks import Callback as _Base  # type: ignore[no-redef]
    except ImportError:
        _Base = object  # type: ignore[assignment, misc]


class EcoCodeCallback(_Base):
    """
    Keras Callback that tracks training phases and metrics.

    Hooks:
      on_train_begin       → phase = "data_loading"
      on_epoch_begin       → log_epoch(epoch)
      on_train_batch_begin → phase = "forward"
      on_train_batch_end   → phase = "optimizer_step", log step + loss + lr
      on_test_begin        → phase = "eval"
      on_train_end         → phase = "done"
    """

    def __init__(self, run: "EcoCodeRun") -> None:
        super().__init__()
        self._run = run
        self._global_step = 0

    # ── Training lifecycle ────────────────────────────────────────────

    def on_train_begin(self, logs=None):
        self._run.mark("phase", "data_loading")

    def on_epoch_begin(self, epoch, logs=None):
        self._run.log_epoch(epoch)

    def on_train_batch_begin(self, batch, logs=None):
        self._run.mark("phase", "forward")

    def on_train_batch_end(self, batch, logs=None):
        self._run.mark("phase", "optimizer_step")
        self._run.log_step(self._global_step)
        self._global_step += 1

        if logs:
            if "loss" in logs:
                self._run.log_loss(float(logs["loss"]))
            # Keras exposes LR as "lr" or "learning_rate"
            lr = logs.get("lr", logs.get("learning_rate"))
            if lr is not None:
                self._run.log_lr(float(lr))
            # Accuracy (Keras may use "accuracy" or "acc")
            acc = logs.get("accuracy", logs.get("acc"))
            if acc is not None:
                self._run.log_accuracy(float(acc) * 100)
            # Forward any other numeric values as custom metrics.
            # CS-TYPE-001: Validate the key before forwarding to prevent keys
            # with control characters or excessive length reaching the backend.
            _KNOWN = {"loss", "lr", "learning_rate", "accuracy", "acc", "batch", "size"}
            for key, value in logs.items():
                if key not in _KNOWN and isinstance(value, (int, float)):
                    try:
                        _validate_metric_key(key)
                        self._run.log_custom(key, float(value))
                    except ValueError:
                        pass  # Skip keys that fail validation silently

    # ── Eval & end ────────────────────────────────────────────────────

    def on_test_begin(self, logs=None):
        self._run.mark("phase", "eval")

    def on_test_end(self, logs=None):
        if logs:
            for key, value in logs.items():
                if isinstance(value, (int, float)):
                    # CS-TYPE-001: Validate key before forwarding.
                    try:
                        _validate_metric_key(f"val_{key}")
                        self._run.log_custom(f"val_{key}", float(value))
                    except ValueError:
                        pass  # Skip keys that fail validation silently

    def on_train_end(self, logs=None):
        self._run.mark("phase", "done")
