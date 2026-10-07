"""
Heavy HuggingFace Integration Test
- Model:  GPT-2 (124M params), downloaded from the Hub
- Task:   causal LM fine-tuning on synthetic instruction data
- Device: CUDA if available, else CPU (scaled down)
- Callback: EcoCodeCallback (phases, loss, lr, step, tokens)
"""
import pytest

from ecocode.integrations.huggingface import EcoCodeCallback

torch = pytest.importorskip("torch")
transformers = pytest.importorskip("transformers")
pytest.importorskip("datasets")

from datasets import Dataset  # noqa: E402
from transformers import (  # noqa: E402
    AutoModelForCausalLM,
    AutoTokenizer,
    DataCollatorForLanguageModeling,
    Trainer,
    TrainingArguments,
)

TOPICS = [
    "the theory of relativity", "attention mechanisms in transformers", "backpropagation through time",
    "reinforcement learning from human feedback", "mixture of experts architecture",
    "rotary positional embeddings", "grouped query attention", "speculative decoding for LLM inference",
    "constitutional AI alignment", "retrieval-augmented generation", "tokenizer vocabulary design",
    "low-rank adaptation fine-tuning", "knowledge distillation", "contrastive learning objectives",
    "diffusion model score matching", "neural architecture search", "gradient checkpointing",
    "pipeline parallelism", "tensor parallelism", "flash attention",
]


def test_heavy_hf(device, scale, scaled, tmp_path, heavy_run, marker_tap):
    print(f"[HF heavy] device={device} scale={scale}")
    tokenizer = AutoTokenizer.from_pretrained("gpt2")
    tokenizer.pad_token = tokenizer.eos_token
    model = AutoModelForCausalLM.from_pretrained(
        "gpt2", torch_dtype=torch.bfloat16 if device == "cuda" else torch.float32
    ).to(device)

    texts = [
        f"Explain {t} in depth, covering the mathematical formulation, intuition, key advantages, "
        f"practical limitations, and recent research directions. Include concrete examples where helpful."
        for t in TOPICS
    ] * 10
    ds = Dataset.from_dict({"text": texts[: scaled(200, 8)]})
    ds = ds.map(lambda b: tokenizer(b["text"], truncation=True, max_length=128, padding="max_length"),
                batched=True, remove_columns=["text"]).with_format("torch")

    epochs = scaled(10, 1)
    with heavy_run("hf-gpt2-finetune") as eco_run:
        args = TrainingArguments(
            output_dir=str(tmp_path / "hf"),
            num_train_epochs=epochs,
            per_device_train_batch_size=2,
            gradient_accumulation_steps=2,
            learning_rate=2e-5,
            warmup_steps=scaled(20, 1),
            lr_scheduler_type="cosine",
            logging_steps=1,
            save_strategy="no",
            report_to="none",
            bf16=(device == "cuda"),
            dataloader_num_workers=4 if device == "cuda" else 0,
            gradient_checkpointing=True,
            include_num_input_tokens_seen=True,
            use_cpu=(device == "cpu"),
            disable_tqdm=True,
        )
        trainer = Trainer(model=model, args=args, train_dataset=ds,
                          data_collator=DataCollatorForLanguageModeling(tokenizer=tokenizer, mlm=False),
                          callbacks=[EcoCodeCallback(eco_run)])
        result = trainer.train()
    n_steps = trainer.state.global_step
    print(f"[HF heavy] steps={n_steps} loss={result.training_loss:.4f}")

    phases = marker_tap.phases()
    assert phases[0] == "data_loading" and phases[-1] == "done"
    assert phases.count("optimizer_step") == n_steps
    assert marker_tap.values("step") == list(range(n_steps))
    assert marker_tap.values("tokens") == [trainer.state.num_input_tokens_seen]
    trainer_losses = [h["loss"] for h in trainer.state.log_history if "loss" in h]
    assert all(any(abs(s - t) < 1e-9 for t in trainer_losses) for s in marker_tap.values("loss"))
    if not heavy_run.online:
        assert heavy_run.last()["phase"] == "done"
        assert heavy_run.max_of("training_step") == n_steps - 1
        assert heavy_run.last()["tokens_processed"] == trainer.state.num_input_tokens_seen
