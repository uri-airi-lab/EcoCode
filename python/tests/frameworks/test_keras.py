"""EcoCodeCallback (Keras 3, torch backend) through a real model.fit()."""
import numpy as np
import pytest

keras = pytest.importorskip("keras")
pytest.importorskip("torch")

from ecocode.integrations.keras import EcoCodeCallback  # noqa: E402

EPOCHS = 2
BATCH = 8
N = 64


class Capture(keras.callbacks.Callback):
    """A second real Keras callback that records the exact logs dicts Keras passes."""

    def __init__(self):
        super().__init__()
        self.batch_logs = []
        self.test_logs = []

    def on_train_batch_end(self, batch, logs=None):
        self.batch_logs.append(dict(logs or {}))

    def on_test_end(self, logs=None):
        self.test_logs.append(dict(logs or {}))


def test_keras_callback_real_fit(marker_tap, json_run):
    rng = np.random.default_rng(0)
    x = rng.standard_normal((N, 8)).astype("float32")
    y = (x[:, 0] > 0).astype("int32")
    model = keras.Sequential([keras.layers.Input((8,)), keras.layers.Dense(16, activation="relu"), keras.layers.Dense(2)])
    model.compile(
        optimizer=keras.optimizers.SGD(learning_rate=0.1),
        loss=keras.losses.SparseCategoricalCrossentropy(from_logits=True),
        metrics=["accuracy"],
    )
    capture = Capture()
    with json_run("keras") as run:
        # EcoCode's callback goes first so Keras invokes it before the capture
        # callback; both receive the same logs dict for each batch.
        model.fit(x, y, epochs=EPOCHS, batch_size=BATCH, validation_data=(x[:16], y[:16]),
                  callbacks=[EcoCodeCallback(run), capture], verbose=0)

    steps_per_epoch = N // BATCH
    n_steps = EPOCHS * steps_per_epoch
    assert len(capture.batch_logs) == n_steps

    phases = marker_tap.phases()
    assert phases[0] == "data_loading"
    assert phases[-1] == "done"
    per_batch = [p for p in phases if p in ("forward_backward", "optimizer_step")]
    assert per_batch == ["forward_backward", "optimizer_step"] * n_steps
    assert phases.count("eval") == EPOCHS  # validation runs once per epoch

    assert marker_tap.values("epoch") == list(range(EPOCHS))
    assert marker_tap.values("step") == list(range(n_steps))
    assert marker_tap.values("loss") == pytest.approx([float(l["loss"]) for l in capture.batch_logs])
    assert marker_tap.values("accuracy") == pytest.approx([float(l["accuracy"]) * 100 for l in capture.batch_logs])
    # Validation metrics are forwarded as val_* custom metrics.
    assert len(capture.test_logs) == EPOCHS
    assert marker_tap.customs("val_loss") == pytest.approx([float(l["loss"]) for l in capture.test_logs])
    assert marker_tap.customs("val_accuracy") == pytest.approx([float(l["accuracy"]) for l in capture.test_logs])

    assert json_run.last()["phase"] == "done"
    assert json_run.max_of("training_step") == n_steps - 1
    assert {"val_loss", "val_accuracy"} <= json_run.custom_keys()
