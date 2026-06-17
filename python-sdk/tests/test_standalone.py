"""
Test: EcoCode SDK — Standalone Mode
Trains a ResNet-18 on CIFAR-10 (real PyTorch training, CPU-heavy).
The SDK spawns `ecocode monitor` as a subprocess and sends markers via pipe.

Usage:
    export ECOCODE_API_KEY=eco_...
    python test_standalone.py
"""

import os
import sys
import time

# Point SDK at local backend

import torch
import torch.nn as nn
import torch.optim as optim
from torch.utils.data import DataLoader, TensorDataset

import ecocode


def build_resnet_block(in_ch, out_ch, stride=1):
    """Residual block with two 3x3 convolutions."""
    downsample = None
    if stride != 1 or in_ch != out_ch:
        downsample = nn.Sequential(
            nn.Conv2d(in_ch, out_ch, 1, stride=stride, bias=False),
            nn.BatchNorm2d(out_ch),
        )
    return ResBlock(in_ch, out_ch, stride, downsample)


class ResBlock(nn.Module):
    def __init__(self, in_ch, out_ch, stride, downsample):
        super().__init__()
        self.conv1 = nn.Conv2d(in_ch, out_ch, 3, stride=stride, padding=1, bias=False)
        self.bn1 = nn.BatchNorm2d(out_ch)
        self.conv2 = nn.Conv2d(out_ch, out_ch, 3, padding=1, bias=False)
        self.bn2 = nn.BatchNorm2d(out_ch)
        self.relu = nn.ReLU(inplace=True)
        self.downsample = downsample

    def forward(self, x):
        identity = x
        out = self.relu(self.bn1(self.conv1(x)))
        out = self.bn2(self.conv2(out))
        if self.downsample:
            identity = self.downsample(x)
        out += identity
        return self.relu(out)


class MiniResNet(nn.Module):
    """Simplified ResNet with ~2.8M parameters — enough to stress CPU."""

    def __init__(self, num_classes=10):
        super().__init__()
        self.prep = nn.Sequential(
            nn.Conv2d(3, 64, 3, padding=1, bias=False),
            nn.BatchNorm2d(64),
            nn.ReLU(inplace=True),
        )
        self.layer1 = nn.Sequential(
            build_resnet_block(64, 64),
            build_resnet_block(64, 64),
        )
        self.layer2 = nn.Sequential(
            build_resnet_block(64, 128, stride=2),
            build_resnet_block(128, 128),
        )
        self.layer3 = nn.Sequential(
            build_resnet_block(128, 256, stride=2),
            build_resnet_block(256, 256),
        )
        self.pool = nn.AdaptiveAvgPool2d(1)
        self.fc = nn.Linear(256, num_classes)

    def forward(self, x):
        x = self.prep(x)
        x = self.layer1(x)
        x = self.layer2(x)
        x = self.layer3(x)
        x = self.pool(x).flatten(1)
        return self.fc(x)


def generate_fake_cifar(n_samples=2000):
    """Generate random tensors shaped like CIFAR-10 to avoid download issues."""
    images = torch.randn(n_samples, 3, 32, 32)
    labels = torch.randint(0, 10, (n_samples,))
    return TensorDataset(images, labels)


def main():
    print(f"[test] PyTorch: {torch.__version__}")
    print(f"[test] Device:  CPU")
    print()

    # --- Model setup ---
    model = MiniResNet(num_classes=10)
    param_count = sum(p.numel() for p in model.parameters())
    print(f"[test] Model: MiniResNet ({param_count:,} params)")

    criterion = nn.CrossEntropyLoss()
    optimizer = optim.SGD(model.parameters(), lr=0.01, momentum=0.9, weight_decay=1e-4)
    scheduler = optim.lr_scheduler.StepLR(optimizer, step_size=2, gamma=0.5)

    # --- Data ---
    train_dataset = generate_fake_cifar(n_samples=3000)
    val_dataset = generate_fake_cifar(n_samples=500)
    train_loader = DataLoader(train_dataset, batch_size=64, shuffle=True, num_workers=2)
    val_loader = DataLoader(val_dataset, batch_size=128, shuffle=False)

    EPOCHS = 4
    global_step = 0
    total_tokens = 0  # simulated token count

    # =============================================
    # STANDALONE MODE: SDK spawns monitor subprocess
    # =============================================
    print("\n" + "=" * 60)
    print("  STANDALONE LOCAL MODE TEST")
    print("=" * 60 + "\n")

    csv_path = "test_standalone.csv"
    if os.path.exists(csv_path):
        os.remove(csv_path)

    with ecocode.run(
        "resnet-cifar10-standalone-test",
        exporter="online",
        interval=1.0,
    ) as run:

        print(f"[ecocode] Run started (mode should be standalone_local, exporting to CSV)")
        print(f"[ecocode] Run ID: {run._run_id}")
        print(f"[ecocode] Mode:   {run._mode}")
        print()

        run.mark("phase", "data_loading")
        time.sleep(0.5)  # simulate data prep

        for epoch in range(EPOCHS):
            run.log_epoch(epoch)
            model.train()
            epoch_loss = 0.0
            correct = 0
            total = 0

            run.mark("phase", "training")

            for batch_idx, (images, labels) in enumerate(train_loader):
                run.mark("phase", "forward")
                outputs = model(images)
                loss = criterion(outputs, labels)

                run.mark("phase", "backward")
                optimizer.zero_grad()
                loss.backward()

                run.mark("phase", "optimizer_step")
                optimizer.step()

                global_step += 1
                batch_tokens = images.size(0) * 32 * 32  # pixels as "tokens"
                total_tokens += batch_tokens

                # Log all markers
                run.log_step(global_step)
                run.log_loss(loss.item())
                run.log_lr(scheduler.get_last_lr()[0])
                run.log_tokens(total_tokens)
                run.log_sequence_length(32 * 32)

                epoch_loss += loss.item()
                _, predicted = outputs.max(1)
                total += labels.size(0)
                correct += predicted.eq(labels).sum().item()

                if batch_idx % 10 == 0:
                    print(
                        f"  Epoch {epoch+1}/{EPOCHS} "
                        f"[{batch_idx:>3d}/{len(train_loader)}] "
                        f"loss={loss.item():.4f} "
                        f"acc={100.*correct/total:.1f}% "
                        f"step={global_step}"
                    )

            scheduler.step()
            train_acc = 100.0 * correct / total
            avg_loss = epoch_loss / len(train_loader)

            # --- Validation ---
            run.mark("phase", "eval")
            model.eval()
            val_correct = 0
            val_total = 0
            with torch.no_grad():
                for images, labels in val_loader:
                    outputs = model(images)
                    _, predicted = outputs.max(1)
                    val_total += labels.size(0)
                    val_correct += predicted.eq(labels).sum().item()

            val_acc = 100.0 * val_correct / val_total
            run.log_accuracy(val_acc)
            run.log_custom("train_acc", train_acc)
            run.log_custom("avg_epoch_loss", avg_loss)

            print(
                f"\n  Epoch {epoch+1} done — "
                f"train_loss={avg_loss:.4f} train_acc={train_acc:.1f}% "
                f"val_acc={val_acc:.1f}%\n"
            )

        run.mark("phase", "done")
        print(f"[ecocode] Training complete. {global_step} steps, {total_tokens:,} tokens")

    # Clean up test CSV
    if os.path.exists(csv_path):
        os.remove(csv_path)

    print("\n[test] Standalone local mode test PASSED — run completed cleanly.")


if __name__ == "__main__":
    main()
