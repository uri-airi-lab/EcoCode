"""
Heavy ONNX Runtime Integration Test
- Model:   ResNet-101 (torchvision, ~44M params, random init) exported to ONNX
- Task:    Batched image classification inference with mixed batch sizes
- Provider: CUDAExecutionProvider if available, else CPU
- Wrapper:  EcoCodeORTWrapper, phase markers, latency, batch_size, step count
"""
import numpy as np
import pytest

from ecocode.integrations.onnxruntime import EcoCodeORTWrapper

torch = pytest.importorskip("torch")
ort = pytest.importorskip("onnxruntime")
torchvision = pytest.importorskip("torchvision")

from torchvision.models import resnet101  # noqa: E402

BATCH_SIZES = [1, 4, 8, 16, 32, 64]


def test_heavy_ort(device, scaled, tmp_path, heavy_run, marker_tap):
    img = 224 if device == "cuda" else 64
    onnx_path = tmp_path / "resnet101.onnx"
    model = resnet101(weights=None).eval()
    torch.onnx.export(model, torch.randn(1, 3, img, img), str(onnx_path), input_names=["image"],
                      output_names=["logits"], dynamic_axes={"image": {0: "batch"}, "logits": {0: "batch"}},
                      opset_version=17, dynamo=False)
    del model

    providers = ["CUDAExecutionProvider", "CPUExecutionProvider"] if device == "cuda" else ["CPUExecutionProvider"]
    opts = ort.SessionOptions()
    opts.graph_optimization_level = ort.GraphOptimizationLevel.ORT_ENABLE_ALL
    session = ort.InferenceSession(str(onnx_path), sess_options=opts, providers=providers)
    print(f"[ORT heavy] provider={session.get_providers()[0]} onnxruntime={ort.__version__}")

    n_requests = scaled(1000, 12)
    sizes = [BATCH_SIZES[i % len(BATCH_SIZES)] if device == "cuda" else BATCH_SIZES[i % 3] for i in range(n_requests)]
    with heavy_run("ort-resnet101-inference") as eco_run:
        wrapper = EcoCodeORTWrapper(session, eco_run)
        for b in sizes:
            images = np.random.randn(b, 3, img, img).astype("float32")
            result = wrapper.run(None, {"image": images})
            assert result[0].shape == (b, 1000)

    assert marker_tap.phases()[0] == "model_load"
    # Image batches never have a sequence axis of 1, so every call is prefill.
    assert marker_tap.phases()[1:] == ["prefill"] * n_requests
    assert marker_tap.values("step") == list(range(1, n_requests + 1))
    assert marker_tap.customs("batch_size") == [float(b) for b in sizes]
    assert len(marker_tap.customs("latency_ms")) == n_requests and min(marker_tap.customs("latency_ms")) > 0
    if not heavy_run.online:
        assert heavy_run.max_of("training_step") == n_requests
        assert {"batch_size", "latency_ms"} <= heavy_run.custom_keys()
