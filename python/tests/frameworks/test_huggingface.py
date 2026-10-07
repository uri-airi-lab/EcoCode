"""EcoCodeCallback (HuggingFace) through a real transformers.Trainer on CPU."""
import pytest

from ecocode.integrations.huggingface import EcoCodeCallback

transformers = pytest.importorskip("transformers")
pytest.importorskip("datasets")

from datasets import Dataset  # noqa: E402
from transformers import DataCollatorForLanguageModeling, Trainer, TrainingArguments  # noqa: E402

MAX_STEPS = 12
EVAL_EVERY = 6
SEQ = 16


def tokenized(tokenizer, corpus):
    ds = Dataset.from_dict({"text": corpus})
    return ds.map(lambda b: tokenizer(b["text"], truncation=True, max_length=SEQ, padding="max_length"),
                  batched=True, remove_columns=["text"])


def test_hf_trainer_callback(tokenizer, tiny_gpt2, corpus, tmp_path, marker_tap, json_run):
    ds = tokenized(tokenizer, corpus)
    with json_run("hf") as run:
        args = TrainingArguments(
            output_dir=str(tmp_path / "out"),
            max_steps=MAX_STEPS,
            per_device_train_batch_size=2,
            per_device_eval_batch_size=2,
            gradient_accumulation_steps=2,
            learning_rate=1e-3,
            lr_scheduler_type="constant",
            logging_steps=1,
            eval_strategy="steps",
            eval_steps=EVAL_EVERY,
            save_strategy="steps",
            save_steps=EVAL_EVERY,
            save_total_limit=1,
            include_num_input_tokens_seen=True,
            report_to="none",
            use_cpu=True,
            dataloader_num_workers=0,
            disable_tqdm=True,
        )
        trainer = Trainer(
            model=tiny_gpt2,
            args=args,
            train_dataset=ds,
            eval_dataset=ds.select(range(4)),
            data_collator=DataCollatorForLanguageModeling(tokenizer=tokenizer, mlm=False),
            callbacks=[EcoCodeCallback(run)],
        )
        trainer.train()

    state = trainer.state
    assert state.global_step == MAX_STEPS

    phases = marker_tap.phases()
    assert phases[0] == "data_loading"
    assert phases[-1] == "done"
    per_step = [p for p in phases if p in ("forward_backward", "optimizer_step")]
    assert per_step == ["forward_backward", "optimizer_step"] * MAX_STEPS
    assert phases.count("eval") == MAX_STEPS // EVAL_EVERY
    assert phases.count("checkpoint_save") == MAX_STEPS // EVAL_EVERY

    # Trainer fires on_optimizer_step before incrementing global_step, so the
    # step the callback reports is the 0-based index of the step just taken.
    assert marker_tap.values("step") == list(range(MAX_STEPS))
    # on_optimizer_step reads the *latest* log_history entry. With logging_steps=1
    # that is the previous step's training log, so the losses the callback sent
    # must all be losses the trainer itself logged.
    trainer_losses = [h["loss"] for h in state.log_history if "loss" in h]
    sent = marker_tap.values("loss")
    assert sent and all(any(abs(s - t) < 1e-9 for t in trainer_losses) for s in sent)
    assert set(marker_tap.values("lr")) <= {1e-3}
    assert marker_tap.values("tokens") == [state.num_input_tokens_seen]
    assert state.num_input_tokens_seen > 0

    assert json_run.last()["phase"] == "done"
    assert json_run.last()["tokens_processed"] == state.num_input_tokens_seen
    assert json_run.max_of("training_step") == MAX_STEPS - 1
    assert set(json_run.phases()) <= {
        "data_loading", "forward_backward", "optimizer_step", "eval", "checkpoint_save", "done",
    }
