"""
Heavy Keras Integration Test
- Model:  Wide EfficientNet-like CNN (~12M params, built with Keras)
- Task:   100-class image classification (ImageNet-scale synthetic data)
- Backend: torch (GPU)
- Callback: EcoCodeCallback — phases, loss, accuracy, lr, custom metrics
"""
import os
os.environ.setdefault("ECOCODE_API_URL", "http://localhost:3001")
os.environ["KERAS_BACKEND"] = "torch"

import numpy as np
import keras
from keras import layers
import torch
import ecocode
from ecocode.integrations.keras import EcoCodeCallback

print(f"[Keras heavy] keras={keras.__version__}  backend={keras.backend.backend()}")
device = "cuda" if torch.cuda.is_available() else "cpu"
print(f"[Keras heavy] device={device}")

N_TRAIN, N_VAL = 20000, 4000
IMG_SIZE  = 128
N_CLASSES = 100

print(f"[Keras heavy] Generating data ({N_TRAIN} train, {N_VAL} val, {IMG_SIZE}x{IMG_SIZE}, {N_CLASSES} classes)...")
x_train = np.random.randn(N_TRAIN, IMG_SIZE, IMG_SIZE, 3).astype("float32")
y_train = np.random.randint(0, N_CLASSES, N_TRAIN)
x_val   = np.random.randn(N_VAL,   IMG_SIZE, IMG_SIZE, 3).astype("float32")
y_val   = np.random.randint(0, N_CLASSES, N_VAL)


# Wide EfficientNet-style backbone with squeeze-excitation blocks
def se_block(x, filters, ratio=16):
    se = layers.GlobalAveragePooling2D()(x)
    se = layers.Reshape((1, 1, filters))(se)
    se = layers.Conv2D(max(1, filters // ratio), 1, activation="swish")(se)
    se = layers.Conv2D(filters, 1, activation="sigmoid")(se)
    return layers.Multiply()([x, se])

def mbconv_block(x, filters, strides=1, expand=6):
    in_ch = x.shape[-1]
    expanded = in_ch * expand
    # Expand
    x = layers.Conv2D(expanded, 1, use_bias=False)(x)
    x = layers.BatchNormalization()(x)
    x = layers.Activation("swish")(x)
    # Depthwise
    x = layers.DepthwiseConv2D(3, strides=strides, padding="same", use_bias=False)(x)
    x = layers.BatchNormalization()(x)
    x = layers.Activation("swish")(x)
    # SE
    x = se_block(x, expanded)
    # Project
    x = layers.Conv2D(filters, 1, use_bias=False)(x)
    x = layers.BatchNormalization()(x)
    return x

inputs = keras.Input(shape=(IMG_SIZE, IMG_SIZE, 3))
x = layers.Conv2D(40, 3, strides=2, padding="same", use_bias=False)(inputs)
x = layers.BatchNormalization()(x)
x = layers.Activation("swish")(x)

# Stage 1
for _ in range(2): x = mbconv_block(x, 24)
# Stage 2
x = mbconv_block(x, 48, strides=2)
for _ in range(3): x = mbconv_block(x, 48)
# Stage 3
x = mbconv_block(x, 96, strides=2)
for _ in range(3): x = mbconv_block(x, 96)
# Stage 4
x = mbconv_block(x, 192, strides=2)
for _ in range(4): x = mbconv_block(x, 192)
# Stage 5
x = mbconv_block(x, 320, strides=2)
for _ in range(1): x = mbconv_block(x, 320)

x = layers.Conv2D(1280, 1, use_bias=False)(x)
x = layers.BatchNormalization()(x)
x = layers.Activation("swish")(x)
x = layers.GlobalAveragePooling2D()(x)
x = layers.Dropout(0.3)(x)
outputs = layers.Dense(N_CLASSES, activation="softmax")(x)

model = keras.Model(inputs, outputs)
model.compile(
    optimizer=keras.optimizers.Adam(1e-3),
    loss="sparse_categorical_crossentropy",
    metrics=["accuracy"],
)
params = model.count_params() / 1e6
print(f"[Keras heavy] Model: {params:.2f}M params")

print(f"\n[Keras heavy] Starting training ({N_TRAIN} samples, 10 epochs)...")
with ecocode.run("keras-efficientnet-wide", exporter="terminal", interval=1.0) as eco_run:
    history = model.fit(
        x_train, y_train,
        epochs=10,
        batch_size=128,
        validation_data=(x_val, y_val),
        callbacks=[EcoCodeCallback(eco_run)],
        verbose=1,
    )
    final_loss = history.history["loss"][-1]
    final_acc  = history.history["accuracy"][-1]
    val_acc    = history.history["val_accuracy"][-1]
    print(f"\n[Keras heavy] PASS  run_id={eco_run._run_id}  epochs=10")
    print(f"  final_loss={final_loss:.4f}  train_acc={final_acc:.4f}  val_acc={val_acc:.4f}")
    if device == "cuda":
        print(f"  VRAM peak: {torch.cuda.max_memory_allocated()/1e9:.2f}GB")


