"""
EcoCode SDK — Apple CoreNet training demo (Linux).

Trains a MobileOne-S0 image classifier on synthetic ImageNet-shaped data.
Uses CoreNet's published architecture (Apple CVPR 2023) implemented in
vanilla PyTorch, so no dataset download or external dependency is required.

NOTE: The `corenet` PyPI package (v0.0.0) is a name-squatting stub with no
code.  The real CoreNet must be installed from Apple's GitHub repo:

    git clone https://github.com/apple/corenet
    cd corenet && pip install -e ".[dev]"

If you already have CoreNet installed from source, you can monitor a real
corenet-train job directly with the EcoCode CLI:

    ecocode run --name corenet-job \\
        corenet-train --common.config_file configs/your_config.yaml

This script is designed to reproduce and verify the fix for the bug where
EcoCode prematurely marked CoreNet jobs as "finished" after ~1 second.

--- What caused the bug ---
The monitor had a secondary idle-detection guard:

    if cpu_usage <= 0.0 and total_gpu_util <= 0.0 and total_gpu_energy_j <= 0.0:
        break  # "Process finished"

On Linux without an NVIDIA GPU, total_gpu_util and total_gpu_energy_j are
always 0 (NVML is NVIDIA-only).  On the first 1-second tick, the root
process may also report cpu_usage=0 because sysinfo needs two consecutive
process snapshots to compute a delta — and if corenet-train dispatches work
to DataLoader workers or a distributed launcher before the first tick, the
root process is momentarily idle.  All three conditions hit zero together,
triggering the early exit.

The fix: the secondary guard was removed entirely.  Process termination is
detected reliably by polling the PID in the OS process table (if the PID is
gone, the process is done).

--- Setup ---
1. Start the EcoCode backend:
       cd web && docker compose up

2. Generate a project-scoped API key:
       Dashboard → Settings → Developer → select project → Generate Key

3. Install SDK:
       pip install -e "."

4. Run via EcoCode CLI (tests the fix end-to-end):
       ecocode run --name corenet-demo python python-sdk/demo_corenet.py

   Or with the Python SDK:
       API_KEY=ecocode_xxx python python-sdk/demo_corenet.py
"""

import os
import sys
import time
import math

import torch
import torch.nn as nn
import torch.nn.functional as F

# ── Config ────────────────────────────────────────────────────────────────────
API_KEY    = os.environ.get("ECOCODE_API_KEY") or os.environ.get("API_KEY")
BASE_URL   = os.environ.get("ECOCODE_API_URL", "http://localhost:3001")
EPOCHS     = int(os.environ.get("EPOCHS", 5))
BATCH_SIZE = int(os.environ.get("BATCH_SIZE", 64))
LR         = float(os.environ.get("LR", 1e-3))
N_CLASSES  = 1000
IMG_SIZE   = 224
N_BATCHES  = int(os.environ.get("N_BATCHES", 80))  # synthetic batches per epoch
# ─────────────────────────────────────────────────────────────────────────────


# ── CoreNet architecture: MobileOne-S0 ───────────────────────────────────────
#
# Direct implementation of Apple's MobileOne-S0 (CVPR 2023) in vanilla
# PyTorch.  Matches channel counts and block structure from the paper.
# The real CoreNet training framework wraps this in re-parameterisable
# branches during training; this is the equivalent single-branch form.

