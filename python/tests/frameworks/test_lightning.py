"""EcoCodeCallback (Lightning) through a real pytorch_lightning.Trainer on CPU."""
import pytest

from ecocode.integrations.lightning import EcoCodeCallback

pl = pytest.importorskip("pytorch_lightning")

LR = 0.05
TRAIN_BATCHES = 4
VAL_BATCHES = 2
EPOCHS = 2


def _module(torch, log_name="train_loss"):
    from torch.utils.data import DataLoader, TensorDataset

    class Mlp(pl.LightningModule):
        def __init__(self):
            super().__init__()
            self.net = torch.nn.Sequential(torch.nn.Linear(8, 16), torch.nn.ReLU(), torch.nn.Linear(16, 2))
            self.step_losses = []

        def training_step(self, batch, batch_idx):
            x, y = batch
            loss = torch.nn.functional.cross_entropy(self.net(x), y)
            self.log(log_name, loss, on_step=True, on_epoch=False, prog_bar=False)
            self.step_losses.append(loss.item())
            return loss

        def validation_step(self, batch, batch_idx):
            x, y = batch
            self.log("val_loss", torch.nn.functional.cross_entropy(self.net(x), y))

        def configure_optimizers(self):
            return torch.optim.SGD(self.parameters(), lr=LR)

    x = torch.randn(64, 8)
    y = torch.randint(0, 2, (64,))
    train = DataLoader(TensorDataset(x, y), batch_size=8)
    val = DataLoader(TensorDataset(x[:16], y[:16]), batch_size=8)
    return Mlp(), train, val


def test_lightning_callback_real_trainer(torch, tmp_path, marker_tap, json_run):
    model, train, val = _module(torch)
    with json_run("lightning") as run:
        trainer = pl.Trainer(
            max_epochs=EPOCHS,
            limit_train_batches=TRAIN_BATCHES,
            limit_val_batches=VAL_BATCHES,
            accelerator="cpu",
            devices=1,
            logger=False,
            enable_checkpointing=True,
            enable_progress_bar=False,
            enable_model_summary=False,
            default_root_dir=str(tmp_path),
            callbacks=[EcoCodeCallback(run)],
            num_sanity_val_steps=0,
        )
        trainer.fit(model, train, val)

    n_steps = trainer.global_step
    assert n_steps == EPOCHS * TRAIN_BATCHES

    phases = marker_tap.phases()
    assert phases[0] == "data_loading"
    assert phases[-1] == "done"
    # Each optimisation step is forward → backward → optimizer_step, in that order.
    per_batch = [p for p in phases if p in ("forward", "backward", "optimizer_step")]
    assert per_batch == ["forward", "backward", "optimizer_step"] * n_steps
    assert phases.count("eval") == EPOCHS  # one validation epoch per training epoch
    assert phases.count("checkpoint_save") >= 1

    assert marker_tap.values("epoch") == list(range(EPOCHS))
    assert marker_tap.values("step") == list(range(n_steps))  # global_step before the optimizer step
    assert marker_tap.values("lr") == [LR] * n_steps
    # The loss logged to EcoCode is exactly the loss the module logged as train_loss.
    assert marker_tap.values("loss") == pytest.approx(model.step_losses, rel=1e-6)

    assert json_run.last()["phase"] == "done"
    assert set(json_run.phases()) <= {
        "data_loading", "forward", "backward", "optimizer_step", "eval", "checkpoint_save", "done",
    }
    assert json_run.max_of("training_step") == n_steps - 1
    assert json_run.last()["learning_rate"] == LR


def test_lightning_loss_without_train_loss_metric(torch, tmp_path, marker_tap, json_run):
    """A module that logs its loss under another name still gets it recorded,
    unscaled by gradient accumulation: the last micro-batch's training_step loss."""
    accumulate = 2
    model, train, _ = _module(torch, log_name="loss")
    with json_run("lightning-loss") as run:
        trainer = pl.Trainer(
            max_epochs=1,
            limit_train_batches=TRAIN_BATCHES,
            accumulate_grad_batches=accumulate,
            accelerator="cpu",
            devices=1,
            logger=False,
            enable_checkpointing=False,
            enable_progress_bar=False,
            enable_model_summary=False,
            default_root_dir=str(tmp_path),
            callbacks=[EcoCodeCallback(run)],
        )
        trainer.fit(model, train)

    assert trainer.global_step == TRAIN_BATCHES // accumulate
    expected = model.step_losses[accumulate - 1::accumulate]
    assert marker_tap.values("loss") == pytest.approx(expected, rel=1e-6)
    assert json_run.last()["loss"] == pytest.approx(expected[-1], rel=1e-6)
