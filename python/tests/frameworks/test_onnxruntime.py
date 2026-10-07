"""EcoCodeORTWrapper around a real onnxruntime.InferenceSession."""
import numpy as np
import pytest

from ecocode.integrations.onnxruntime import EcoCodeORTWrapper

ort = pytest.importorskip("onnxruntime")


@pytest.fixture
def session(torch, tmp_path):
    """Export a real 2-layer MLP to ONNX with dynamic batch and sequence axes."""
    model = torch.nn.Sequential(torch.nn.Linear(8, 16), torch.nn.ReLU(), torch.nn.Linear(16, 4)).eval()
    path = tmp_path / "mlp.onnx"
    torch.onnx.export(
        model,
        torch.randn(2, 3, 8),
        str(path),
        input_names=["x"],
        output_names=["y"],
        dynamic_axes={"x": {0: "batch", 1: "seq"}, "y": {0: "batch", 1: "seq"}},
        dynamo=False,
    )
    return ort.InferenceSession(str(path), providers=["CPUExecutionProvider"])


def test_ort_wrapper_marks_phases_and_metrics(session, marker_tap, json_run, burn):
    calls = [
        np.random.randn(4, 8, 8).astype("float32"),  # first call: prefill (batch 4, seq 8)
        np.random.randn(4, 1, 8).astype("float32"),  # seq axis == 1 → decode
        np.random.randn(1, 1, 8).astype("float32"),  # decode
        np.random.randn(2, 8, 8).astype("float32"),  # back to prefill
    ]
    with json_run("ort") as run:
        wrapper = EcoCodeORTWrapper(session, run)
        assert marker_tap.phases() == ["model_load"]

        for x in calls:
            out = wrapper.run(None, {"x": x})
            direct = session.run(None, {"x": x})
            np.testing.assert_allclose(out[0], direct[0])
            assert out[0].shape == (x.shape[0], x.shape[1], 4)
        # __getattr__ proxies to the underlying session.
        assert [i.name for i in wrapper.get_inputs()] == [i.name for i in session.get_inputs()]
        burn(0.4)

    assert marker_tap.phases() == ["model_load", "prefill", "decode", "decode", "prefill"]
    assert marker_tap.values("step") == [1, 2, 3, 4]
    assert marker_tap.customs("batch_size") == [4.0, 4.0, 1.0, 2.0]
    latencies = marker_tap.customs("latency_ms")
    assert len(latencies) == 4 and all(l > 0 for l in latencies)

    # What the monitor persisted.
    recs = json_run.records
    assert recs, "monitor wrote no records"
    assert set(json_run.phases()) <= {"model_load", "prefill", "decode"}
    assert json_run.max_of("training_step") == 4
    assert {"batch_size", "latency_ms"} <= json_run.custom_keys()
    assert json_run.last()["custom_metrics"]["batch_size"] == 2.0
