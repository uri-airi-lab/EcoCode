"""
EcoCode SDK — PyTorch ResNet-18 on CIFAR-10.

A realistic image classification training run:
  - ResNet-18 (~11.2M parameters, built from scratch)
  - CIFAR-10 dataset (50k train / 10k val, auto-downloaded ~170 MB)
  - AdamW + cosine LR with linear warmup
  - Mixed-precision training (AMP) when CUDA is available
  - Gradient clipping, label smoothing, random erasing augmentation
  - Best-checkpoint saving

Setup
-----
1. Start the EcoCode backend:
       cd web && docker compose up

2. Generate a project-scoped API key:
       Dashboard → Settings → Developer → select project → Generate Key

3. Install dependencies:
       pip install -e ".[gpu]"
       pip install torch torchvision --index-url https://download.pytorch.org/whl/cu121

4. Run:
       API_KEY=ecocode_xxx python demo_training.py

   Tune with env vars:
       EPOCHS=30 BATCH_SIZE=256 LR=3e-4 API_KEY=ecocode_xxx python demo_training.py

What to expect on the dashboard
--------------------------------
  Phases   : data_loading → warmup → forward → backward →
             optimizer_step → eval → checkpoint_save → done
  Loss     : drops from ~2.3 → ~0.4 over 20 epochs
  Accuracy : ~70-75% val after 20 epochs on CPU; ~92%+ with GPU + more epochs
  LR       : linear warmup for 5 steps, then cosine decay to 1e-6
  Tokens   : cumulative samples processed (reaches ~1M for 20 epochs)
  GPU energy: real NVML joules per second if CUDA is available
"""

import os
import sys
import time
import math

import torch
import torch.nn as nn
import torch.nn.functional as F
import torch.optim as optim
from torch.amp import GradScaler, autocast
from torch.optim.lr_scheduler import LambdaLR
from torchvision import datasets, transforms
from torch.utils.data import DataLoader

# ── CONFIG ────────────────────────────────────────────────────────────────────
API_KEY     = os.environ.get("ECOCODE_API_KEY") or os.environ.get("API_KEY")
BASE_URL    = os.environ.get("ECOCODE_API_URL", "https://backend.blacktree-5d3c8de6.eastus.azurecontainerapps.io")
EPOCHS      = int(os.environ.get("EPOCHS", 20))
BATCH_SIZE  = int(os.environ.get("BATCH_SIZE", 128))
LR          = float(os.environ.get("LR", 1e-3))
WEIGHT_DECAY= float(os.environ.get("WEIGHT_DECAY", 1e-4))
WARMUP_EPOCHS = int(os.environ.get("WARMUP_EPOCHS", 2))
GRAD_CLIP   = float(os.environ.get("GRAD_CLIP", 1.0))
LABEL_SMOOTH= float(os.environ.get("LABEL_SMOOTH", 0.1))
EVAL_EVERY  = int(os.environ.get("EVAL_EVERY", 2))
DATA_DIR    = os.environ.get("DATA_DIR", "/tmp/cifar10")
CKPT_PATH   = os.environ.get("CKPT_PATH", "/tmp/resnet18_cifar10_best.pt")
# ─────────────────────────────────────────────────────────────────────────────

CIFAR10_CLASSES = [
    "airplane", "automobile", "bird", "cat", "deer",
    "dog", "frog", "horse", "ship", "truck",
]


def check_config():
    if not API_KEY:
        print(
            "\n[error] Missing API key.\n"
            "  Generate a project-scoped key from the dashboard:\n"
            "  Settings → Developer → select a project → Generate Key\n\n"
            "  Then run:\n"
            "    API_KEY=ecocode_xxx python demo_training.py\n"
        )
        sys.exit(1)


# ── Model: ResNet-18 for CIFAR-10 ─────────────────────────────────────────────
#
# Standard ResNet-18 adapted for 32×32 inputs:
#   - No 7×7 stem or initial MaxPool (would collapse 32×32 too quickly)
#   - 3×3 stem conv instead, stride=1
#   - 4 residual stages: 64 → 128 → 256 → 512 channels
#   - Global average pool → FC(512, 10)
#   ~11.2M parameters

class ResidualBlock(nn.Module):
    """BasicBlock with optional downsampling shortcut."""

    def __init__(self, in_ch: int, out_ch: int, stride: int = 1) -> None:
        super().__init__()
        self.conv1 = nn.Conv2d(in_ch, out_ch, 3, stride=stride, padding=1, bias=False)
        self.bn1   = nn.BatchNorm2d(out_ch)
        self.conv2 = nn.Conv2d(out_ch, out_ch, 3, padding=1, bias=False)
        self.bn2   = nn.BatchNorm2d(out_ch)

        self.shortcut = nn.Sequential()
        if stride != 1 or in_ch != out_ch:
            self.shortcut = nn.Sequential(
                nn.Conv2d(in_ch, out_ch, 1, stride=stride, bias=False),
                nn.BatchNorm2d(out_ch),
            )

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        out = F.relu(self.bn1(self.conv1(x)), inplace=True)
        out = self.bn2(self.conv2(out))
        out = F.relu(out + self.shortcut(x), inplace=True)
        return out


