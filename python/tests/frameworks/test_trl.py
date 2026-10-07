"""TRL callbacks through real SFTTrainer, DPOTrainer and GRPOTrainer on CPU."""
import pytest

from ecocode.integrations.trl import EcoCodeDPOCallback, EcoCodeGRPOCallback, EcoCodeSFTCallback

trl = pytest.importorskip("trl", minversion="0.15")
pytest.importorskip("datasets")

from datasets import Dataset  # noqa: E402
from trl import DPOConfig, DPOTrainer, GRPOConfig, GRPOTrainer, SFTConfig, SFTTrainer  # noqa: E402

COMMON = dict(
    per_device_train_batch_size=2,
    learning_rate=1e-3,
    lr_scheduler_type="constant",
    logging_steps=1,
    save_strategy="no",
    report_to="none",
    use_cpu=True,
    dataloader_num_workers=0,
    disable_tqdm=True,
)


def _check_training_stream(tap, n_steps, update_phase, first_phase="forward_backward"):
    phases = tap.phases()
    assert phases[0] == "data_loading"
    assert phases[-1] == "done"
    per_step = [p for p in phases if p in (first_phase, update_phase)]
    assert per_step == [first_phase, update_phase] * n_steps
    # on_optimizer_step fires before global_step is incremented: 0-based indices.
    assert tap.values("step") == list(range(n_steps))


def _losses_match(tap, trainer):
    trainer_losses = [h["loss"] for h in trainer.state.log_history if "loss" in h]
    sent = tap.values("loss")
    assert sent, "callback never logged a loss"
    assert all(any(abs(s - t) < 1e-9 for t in trainer_losses) for s in sent)


def test_sft_trainer_callback(tokenizer, tiny_gpt2, corpus, tmp_path, marker_tap, json_run):
    n_steps = 8
    ds = Dataset.from_dict({"text": corpus})
    with json_run("trl-sft") as run:
        args = SFTConfig(output_dir=str(tmp_path / "sft"), max_steps=n_steps, max_length=32, **COMMON)
        trainer = SFTTrainer(model=tiny_gpt2, args=args, train_dataset=ds, processing_class=tokenizer,
                             callbacks=[EcoCodeSFTCallback(run)])
        trainer.train()

    assert trainer.state.global_step == n_steps
    _check_training_stream(marker_tap, n_steps, "optimizer_step")
    assert marker_tap.values("seq_len") == [32]
    _losses_match(marker_tap, trainer)
    assert json_run.last()["phase"] == "done"
    assert json_run.max_of("training_step") == n_steps - 1
    assert json_run.last()["sequence_length"] == 32


def test_dpo_trainer_callback(tokenizer, tiny_gpt2, tiny_gpt2_factory, corpus, tmp_path, marker_tap, json_run):
    n_steps = 6
    ds = Dataset.from_dict({
        "prompt": corpus[:16],
        "chosen": corpus[16:32],
        "rejected": corpus[32:48],
    })
    with json_run("trl-dpo") as run:
        args = DPOConfig(output_dir=str(tmp_path / "dpo"), max_steps=n_steps, max_length=32,
                         beta=0.1, **COMMON)
        # An in-memory model has no hub name for TRL to clone, so pass a real reference model.
        trainer = DPOTrainer(model=tiny_gpt2, ref_model=tiny_gpt2_factory(), args=args, train_dataset=ds,
                             processing_class=tokenizer, callbacks=[EcoCodeDPOCallback(run)])
        trainer.train()

    assert trainer.state.global_step == n_steps
    _check_training_stream(marker_tap, n_steps, "dpo_update")
    assert marker_tap.values("seq_len") == [32]
    _losses_match(marker_tap, trainer)
    assert json_run.last()["phase"] == "done"
    assert json_run.max_of("training_step") == n_steps - 1


def test_grpo_trainer_callback(tokenizer, tiny_gpt2, corpus, tmp_path, marker_tap, json_run):
    n_steps = 4
    max_completion = 8
    ds = Dataset.from_dict({"prompt": corpus[:16]})

    def reward_len(completions, **kwargs):
        return [float(len(c)) for c in completions]

    with json_run("trl-grpo") as run:
        args = GRPOConfig(output_dir=str(tmp_path / "grpo"), max_steps=n_steps,
                          max_completion_length=max_completion,
                          num_generations=2, **COMMON)
        trainer = GRPOTrainer(model=tiny_gpt2, reward_funcs=reward_len, args=args, train_dataset=ds,
                              processing_class=tokenizer, callbacks=[EcoCodeGRPOCallback(run)])
        trainer.train()

    assert trainer.state.global_step == n_steps
    _check_training_stream(marker_tap, n_steps, "grpo_update", first_phase="policy_rollout")
    seq = getattr(args, "max_length", None) or getattr(args, "max_seq_length", None) or max_completion
    assert marker_tap.values("seq_len") == [int(seq)]
    _losses_match(marker_tap, trainer)
    assert json_run.last()["phase"] == "done"
    assert json_run.max_of("training_step") == n_steps - 1
