"""
Heavy DeepSpeed Integration Test (Linux only)
- Model:  GPT-2 (124M params) wrapped with a real deepspeed.initialize()
- ZeRO:   Stage 3 on CUDA, stage 1 on CPU
- Callback: EcoCodeDeepSpeedCallback driven from the real engine loop
"""
import sys

import pytest

from ecocode.integrations.deepspeed import EcoCodeDeepSpeedCallback

if sys.platform != "linux":
    pytest.skip("DeepSpeed only builds on Linux", allow_module_level=True)
torch = pytest.importorskip("torch")
deepspeed = pytest.importorskip("deepspeed")
pytest.importorskip("transformers")

from torch.utils.data import DataLoader, TensorDataset  # noqa: E402
from transformers import AutoModelForCausalLM, AutoTokenizer  # noqa: E402


def test_heavy_deepspeed(device, scaled, heavy_run, marker_tap, monkeypatch):
    for k, v in {"MASTER_ADDR": "localhost", "MASTER_PORT": "29502", "WORLD_SIZE": "1"}.items():
        monkeypatch.setenv(k, v)
    tokenizer = AutoTokenizer.from_pretrained("gpt2")
    model = AutoModelForCausalLM.from_pretrained("gpt2")
    stage = 3 if device == "cuda" else 1
    ds_config = {
        "train_micro_batch_size_per_gpu": 4,
        "gradient_accumulation_steps": 8 if device == "cuda" else 1,
        "optimizer": {"type": "AdamW", "params": {"lr": 1e-5, "weight_decay": 0.01, "torch_adam": True}},
        "zero_optimization": {"stage": stage},
        "fp16": {"enabled": device == "cuda"},
        "steps_per_print": 1_000_000,
    }
    n, seq = scaled(2048, 16), 128 if device == "cuda" else 32
    ids = torch.randint(0, tokenizer.vocab_size, (n, seq))
    loader = DataLoader(TensorDataset(ids, ids.clone()), batch_size=4, shuffle=True)
    engine, _, _, _ = deepspeed.initialize(model=model, config=ds_config, model_parameters=model.parameters())

    # ``ecocode.run`` is a noop on non-zero ranks; make sure rank 0 is visible.
    monkeypatch.setenv("RANK", "0")
    monkeypatch.setenv("LOCAL_RANK", "0")
    losses, step = [], 0
    with heavy_run(f"deepspeed-gpt2-zero{stage}") as eco_run:
        cb = EcoCodeDeepSpeedCallback(eco_run, zero_stage=stage)
        cb.on_data_loading_begin()
        for _ in range(scaled(5, 1)):
            for x, y in loader:
                cb.on_step_begin(step)
                cb.on_forward_begin()
                loss = engine(input_ids=x.to(engine.device), labels=y.to(engine.device)).loss
                cb.on_backward_begin()
                engine.backward(loss)
                cb.on_backward_end()
                engine.step()
                cb.on_optimizer_step(step, loss.item())
                losses.append(loss.item())
                step += 1
        cb.on_train_end()

    phases = marker_tap.phases()
    assert phases[:2] == [f"deepspeed_zero{stage}_init", "data_loading"]
    assert phases[2:-1] == ["forward", "forward", "backward", "optimizer_step"] * step
    assert phases[-1] == "done"
    assert marker_tap.values("step") == list(range(step))
    assert marker_tap.values("loss") == losses
    if not heavy_run.online:
        assert heavy_run.last()["phase"] == "done"
