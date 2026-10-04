"""
Heavy PyTorch Lightning Integration Test
- Model:  ResNet-50 (torchvision, ~25M params)
- Task:   10-class image classification on synthetic CIFAR-like data
- Device: CUDA
- Callback: EcoCodeCallback
"""
import os, torch, torch.nn as nn

import pytorch_lightning as pl
from torch.utils.data import DataLoader, TensorDataset
from torchvision.models import resnet50
import ecocode
from ecocode.integrations.lightning import EcoCodeCallback

device = "cuda" if torch.cuda.is_available() else "cpu"
print(f"[Lightning heavy] device={device}")

N_TRAIN, N_VAL = 20000, 4000
IMG_SIZE  = 224
N_CLASSES = 10

print(f"[Lightning heavy] Generating synthetic data ({N_TRAIN} train, {N_VAL} val, {IMG_SIZE}x{IMG_SIZE})...")
x_train = torch.randn(N_TRAIN, 3, IMG_SIZE, IMG_SIZE)
y_train = torch.randint(0, N_CLASSES, (N_TRAIN,))
x_val   = torch.randn(N_VAL,   3, IMG_SIZE, IMG_SIZE)
y_val   = torch.randint(0, N_CLASSES, (N_VAL,))

train_loader = DataLoader(TensorDataset(x_train, y_train), batch_size=128, shuffle=True,  num_workers=8, pin_memory=True)
val_loader   = DataLoader(TensorDataset(x_val,   y_val),   batch_size=128, shuffle=False, num_workers=8, pin_memory=True)


class ResNet50Module(pl.LightningModule):
    def __init__(self, n_classes=10, lr=1e-2):
        super().__init__()
        self.save_hyperparameters()
        self.model   = resnet50(weights=None)
        self.model.fc = nn.Linear(self.model.fc.in_features, n_classes)
        self.loss_fn  = nn.CrossEntropyLoss()

    def forward(self, x): return self.model(x)

    def training_step(self, batch, batch_idx):
        x, y = batch
        logits = self(x)
        loss = self.loss_fn(logits, y)
        acc  = (logits.argmax(1) == y).float().mean()
        self.log("train_loss", loss, on_step=True, on_epoch=True, prog_bar=True)
        self.log("train_acc",  acc,  on_step=False, on_epoch=True)
        return loss

    def validation_step(self, batch, batch_idx):
        x, y = batch
        logits = self(x)
        loss = self.loss_fn(logits, y)
        acc  = (logits.argmax(1) == y).float().mean()
        self.log("val_loss", loss, prog_bar=True)
        self.log("val_acc",  acc,  prog_bar=True)

    def configure_optimizers(self):
        opt   = torch.optim.SGD(self.parameters(), lr=self.hparams.lr, momentum=0.9, weight_decay=1e-4)
        sched = torch.optim.lr_scheduler.OneCycleLR(
            opt, max_lr=self.hparams.lr,
            steps_per_epoch=len(train_loader), epochs=10,
        )
        return {"optimizer": opt, "lr_scheduler": {"scheduler": sched, "interval": "step"}}


json_path = "test_heavy_lightning.json"
if os.path.exists(json_path):
    os.remove(json_path)

print(f"\n[Lightning heavy] Starting ResNet-50 training...")
with ecocode.run("lightning-resnet50", exporter="online", interval=1.0) as eco_run:
    pl_model = ResNet50Module(n_classes=N_CLASSES, lr=1e-2)
    params   = sum(p.numel() for p in pl_model.parameters()) / 1e6
    print(f"[Lightning heavy] ResNet-50: {params:.1f}M params")

    trainer = pl.Trainer(
        max_epochs=10,
        accelerator="gpu" if device == "cuda" else "cpu",
        devices=1,
        callbacks=[EcoCodeCallback(eco_run)],
        enable_progress_bar=True,
        enable_model_summary=False,
        logger=False,
        precision="16-mixed" if device == "cuda" else "32",
        gradient_clip_val=1.0,
    )
    trainer.fit(pl_model, train_loader, val_loader)

    val_result = trainer.validate(pl_model, val_loader, verbose=False)
    val_acc    = val_result[0].get("val_acc", 0.0)
    print(f"\n[Lightning heavy] PASS  run_id={eco_run._run_id}  global_step={trainer.global_step}  val_acc={val_acc:.4f}")
    if device == "cuda":
        print(f"  VRAM peak: {torch.cuda.max_memory_allocated()/1e9:.2f}GB")

if os.path.exists(json_path):
    os.remove(json_path)
