"""
Heavy TRL Integration Test
- SFT:  GPT-2 fine-tuned with a real SFTTrainer
- DPO:  GPT-2 policy vs frozen reference with a real DPOTrainer
- GRPO: GPT-2 policy with a real GRPOTrainer and a length reward
- Device: CUDA if available, else CPU (scaled down)
"""
import pytest

from ecocode.integrations.trl import EcoCodeDPOCallback, EcoCodeGRPOCallback, EcoCodeSFTCallback

torch = pytest.importorskip("torch")
pytest.importorskip("transformers")
pytest.importorskip("trl", minversion="0.15")
pytest.importorskip("datasets")

from datasets import Dataset  # noqa: E402
from transformers import AutoModelForCausalLM, AutoTokenizer  # noqa: E402
from trl import DPOConfig, DPOTrainer, GRPOConfig, GRPOTrainer, SFTConfig, SFTTrainer  # noqa: E402

TOPICS = [
    "backpropagation", "attention mechanisms", "dropout regularization", "batch normalization",
    "gradient descent", "cross-entropy loss", "softmax activation", "embedding layers", "tokenization",
    "fine-tuning", "transfer learning", "overfitting", "regularization", "activation functions",
    "residual connections", "layer normalization", "weight decay", "learning rate scheduling",
    "training epochs", "mini-batch training", "knowledge distillation", "model pruning", "quantization",
    "speculative decoding", "retrieval augmented generation", "constitutional AI", "RLHF",
    "mixture of experts", "flash attention", "low-rank adaptation",
]


@pytest.fixture
def gpt2(device):
    tok = AutoTokenizer.from_pretrained("gpt2")
    tok.pad_token = tok.eos_token

    def load():
        m = AutoModelForCausalLM.from_pretrained(
            "gpt2", torch_dtype=torch.bfloat16 if device == "cuda" else torch.float32
        ).to(device)
        m.config.use_cache = False
        return m

    return tok, load


def _common(device, tmp_path, name):
    return dict(
        output_dir=str(tmp_path / name),
        per_device_train_batch_size=2,
        logging_steps=1,
        save_strategy="no",
        report_to="none",
        bf16=(device == "cuda"),
        use_cpu=(device == "cpu"),
        dataloader_num_workers=0,
        disable_tqdm=True,
    )


def _assert_stream(tap, trainer, update_phase, first_phase="forward_backward"):
    n = trainer.state.global_step
    assert n > 0
    phases = tap.phases()
    assert phases[0] == "data_loading" and phases[-1] == "done"
    per_step = [p for p in phases if p in (first_phase, update_phase)]
    assert per_step == [first_phase, update_phase] * n
    assert tap.values("step") == list(range(n))
    trainer_losses = [h["loss"] for h in trainer.state.log_history if "loss" in h]
    assert all(any(abs(s - t) < 1e-9 for t in trainer_losses) for s in tap.values("loss"))
    return n


def test_heavy_trl_sft(device, scaled, gpt2, tmp_path, heavy_run, marker_tap):
    tok, load = gpt2
    texts = [
        f"Question: Can you give a thorough technical explanation of {t}? Answer: {t} is a critical "
        f"technique in modern deep learning. It works by optimizing the loss landscape through carefully "
        f"designed mathematical operations that allow gradients to flow efficiently."
        for t in TOPICS
    ] * 4
    ds = Dataset.from_dict({"text": texts[: scaled(120, 8)]})
    with heavy_run("trl-sft-gpt2") as eco_run:
        args = SFTConfig(num_train_epochs=scaled(3, 1), gradient_accumulation_steps=2, learning_rate=1e-5,
                         warmup_steps=scaled(10, 1), max_length=128, gradient_checkpointing=True,
                         **_common(device, tmp_path, "sft"))
        trainer = SFTTrainer(model=load(), args=args, train_dataset=ds, processing_class=tok,
                             callbacks=[EcoCodeSFTCallback(eco_run)])
        trainer.train()
    n = _assert_stream(marker_tap, trainer, "optimizer_step")
    assert marker_tap.values("seq_len") == [128]
    if not heavy_run.online:
        assert heavy_run.last()["phase"] == "done" and heavy_run.max_of("training_step") == n - 1


def test_heavy_trl_dpo(device, scaled, gpt2, tmp_path, heavy_run, marker_tap):
    tok, load = gpt2
    n_pairs = scaled(120, 8)
    ds = Dataset.from_dict({
        "prompt": [f"Explain {TOPICS[i % len(TOPICS)]}." for i in range(n_pairs)],
        "chosen": [f"{TOPICS[i % len(TOPICS)]} is a well-studied technique with clear benefits." for i in range(n_pairs)],
        "rejected": ["I don't know." for _ in range(n_pairs)],
    })
    with heavy_run("trl-dpo-gpt2") as eco_run:
        args = DPOConfig(num_train_epochs=scaled(3, 1), learning_rate=5e-6, max_length=128,
                         beta=0.1, **_common(device, tmp_path, "dpo"))
        trainer = DPOTrainer(model=load(), ref_model=None, args=args, train_dataset=ds,
                             processing_class=tok, callbacks=[EcoCodeDPOCallback(eco_run)])
        trainer.train()
    n = _assert_stream(marker_tap, trainer, "dpo_update")
    assert marker_tap.values("seq_len") == [128]
    if not heavy_run.online:
        assert heavy_run.last()["phase"] == "done" and heavy_run.max_of("training_step") == n - 1


def test_heavy_trl_grpo(device, scaled, gpt2, tmp_path, heavy_run, marker_tap):
    tok, load = gpt2
    ds = Dataset.from_dict({"prompt": [f"Explain {t}." for t in TOPICS] * 2})

    def reward_unique_tokens(completions, **kwargs):
        return [len(set(c.split())) / max(1, len(c.split())) for c in completions]

    with heavy_run("trl-grpo-gpt2") as eco_run:
        args = GRPOConfig(max_steps=scaled(60, 2), learning_rate=5e-6,
                          max_completion_length=64, num_generations=2, **_common(device, tmp_path, "grpo"))
        trainer = GRPOTrainer(model=load(), reward_funcs=reward_unique_tokens, args=args, train_dataset=ds,
                              processing_class=tok, callbacks=[EcoCodeGRPOCallback(eco_run)])
        trainer.train()
    n = _assert_stream(marker_tap, trainer, "grpo_update", first_phase="policy_rollout")
    assert len(marker_tap.values("seq_len")) == 1
    if not heavy_run.online:
        assert heavy_run.last()["phase"] == "done" and heavy_run.max_of("training_step") == n - 1
