"""
Heavy Keras Integration Test
- Model:  Wide EfficientNet-like CNN (~12M params, built with Keras)
- Task:   100-class image classification on synthetic data
- Backend: torch (CUDA if available, else CPU, scaled down)
- Callback: EcoCodeCallback, phases, loss, accuracy, lr, custom metrics
"""
import numpy as np
import pytest

keras = pytest.importorskip("keras")
torch = pytest.importorskip("torch")

from keras import layers  # noqa: E402

from ecocode.integrations.keras import EcoCodeCallback  # noqa: E402

N_CLASSES = 100


def se_block(x, filters, ratio=16):
    se = layers.GlobalAveragePooling2D()(x)
    se = layers.Reshape((1, 1, filters))(se)
    se = layers.Conv2D(max(1, filters // ratio), 1, activation="swish")(se)
    se = layers.Conv2D(filters, 1, activation="sigmoid")(se)
    return layers.Multiply()([x, se])


def mbconv_block(x, filters, strides=1, expand=6):
    expanded = x.shape[-1] * expand
    x = layers.Conv2D(expanded, 1, use_bias=False)(x)
    x = layers.BatchNormalization()(x)
    x = layers.Activation("swish")(x)
    x = layers.DepthwiseConv2D(3, strides=strides, padding="same", use_bias=False)(x)
    x = layers.BatchNormalization()(x)
    x = layers.Activation("swish")(x)
    x = se_block(x, expanded)
    x = layers.Conv2D(filters, 1, use_bias=False)(x)
    return layers.BatchNormalization()(x)


def build_model(img):
    inputs = keras.Input(shape=(img, img, 3))
    x = layers.Conv2D(40, 3, strides=2, padding="same", use_bias=False)(inputs)
    x = layers.BatchNormalization()(x)
    x = layers.Activation("swish")(x)
    for _ in range(2):
        x = mbconv_block(x, 24)
    x = mbconv_block(x, 48, strides=2)
    for _ in range(3):
        x = mbconv_block(x, 48)
    x = mbconv_block(x, 96, strides=2)
    for _ in range(3):
        x = mbconv_block(x, 96)
    x = mbconv_block(x, 192, strides=2)
    for _ in range(4):
        x = mbconv_block(x, 192)
    x = mbconv_block(x, 320, strides=2)
    x = mbconv_block(x, 320)
    x = layers.Conv2D(1280, 1, use_bias=False)(x)
    x = layers.BatchNormalization()(x)
    x = layers.Activation("swish")(x)
    x = layers.GlobalAveragePooling2D()(x)
    x = layers.Dropout(0.3)(x)
    outputs = layers.Dense(N_CLASSES, activation="softmax")(x)
    model = keras.Model(inputs, outputs)
    model.compile(optimizer=keras.optimizers.Adam(1e-3), loss="sparse_categorical_crossentropy", metrics=["accuracy"])
    return model


class Capture(keras.callbacks.Callback):
    def __init__(self):
        super().__init__()
        self.batch_logs, self.test_logs = [], []

    def on_train_batch_end(self, batch, logs=None):
        self.batch_logs.append(dict(logs or {}))

    def on_test_end(self, logs=None):
        self.test_logs.append(dict(logs or {}))


def test_heavy_keras(device, scaled, heavy_run, marker_tap):
    img = 128 if device == "cuda" else 64
    n_train, n_val = scaled(20000, 64), scaled(4000, 16)
    batch = 128 if device == "cuda" else 16
    epochs = scaled(10, 2)
    x_train = np.random.randn(n_train, img, img, 3).astype("float32")
    y_train = np.random.randint(0, N_CLASSES, n_train)
    x_val = np.random.randn(n_val, img, img, 3).astype("float32")
    y_val = np.random.randint(0, N_CLASSES, n_val)

    model = build_model(img)
    print(f"[Keras heavy] {model.count_params() / 1e6:.2f}M params, backend={keras.backend.backend()}")
    capture = Capture()
    with heavy_run("keras-efficientnet-wide") as eco_run:
        history = model.fit(x_train, y_train, epochs=epochs, batch_size=batch, validation_data=(x_val, y_val),
                            callbacks=[EcoCodeCallback(eco_run), capture], verbose=0)

    n_steps = len(capture.batch_logs)
    assert n_steps == epochs * int(np.ceil(n_train / batch))
    phases = marker_tap.phases()
    assert phases[0] == "data_loading" and phases[-1] == "done"
    per_batch = [p for p in phases if p in ("forward_backward", "optimizer_step")]
    assert per_batch == ["forward_backward", "optimizer_step"] * n_steps
    assert phases.count("eval") == epochs
    assert marker_tap.values("epoch") == list(range(epochs))
    assert marker_tap.values("step") == list(range(n_steps))
    assert marker_tap.values("loss") == pytest.approx([float(l["loss"]) for l in capture.batch_logs])
    assert marker_tap.values("accuracy") == pytest.approx([float(l["accuracy"]) * 100 for l in capture.batch_logs])
    assert marker_tap.customs("val_loss") == pytest.approx([float(l["loss"]) for l in capture.test_logs])
    assert history.history["val_accuracy"][-1] == pytest.approx(marker_tap.customs("val_accuracy")[-1])
    if not heavy_run.online:
        assert heavy_run.last()["phase"] == "done"
        assert {"val_loss", "val_accuracy"} <= heavy_run.custom_keys()
