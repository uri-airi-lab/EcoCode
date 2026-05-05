"""
EcoCode Keras / TensorFlow Demo — MNIST CNN

Trains a small CNN on the MNIST handwritten-digit dataset while tracking
energy consumption through EcoCode.

Requirements:
    pip install tensorflow ecocode
"""
import os
import ecocode
from ecocode.integrations.keras import EcoCodeCallback

# Suppress TF info logs
os.environ.setdefault("TF_CPP_MIN_LOG_LEVEL", "2")


def run_keras_demo():
    import tensorflow as tf
    from tensorflow import keras

    model_name = "keras-mnist-cnn"
    api_key = "ecocode_SvB6ETEjqwdJVjd6x2QkfdBZqXObta0F"

    print("\n🚀 Loading MNIST dataset...")
    (x_train, y_train), (x_test, y_test) = keras.datasets.mnist.load_data()
    x_train = x_train.reshape(-1, 28, 28, 1).astype("float32") / 255.0
    x_test = x_test.reshape(-1, 28, 28, 1).astype("float32") / 255.0
    # Use a subset for a quick demo
    x_train, y_train = x_train[:5000], y_train[:5000]
    x_test, y_test = x_test[:1000], y_test[:1000]

    print("🏗️  Building CNN model...")
    model = keras.Sequential([
        keras.layers.Conv2D(32, 3, activation="relu", input_shape=(28, 28, 1)),
        keras.layers.MaxPooling2D(2),
        keras.layers.Conv2D(64, 3, activation="relu"),
        keras.layers.MaxPooling2D(2),
        keras.layers.Flatten(),
        keras.layers.Dense(128, activation="relu"),
        keras.layers.Dropout(0.5),
        keras.layers.Dense(10, activation="softmax"),
    ])

    model.compile(
        optimizer="adam",
        loss="sparse_categorical_crossentropy",
        metrics=["accuracy"],
    )

    print(f"📊 Model has {model.count_params():,} parameters\n")

    with ecocode.run(model_name, api_key=api_key, interval=0.1) as eco_run:
        print("⚡ Training with EcoCode energy tracking...\n")
        model.fit(
            x_train, y_train,
            validation_data=(x_test, y_test),
            epochs=5,
            batch_size=64,
            callbacks=[EcoCodeCallback(eco_run)],
            verbose=1,
        )

        print("\n✅ Training complete!")
        print(f"View energy metrics at: http://localhost:3001")


if __name__ == "__main__":
    run_keras_demo()