class ResNet18(nn.Module):
    """ResNet-18 for CIFAR-10 (32×32 input, 10 classes)."""

    def __init__(self, num_classes: int = 10) -> None:
        super().__init__()
        self._in_ch = 64

        # Stem: single 3×3 conv, no maxpool
        self.stem = nn.Sequential(
            nn.Conv2d(3, 64, 3, padding=1, bias=False),
            nn.BatchNorm2d(64),
            nn.ReLU(inplace=True),
        )

        # Residual stages — [2, 2, 2, 2] blocks per stage
        self.layer1 = self._make_stage(64,  2, stride=1)
        self.layer2 = self._make_stage(128, 2, stride=2)
        self.layer3 = self._make_stage(256, 2, stride=2)
        self.layer4 = self._make_stage(512, 2, stride=2)

        self.pool = nn.AdaptiveAvgPool2d(1)
        self.head = nn.Linear(512, num_classes)

        self._init_weights()

    def _make_stage(self, out_ch: int, n_blocks: int, stride: int) -> nn.Sequential:
        layers = [ResidualBlock(self._in_ch, out_ch, stride)]
        self._in_ch = out_ch
        for _ in range(n_blocks - 1):
            layers.append(ResidualBlock(out_ch, out_ch))
        return nn.Sequential(*layers)

    def _init_weights(self) -> None:
        for m in self.modules():
            if isinstance(m, nn.Conv2d):
                nn.init.kaiming_normal_(m.weight, mode="fan_out", nonlinearity="relu")
            elif isinstance(m, nn.BatchNorm2d):
                nn.init.ones_(m.weight)
                nn.init.zeros_(m.bias)
            elif isinstance(m, nn.Linear):
                nn.init.normal_(m.weight, 0, 0.01)
                nn.init.zeros_(m.bias)

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        x = self.stem(x)
        x = self.layer1(x)
        x = self.layer2(x)
        x = self.layer3(x)
        x = self.layer4(x)
        x = self.pool(x).flatten(1)
        return self.head(x)


# ── LR schedule: linear warmup + cosine decay ─────────────────────────────────

def build_scheduler(optimizer, warmup_epochs: int, total_epochs: int, steps_per_epoch: int):
    warmup_steps = warmup_epochs * steps_per_epoch
    total_steps  = total_epochs  * steps_per_epoch

    def lr_lambda(step: int) -> float:
        if step < warmup_steps:
            return step / max(warmup_steps, 1)
        progress = (step - warmup_steps) / max(total_steps - warmup_steps, 1)
        return max(1e-6 / LR, 0.5 * (1.0 + math.cos(math.pi * progress)))

    return LambdaLR(optimizer, lr_lambda)


# ── Helpers ───────────────────────────────────────────────────────────────────

def grad_norm(model: nn.Module) -> float:
    total = 0.0
    for p in model.parameters():
        if p.grad is not None:
            total += p.grad.detach().norm(2).item() ** 2
    return math.sqrt(total)


def format_time(seconds: float) -> str:
    m, s = divmod(int(seconds), 60)
    return f"{m}m{s:02d}s" if m else f"{s}s"


# ── Main ──────────────────────────────────────────────────────────────────────