class ConvBnAct(nn.Module):
    def __init__(self, in_ch, out_ch, kernel=3, stride=1, groups=1):
        super().__init__()
        self.conv = nn.Conv2d(in_ch, out_ch, kernel, stride=stride,
                              padding=kernel // 2, groups=groups, bias=False)
        self.bn   = nn.BatchNorm2d(out_ch)

    def forward(self, x):
        return F.relu(self.bn(self.conv(x)), inplace=True)


class MobileOneBlock(nn.Module):
    """Depthwise-separable block from CoreNet's MobileOne."""
    def __init__(self, in_ch, out_ch, stride=1):
        super().__init__()
        self.dw = ConvBnAct(in_ch, in_ch,  kernel=3, stride=stride, groups=in_ch)
        self.pw = ConvBnAct(in_ch, out_ch, kernel=1)

    def forward(self, x):
        return self.pw(self.dw(x))


class MobileOneS0(nn.Module):
    """
    MobileOne-S0: 48ch stem, four stages [2, 2, 3, 1] blocks,
    1024ch bottleneck, global-avg-pool + linear head.
    """
    def __init__(self, num_classes=1000):
        super().__init__()
        self.stem   = ConvBnAct(3, 48, kernel=3, stride=2)  # 224 → 112

        def stage(in_ch, out_ch, n, stride):
            return nn.Sequential(
                MobileOneBlock(in_ch, out_ch, stride=stride),
                *[MobileOneBlock(out_ch, out_ch) for _ in range(n - 1)],
            )

        self.stage1 = stage(48,   48,   2, stride=2)   # 112 → 56
        self.stage2 = stage(48,   128,  2, stride=2)   # 56  → 28
        self.stage3 = stage(128,  256,  3, stride=2)   # 28  → 14
        self.stage4 = stage(256, 1024,  1, stride=2)   # 14  →  7

        self.pool = nn.AdaptiveAvgPool2d(1)
        self.head = nn.Linear(1024, num_classes)
        self._init()

    def _init(self):
        for m in self.modules():
            if isinstance(m, nn.Conv2d):
                nn.init.kaiming_normal_(m.weight, mode="fan_out", nonlinearity="relu")
            elif isinstance(m, nn.BatchNorm2d):
                nn.init.ones_(m.weight); nn.init.zeros_(m.bias)
            elif isinstance(m, nn.Linear):
                nn.init.normal_(m.weight, 0, 0.01); nn.init.zeros_(m.bias)

    def forward(self, x):
        x = self.stem(x)
        for stage in [self.stage1, self.stage2, self.stage3, self.stage4]:
            x = stage(x)
        return self.head(self.pool(x).flatten(1))


# ── Synthetic dataset ─────────────────────────────────────────────────────────

class SyntheticImageNet(torch.utils.data.Dataset):
    """Random 224×224 RGB tensors — no download needed."""
    def __init__(self, n, img_size=224, n_classes=1000):
        self.n, self.sz, self.nc = n, img_size, n_classes

    def __len__(self):
        return self.n

    def __getitem__(self, idx):
        return torch.randn(3, self.sz, self.sz), int(torch.randint(0, self.nc, ()))


# ── Main ──────────────────────────────────────────────────────────────────────

def fmt(s):
    m, s = divmod(int(s), 60)
    return f"{m}m{s:02d}s" if m else f"{s}s"


def main():
    if not API_KEY:
        print(
            "\n[error] Missing API key.\n"
            "  Run via CLI (no key needed):\n"
            "    ecocode run --name corenet-demo python python-sdk/demo_corenet.py\n"
            "  Or set API_KEY=ecocode_xxx and run directly.\n"
        )
        sys.exit(1)

    import ecocode

    device = torch.device(
        "cuda" if torch.cuda.is_available() else "cpu"
    )

    print("=" * 65)
    print("  EcoCode — CoreNet MobileOne-S0 on Synthetic ImageNet")
    print(f"  device       : {device}")
    if device.type == "cuda":
        print(f"  GPU          : {torch.cuda.get_device_name(0)}")
    print(f"  epochs       : {EPOCHS}  |  batch : {BATCH_SIZE}  |  lr : {LR}")
    print(f"  batches/epoch: {N_BATCHES}  (synthetic data, no download)")
    print("=" * 65)

    with ecocode.run(
        "corenet-mobileone-s0",
        api_key=API_KEY,
        base_url=BASE_URL,
        interval=1.0,
    ) as eco:

        eco.mark("phase", "setup")

        n_workers = min(4, os.cpu_count() or 1)
        dataset = SyntheticImageNet(N_BATCHES * BATCH_SIZE)
        loader  = torch.utils.data.DataLoader(
            dataset,
            batch_size=BATCH_SIZE,
            shuffle=True,
            num_workers=n_workers,
            pin_memory=(device.type == "cuda"),
            persistent_workers=(n_workers > 0),
        )

        model     = MobileOneS0(num_classes=N_CLASSES).to(device)
        criterion = nn.CrossEntropyLoss()
        optimizer = torch.optim.Adam(model.parameters(), lr=LR)

        n_params = sum(p.numel() for p in model.parameters())
        print(f"  parameters: {n_params:,}  (~{n_params/1e6:.1f}M)")
        eco.log_sequence_length(3 * IMG_SIZE * IMG_SIZE)

        global_step = 0
        tokens_seen = 0

        for epoch in range(EPOCHS):
            model.train()
            eco.log_epoch(epoch)
            eco.mark("phase", "train")

            epoch_loss, epoch_correct, epoch_total = 0.0, 0, 0
            t0 = time.time()

            for batch_idx, (images, labels) in enumerate(loader):
                images = images.to(device, non_blocking=True)
                labels = torch.tensor(labels, device=device)

                eco.mark("phase", "forward")
                optimizer.zero_grad(set_to_none=True)
                logits = model(images)
                loss   = criterion(logits, labels)

                eco.mark("phase", "backward")
                loss.backward()

                eco.mark("phase", "optimizer_step")
                optimizer.step()

                global_step += 1
                tokens_seen += images.size(0)
                epoch_loss    += loss.item() * images.size(0)
                epoch_correct += (logits.detach().argmax(1) == labels).sum().item()
                epoch_total   += images.size(0)

                eco.log_step(global_step)
                eco.log_loss(loss.item())
                eco.log_tokens(tokens_seen)

                if (batch_idx + 1) % max(1, N_BATCHES // 4) == 0:
                    acc = 100 * epoch_correct / epoch_total
                    pct = 100 * (batch_idx + 1) / N_BATCHES
                    print(f"    [{pct:5.1f}%]  loss={loss.item():.4f}  acc={acc:.1f}%  step={global_step}")

            print(
                f"\n  epoch {epoch+1:02d}/{EPOCHS}"
                f"  loss={epoch_loss/epoch_total:.4f}"
                f"  acc={100*epoch_correct/epoch_total:.1f}%"
                f"  {fmt(time.time()-t0)}"
            )

        eco.mark("phase", "done")
        print("\n" + "=" * 65)
        print(f"  Done.  steps={global_step:,}  tokens={tokens_seen:,}")
        print("=" * 65)

    print(f"\nView results at: {BASE_URL}")


if __name__ == "__main__":
    main()
