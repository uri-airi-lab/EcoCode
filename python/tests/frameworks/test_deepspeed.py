"""EcoCodeDeepSpeedCallback driving a real deepspeed engine (Linux only)."""
import os
import sys

import pytest

from ecocode.integrations.deepspeed import EcoCodeDeepSpeedCallback

if sys.platform != "linux":
    pytest.skip("DeepSpeed only builds on Linux", allow_module_level=True)
deepspeed = pytest.importorskip("deepspeed")


@pytest.fixture
def engine(torch, monkeypatch):
    monkeypatch.setenv("LOCAL_RANK", "0")
    monkeypatch.setenv("RANK", "0")
    monkeypatch.setenv("WORLD_SIZE", "1")
    monkeypatch.setenv("MASTER_ADDR", "127.0.0.1")
    monkeypatch.setenv("MASTER_PORT", "29517")
    model = torch.nn.Sequential(torch.nn.Linear(8, 16), torch.nn.ReLU(), torch.nn.Linear(16, 2))
    config = {
        "train_batch_size": 4,
        "train_micro_batch_size_per_gpu": 4,
        "gradient_accumulation_steps": 1,
        "optimizer": {"type": "Adam", "params": {"lr": 1e-3}},
        "zero_optimization": {"stage": 1},
        "fp16": {"enabled": False},
    }
    eng, _, _, _ = deepspeed.initialize(model=model, model_parameters=model.parameters(), config=config)
    return eng


def test_deepspeed_callback_real_engine(engine, torch, marker_tap, json_run, monkeypatch):
    # ``ecocode.run`` is a noop on non-zero ranks; this is rank 0.
    monkeypatch.delenv("RANK", raising=False)
    monkeypatch.delenv("LOCAL_RANK", raising=False)
    n_steps = 5
    losses = []
    with json_run("deepspeed") as run:
        cb = EcoCodeDeepSpeedCallback(run, zero_stage=1)
        cb.on_data_loading_begin()
        for step in range(n_steps):
            x = torch.randn(4, 8, device=engine.device)
            y = torch.randint(0, 2, (4,), device=engine.device)
            cb.on_step_begin(step)
            loss = torch.nn.functional.cross_entropy(engine(x), y)
            cb.on_backward_begin()
            engine.backward(loss)
            cb.on_backward_end()
            engine.step()
            losses.append(loss.item())
            cb.on_optimizer_step(step, loss.item())
        cb.on_train_end()

    phases = marker_tap.phases()
    assert phases[:2] == ["deepspeed_zero1_init", "data_loading"]
    assert phases[2:-1] == ["forward", "backward", "optimizer_step"] * n_steps
    assert phases[-1] == "done"
    assert marker_tap.values("step") == list(range(n_steps))
    assert marker_tap.values("loss") == losses
    assert json_run.last()["phase"] == "done"