def main():
    check_config()

    import ecocode

    device   = torch.device("cuda" if torch.cuda.is_available() else "cpu")
    use_amp  = device.type == "cuda"
    scaler   = GradScaler('cuda', enabled=use_amp)

    print("=" * 65)
    print("  EcoCode SDK — ResNet-18 on CIFAR-10")
    print(f"  device     : {device}")
    if device.type == "cuda":
        print(f"  GPU        : {torch.cuda.get_device_name(0)}")
    print(f"  epochs     : {EPOCHS}  |  batch : {BATCH_SIZE}  |  lr : {LR}")
    print(f"  weight_decay: {WEIGHT_DECAY}  |  amp : {use_amp}  |  grad_clip : {GRAD_CLIP}")
    print("=" * 65)

    with ecocode.run(
        "resnet18-cifar10",
        api_key=API_KEY,
        base_url=BASE_URL,
        interval=1.0,
    ) as eco:

        # ── Data loading ──────────────────────────────────────────────────────
        print(f"\n⏱ Run started! You have 20 seconds to open the dashboard...")
        print(f"URL: {BASE_URL}")
        time.sleep(20) # WARMUP

        eco.mark("phase", "data_loading")
        print("\n[phase] data_loading")

        train_tfm = transforms.Compose([
            transforms.RandomCrop(32, padding=4),
            transforms.RandomHorizontalFlip(),
            transforms.ColorJitter(brightness=0.2, contrast=0.2, saturation=0.2),
            transforms.ToTensor(),
            transforms.Normalize((0.4914, 0.4822, 0.4465), (0.2470, 0.2435, 0.2616)),
            transforms.RandomErasing(p=0.25, scale=(0.02, 0.2)),
        ])
        val_tfm = transforms.Compose([
            transforms.ToTensor(),
            transforms.Normalize((0.4914, 0.4822, 0.4465), (0.2470, 0.2435, 0.2616)),
        ])

        try:
            train_ds = datasets.CIFAR10(DATA_DIR, train=True,  download=True, transform=train_tfm)
            val_ds   = datasets.CIFAR10(DATA_DIR, train=False, download=True, transform=val_tfm)
        except Exception as e:
            print(f"  [warn] CIFAR-10 download failed ({e}).")
            print("  [warn] Falling back to FashionMNIST (28×28 grayscale, same 10 classes).")
            # FashionMNIST needs 32×32 for ResNet — pad with transforms
            fm_tfm_train = transforms.Compose([
                transforms.Pad(2),                          # 28 → 32
                transforms.Grayscale(num_output_channels=3),# 1ch → 3ch
                transforms.RandomHorizontalFlip(),
                transforms.ToTensor(),
                transforms.Normalize((0.5,), (0.5,)),
                transforms.RandomErasing(p=0.25, scale=(0.02, 0.2)),
            ])
            fm_tfm_val = transforms.Compose([
                transforms.Pad(2),
                transforms.Grayscale(num_output_channels=3),
                transforms.ToTensor(),
                transforms.Normalize((0.5,), (0.5,)),
            ])
            train_ds = datasets.FashionMNIST(DATA_DIR, train=True,  download=True, transform=fm_tfm_train)
            val_ds   = datasets.FashionMNIST(DATA_DIR, train=False, download=True, transform=fm_tfm_val)

        n_workers = min(4, os.cpu_count() or 1)
        train_loader = DataLoader(
            train_ds, batch_size=BATCH_SIZE, shuffle=True,
            num_workers=n_workers, pin_memory=(device.type == "cuda"),
            persistent_workers=(n_workers > 0),
        )
        val_loader = DataLoader(
            val_ds, batch_size=512, shuffle=False,
            num_workers=n_workers, pin_memory=(device.type == "cuda"),
            persistent_workers=(n_workers > 0),
        )

        steps_per_epoch = len(train_loader)
        print(f"  train: {len(train_ds):,} samples  ({steps_per_epoch} steps/epoch)")
        print(f"  val  : {len(val_ds):,} samples")
        print(f"  classes: {', '.join(CIFAR10_CLASSES)}")

        # ── Build model ───────────────────────────────────────────────────────
        model     = ResNet18(num_classes=10).to(device)
        criterion = nn.CrossEntropyLoss(label_smoothing=LABEL_SMOOTH)
        optimizer = optim.AdamW(model.parameters(), lr=LR, weight_decay=WEIGHT_DECAY)
        scheduler = build_scheduler(optimizer, WARMUP_EPOCHS, EPOCHS, steps_per_epoch)

        n_params = sum(p.numel() for p in model.parameters())
        print(f"  parameters: {n_params:,}  (~{n_params/1e6:.1f}M)")

        # Image = 3 × 32 × 32 = 3072 "features" per sample
        eco.log_sequence_length(3 * 32 * 32)

        global_step  = 0
        tokens_seen  = 0
        best_val_acc = 0.0

        # ── Training loop ─────────────────────────────────────────────────────
        for epoch in range(EPOCHS):
            model.train()
            eco.log_epoch(epoch)

            epoch_loss    = 0.0
            epoch_correct = 0
            epoch_total   = 0
            epoch_gnorm   = 0.0
            t0 = time.time()

            is_warmup = epoch < WARMUP_EPOCHS
            if is_warmup:
                eco.mark("phase", "warmup")

            for batch_idx, (images, labels) in enumerate(train_loader):
                images, labels = images.to(device, non_blocking=True), labels.to(device, non_blocking=True)

                # ── forward ───────────────────────────────────────────────────
                eco.mark("phase", "forward")
                optimizer.zero_grad(set_to_none=True)

                with autocast('cuda', enabled=use_amp):
                    logits = model(images)
                    loss   = criterion(logits, labels)

                # ── backward ──────────────────────────────────────────────────
                eco.mark("phase", "backward")
                scaler.scale(loss).backward()

                scaler.unscale_(optimizer)
                gnorm = grad_norm(model)
                nn.utils.clip_grad_norm_(model.parameters(), GRAD_CLIP)

                # ── optimizer step ────────────────────────────────────────────
                eco.mark("phase", "optimizer_step")
                scaler.step(optimizer)
                scaler.update()
                scheduler.step()

                global_step += 1
                tokens_seen += images.size(0)
                current_lr   = scheduler.get_last_lr()[0]

                batch_correct = (logits.detach().argmax(1) == labels).sum().item()
                epoch_loss    += loss.item() * images.size(0)
                epoch_correct += batch_correct
                epoch_total   += images.size(0)
                epoch_gnorm   += gnorm

                eco.log_step(global_step)
                eco.log_loss(loss.item())
                eco.log_lr(current_lr)
                eco.log_tokens(tokens_seen)
                eco.log_accuracy(100 * epoch_correct / epoch_total)

                # In-epoch progress every ~20% of batches
                if (batch_idx + 1) % max(1, steps_per_epoch // 5) == 0:
                    pct  = 100 * (batch_idx + 1) / steps_per_epoch
                    acc  = 100 * batch_correct / images.size(0)
                    print(
                        f"    [{pct:5.1f}%]  loss={loss.item():.4f}"
                        f"  acc={acc:.1f}%  gnorm={gnorm:.3f}  lr={current_lr:.2e}"
                    )

            elapsed   = time.time() - t0
            avg_loss  = epoch_loss / epoch_total
            train_acc = 100 * epoch_correct / epoch_total
            avg_gnorm = epoch_gnorm / steps_per_epoch

            print(
                f"\n  epoch {epoch+1:02d}/{EPOCHS}"
                f"  loss={avg_loss:.4f}"
                f"  acc={train_acc:.1f}%"
                f"  gnorm={avg_gnorm:.3f}"
                f"  lr={current_lr:.2e}"
                f"  {format_time(elapsed)}"
            )

            # ── Eval ──────────────────────────────────────────────────────────
            if (epoch + 1) % EVAL_EVERY == 0 or epoch == EPOCHS - 1:
                eco.mark("phase", "eval")
                model.eval()

                val_loss    = 0.0
                val_correct = 0
                val_total   = 0
                per_class   = torch.zeros(10)
                per_class_n = torch.zeros(10)

                with torch.no_grad():
                    for images, labels in val_loader:
                        images, labels = images.to(device, non_blocking=True), labels.to(device, non_blocking=True)
                        with autocast('cuda', enabled=use_amp):
                            logits = model(images)
                        val_loss    += criterion(logits, labels).item() * images.size(0)
                        preds        = logits.argmax(1)
                        val_correct += (preds == labels).sum().item()
                        val_total   += images.size(0)
                        for c in range(10):
                            mask = labels == c
                            per_class[c]   += (preds[mask] == labels[mask]).sum().item()
                            per_class_n[c] += mask.sum().item()

                val_loss /= val_total
                val_acc   = 100 * val_correct / val_total
                eco.log_accuracy(val_acc)

                # Per-class accuracy
                class_accs = (per_class / per_class_n.clamp(min=1) * 100)
                worst_cls  = class_accs.argmin().item()
                best_cls   = class_accs.argmax().item()

                print(
                    f"         [eval]  val_loss={val_loss:.4f}  val_acc={val_acc:.1f}%"
                    f"  best={CIFAR10_CLASSES[best_cls]}({class_accs[best_cls]:.0f}%)"
                    f"  worst={CIFAR10_CLASSES[worst_cls]}({class_accs[worst_cls]:.0f}%)"
                )

                # Save best checkpoint
                if val_acc > best_val_acc:
                    best_val_acc = val_acc
                    eco.mark("phase", "checkpoint_save")
                    torch.save({
                        "epoch": epoch + 1,
                        "model": model.state_dict(),
                        "optimizer": optimizer.state_dict(),
                        "val_acc": val_acc,
                        "step": global_step,
                    }, CKPT_PATH)
                    print(f"         [checkpoint]  new best → {val_acc:.1f}%  saved to {CKPT_PATH}")

                model.train()

        # ── Done ──────────────────────────────────────────────────────────────
        eco.mark("phase", "done")
        print("\n" + "=" * 65)
        print(f"  Training complete")
        print(f"  Best val accuracy : {best_val_acc:.1f}%")
        print(f"  Total steps       : {global_step:,}")
        print(f"  Total tokens      : {tokens_seen:,}")
        print(f"  Checkpoint        : {CKPT_PATH}")
        print("=" * 65)

    print(f"\nView energy breakdown at: {BASE_URL}")


if __name__ == "__main__":
    main()
