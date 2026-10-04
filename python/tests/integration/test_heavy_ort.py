"""
Heavy ONNX Runtime Integration Test
- Model:   ResNet-101 (torchvision, ~44M params) exported to ONNX
- Task:    Batched image classification inference
- Provider: CUDAExecutionProvider if available, else CPU
- Wrapper:  EcoCodeORTWrapper — phase markers, latency, batch_size, step count
"""
import os, time

import numpy as np
import torch
from torchvision.models import resnet101
import onnxruntime as ort
import ecocode
from ecocode.integrations.onnxruntime import EcoCodeORTWrapper

device = "cuda" if torch.cuda.is_available() else "cpu"
print(f"[ORT heavy] device={device}  onnxruntime={ort.__version__}")

# Export ResNet-101 to ONNX
ONNX_PATH = "/tmp/resnet101_heavy.onnx"
print(f"[ORT heavy] Exporting ResNet-101 to ONNX...")
model = resnet101(weights=None)
model.eval()
dummy = torch.randn(1, 3, 224, 224)
torch.onnx.export(
    model, dummy, ONNX_PATH,
    input_names=["image"],
    output_names=["logits"],
    dynamic_axes={"image": {0: "batch"}, "logits": {0: "batch"}},
    opset_version=17,
)
params = sum(p.numel() for p in model.parameters()) / 1e6
print(f"[ORT heavy] ResNet-101 exported ({params:.1f}M params)  →  {ONNX_PATH}")
del model

# Configure providers — prefer CUDA
providers = (
    ["CUDAExecutionProvider", "CPUExecutionProvider"]
    if device == "cuda" else
    ["CPUExecutionProvider"]
)
sess_opts = ort.SessionOptions()
sess_opts.graph_optimization_level = ort.GraphOptimizationLevel.ORT_ENABLE_ALL
sess_opts.intra_op_num_threads      = 8
sess_opts.enable_mem_pattern        = True

session = ort.InferenceSession(ONNX_PATH, sess_options=sess_opts, providers=providers)
active_provider = session.get_providers()[0]
print(f"[ORT heavy] Session provider: {active_provider}")

api_key  = os.environ.get("ECOCODE_API_KEY")
base_url = os.environ.get("ECOCODE_API_URL")  # None -> SDK uses its _DEFAULT_BASE_URL

N_REQUESTS  = 1000
BATCH_SIZES = [1, 4, 8, 16, 32, 64]

print(f"\n[ORT heavy] Running {N_REQUESTS} inference requests with mixed batch sizes...")
with ecocode.run("ort-resnet101-inference", exporter="online", api_key=api_key, base_url=base_url, interval=1.0) as eco_run:
    wrapper = EcoCodeORTWrapper(session, eco_run)

    latencies    = []
    total_images = 0

    for i in range(N_REQUESTS):
        batch_size = BATCH_SIZES[i % len(BATCH_SIZES)]
        images = np.random.randn(batch_size, 3, 224, 224).astype("float32")

        t0      = time.perf_counter()
        result  = wrapper.run(None, {"image": images})
        elapsed = (time.perf_counter() - t0) * 1000

        assert result[0].shape == (batch_size, 1000), f"Bad shape: {result[0].shape}"
        latencies.append(elapsed)
        total_images += batch_size

        if (i + 1) % 100 == 0:
            recent = latencies[-100:]
            print(f"  [{i+1:4d}/{N_REQUESTS}]  batch={batch_size}  latency={elapsed:.1f}ms  "
                  f"avg_last100={sum(recent)/len(recent):.1f}ms  images_so_far={total_images}")

    # Summary
    sorted_lat  = sorted(latencies)
    p50         = sorted_lat[len(latencies) // 2]
    p95         = sorted_lat[int(len(latencies) * 0.95)]
    p99         = sorted_lat[int(len(latencies) * 0.99)]
    throughput  = total_images / (sum(latencies) / 1000)

    print(f"\n[ORT heavy] PASS  run_id={eco_run._run_id}")
    print(f"  requests={N_REQUESTS}  total_images={total_images}")
    print(f"  latency p50={p50:.1f}ms  p95={p95:.1f}ms  p99={p99:.1f}ms")
    print(f"  throughput={throughput:.1f} img/s")
