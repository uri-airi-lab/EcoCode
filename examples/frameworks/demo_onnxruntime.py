"""
EcoCode ONNX Runtime Inference Demo

Exports a tiny PyTorch model to ONNX, then runs batched inference with
EcoCode energy tracking via the EcoCodeORTWrapper.

Requirements:
    pip install torch onnxruntime numpy ecocode
"""
import os
import time
import numpy as np
import ecocode
from ecocode.integrations.onnxruntime import EcoCodeORTWrapper

ONNX_PATH = os.path.join(os.path.dirname(__file__), "_demo_model.onnx")


def export_dummy_model():
    """Export a small PyTorch linear model to ONNX."""
    import torch
    import torch.nn as nn

    class TinyModel(nn.Module):
        def __init__(self):
            super().__init__()
            self.net = nn.Sequential(
                nn.Linear(128, 256),
                nn.ReLU(),
                nn.Linear(256, 128),
                nn.ReLU(),
                nn.Linear(128, 10),
            )

        def forward(self, x):
            return self.net(x)

    model = TinyModel()
    dummy = torch.randn(1, 128)
    torch.onnx.export(
        model, dummy, ONNX_PATH,
        input_names=["input"],
        output_names=["output"],
        dynamic_axes={"input": {0: "batch"}, "output": {0: "batch"}},
    )
    print(f"   Exported ONNX model to {ONNX_PATH}")


def run_ort_demo():
    import onnxruntime as ort

    api_key = os.environ.get("ECOCODE_API_KEY")
    run_name = "ort-inference-demo"

    # 1. Export model if needed
    print("\n🔧 Preparing ONNX model...")
    if not os.path.exists(ONNX_PATH):
        export_dummy_model()
    else:
        print(f"   Using existing {ONNX_PATH}")

    # 2. Create ORT session
    print("📦 Loading ONNX Runtime session...")
    session = ort.InferenceSession(ONNX_PATH)

    # 3. Run inference with EcoCode tracking
    num_batches = 200
    batch_size = 32

    print(f"\n🚀 Running {num_batches} inference batches (batch_size={batch_size})")

    with ecocode.run(run_name, exporter="online", api_key=api_key, interval=0.5) as eco_run:
        eco_session = EcoCodeORTWrapper(session, eco_run)

        total_samples = 0
        t_start = time.perf_counter()

        for i in range(num_batches):
            input_data = np.random.randn(batch_size, 128).astype(np.float32)
            result = eco_session.run(None, {"input": input_data})
            total_samples += batch_size

            if (i + 1) % 50 == 0:
                elapsed = time.perf_counter() - t_start
                throughput = total_samples / elapsed
                print(f"   Batch {i+1}/{num_batches} — {throughput:.0f} samples/s")

            # Small sleep to simulate realistic inference cadence
            time.sleep(0.01)

        elapsed = time.perf_counter() - t_start
        print(f"\n✅ Inference complete!")
        print(f"   {total_samples} samples in {elapsed:.1f}s ({total_samples/elapsed:.0f} samples/s)")
        print(f"   View energy metrics at: http://localhost:3001")

    # Cleanup
    if os.path.exists(ONNX_PATH):
        os.remove(ONNX_PATH)


if __name__ == "__main__":
    run_ort_demo()
