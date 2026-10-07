"""
Heavy PyTorch Lightning Integration Test
- Model:  ResNet-50 (torchvision, ~25M params, random init)
- Task:   10-class image classification on synthetic CIFAR-like data
- Device: CUDA if available, else CPU (scaled down)
- Callback: EcoCodeCallback
"""
import pytest

from ecocode.integrations.lightning import EcoCodeCallback

torch = pytest.importorskip("torch")
pl = pytest.importorskip("pytorch_lightning")
torchvision = pytest.importorskip("torchvision")

from torch import nn  # noqa: E402
from torch.utils.data import DataLoader, TensorDataset  # noqa: E402
from torchvision.models import resnet50  # noqa: E402

N_CLASSES = 10


class ResNet50Module(pl.LightningModule):
    def __init__(self, steps_per_epoch, epochs, lr=1e-2):
        super().__init__()
        self.save_hyperparameters()
        self.model = resnet50(weights=None)
        self.model.fc = nn.Linear(self.model.fc.in_features, N_CLASSES)
        self.loss_fn = nn.CrossEntropyLoss()
        self.step_losses = []

    def forward(self, x):
        return self.model(x)

    def training_step(self, batch, batch_idx):
        x, y = batch
        loss = self.loss_fn(self(x), y)
        self.log("train_loss", loss, on_step=True, on_epoch=False)
        self.step_losses.append(loss.item())
        return loss

    def validation_step(self, batch, batch_idx):
        x, y = batch
        logits = self(x)
        self.log("val_loss", self.loss_fn(logits, y))
        self.log("val_acc", (logits.argmax(1) == y).float().mean())

    def configure_optimizers(self):
        opt = torch.optim.SGD(self.parameters(), lr=self.hparams.lr, momentum=0.9, weight_decay=1e-4)
        sched = torch.optim.lr_scheduler.OneCycleLR(
            opt, max_lr=self.hparams.lr, steps_per_epoch=self.hparams.steps_per_epoch, epochs=self.hparams.epochs
        )
        return {"optimizer": opt, "lr_scheduler": {"scheduler": sched, "interval": "step"}}


def test_heavy_lightning(device, scaled, tmp_path, heavy_run, marker_tap):
    img = 224 if device == "cuda" else 64
    n_train, n_val, batch = scaled(20000, 64), scaled(4000, 16), 128 if device == "cuda" else 16
    epochs = scaled(10, 2)
    x_train, y_train = torch.randn(n_train, 3, img, img), torch.randint(0, N_CLASSES, (n_train,))
    x_val, y_val = torch.randn(n_val, 3, img, img), torch.randint(0, N_CLASSES, (n_val,))
    workers = 8 if device == "cuda" else 0
    train_loader = DataLoader(TensorDataset(x_train, y_train), batch_size=batch, shuffle=True,
                              num_workers=workers, pin_memory=(device == "cuda"))
    val_loader = DataLoader(TensorDataset(x_val, y_val), batch_size=batch, num_workers=workers)

    with heavy_run("lightning-resnet50") as eco_run:
        model = ResNet50Module(steps_per_epoch=len(train_loader), epochs=epochs)
        trainer = pl.Trainer(
            max_epochs=epochs,
            accelerator="gpu" if device == "cuda" else "cpu",
            devices=1,
            callbacks=[EcoCodeCallback(eco_run)],
            enable_progress_bar=False,
            enable_model_summary=False,
            logger=False,
            default_root_dir=str(tmp_path),
            precision="16-mixed" if device == "cuda" else "32",
            gradient_clip_val=1.0,
            num_sanity_val_steps=0,
        )
        trainer.fit(model, train_loader, val_loader)

    n = trainer.global_step
    assert n == epochs * len(train_loader)
    phases = marker_tap.phases()
    assert phases[0] == "data_loading" and phases[-1] == "done"
    per_batch = [p for p in phases if p in ("forward", "backward", "optimizer_step")]
    assert per_batch == ["forward", "backward", "optimizer_step"] * n
    assert phases.count("eval") == epochs
    assert phases.count("checkpoint_save") >= 1
    assert marker_tap.values("epoch") == list(range(epochs))
    assert marker_tap.values("step") == list(range(n))
    assert marker_tap.values("loss") == pytest.approx(model.step_losses, rel=1e-5)
    assert len(marker_tap.values("lr")) == n and all(lr > 0 for lr in marker_tap.values("lr"))
    if not heavy_run.online:
        assert heavy_run.last()["phase"] == "done"
        assert heavy_run.max_of("training_step") == n - 1
