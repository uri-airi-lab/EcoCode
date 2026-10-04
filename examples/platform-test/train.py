"""Real training run for testing EcoCode end to end, with the web platform.

Trains a small CNN on MNIST with PyTorch (CUDA, Apple MPS or CPU — picked
automatically) and reports epochs, phases, steps, loss, accuracy, learning
rate, samples and throughput through the EcoCode SDK. Two DataLoader worker
processes make sure child processes are measured too.

Attached mode (the CLI measures, the SDK sends markers):
    ecocode run -o online --project "ecocode-test" --name my-run --price 0.2 -- \
        python examples/platform-test/train.py --mode attached

SDK mode (the SDK starts the monitor itself):
    python examples/platform-test/train.py --mode sdk --name my-run --project-id 42

Local mode (no web platform; checks the same run offline):
    python examples/platform-test/train.py --mode local --name my-run

See README.md in this folder for the full test procedure.
"""

from __future__ import annotations

import argparse
import os
import platform
import sys
import tempfile
import time
from pathlib import Path

try:
    import ecocode
except ImportError:  # running from a source checkout without `pip install ./python`
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python" / "src"))
    import ecocode

import torch
import torch.nn as nn
import torch.nn.functional as F
from torch.utils.data import DataLoader
from torchvision import datasets, transforms


class Net(nn.Module):
    def __init__(self) -> None:
        super().__init__()
        self.conv1 = nn.Conv2d(1, 32, 3)
        self.conv2 = nn.Conv2d(32, 64, 3)
        self.fc1 = nn.Linear(64 * 12 * 12, 128)
        self.fc2 = nn.Linear(128, 10)

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        x = F.relu(self.conv1(x))
        x = F.max_pool2d(F.relu(self.conv2(x)), 2)
        x = F.relu(self.fc1(torch.flatten(x, 1)))
        return self.fc2(x)


def pick_device(requested: str) -> torch.device:
    if requested != "auto":
        return torch.device(requested)
    if torch.cuda.is_available():
        return torch.device("cuda")
    if getattr(torch.backends, "mps", None) and torch.backends.mps.is_available():
        return torch.device("mps")
    return torch.device("cpu")


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--mode", choices=["attached", "sdk", "local"], required=True)
    p.add_argument("--name", default=f"platform-test-{platform.node()}")
    p.add_argument("--project-id", type=int, default=None, help="project ID for --mode sdk")
    p.add_argument("--epochs", type=int, default=3)
    p.add_argument("--batch-size", type=int, default=128)
    p.add_argument("--max-batches", type=int, default=0, help="limit batches per epoch (0 = all)")
    p.add_argument("--device", default="auto", help="auto, cpu, cuda, mps")
    p.add_argument("--interval", type=float, default=1.0, help="sampling interval in seconds (sdk/local)")
    p.add_argument("--data", default=str(Path(tempfile.gettempdir()) / "ecocode-mnist"))
    p.add_argument("--crash", action="store_true", help="raise an error after the first epoch (tests a failed run)")
    return p.parse_args()


def open_run(args: argparse.Namespace) -> "ecocode.EcoCodeRun":
    if args.mode == "attached":
        if not os.environ.get("ECOCODE_RUN_ID"):
            sys.exit("--mode attached must be started under `ecocode run ... -- python train.py --mode attached`")
        return ecocode.run(args.name)
    if args.mode == "sdk":
        return ecocode.run(args.name, exporter="online", project_id=args.project_id, interval=args.interval)
    out = f"{args.name}.json"
    print(f"Local mode: records go to {out}")
    return ecocode.run(args.name, exporter="json", output_file=out, interval=args.interval)


def evaluate(model: nn.Module, loader: DataLoader, device: torch.device) -> tuple[float, float]:
    model.eval()
    loss_sum, correct, seen = 0.0, 0, 0
    with torch.no_grad():
        for x, y in loader:
            x, y = x.to(device), y.to(device)
            out = model(x)
            loss_sum += F.cross_entropy(out, y, reduction="sum").item()
            correct += (out.argmax(1) == y).sum().item()
            seen += len(y)
    model.train()
    return loss_sum / seen, correct / seen


def main() -> None:
    args = parse_args()
    device = pick_device(args.device)
    print(f"EcoCode platform test — mode={args.mode} device={device} torch={torch.__version__} "
          f"python={platform.python_version()} os={platform.platform()}")

    started = time.time()
    with open_run(args) as run:
        print(f"SDK mode: {run._mode}")

        run.mark("phase", "data_loading")
        tf = transforms.Compose([transforms.ToTensor(), transforms.Normalize((0.1307,), (0.3081,))])
        train_set = datasets.MNIST(args.data, train=True, download=True, transform=tf)
        test_set = datasets.MNIST(args.data, train=False, download=True, transform=tf)
        train_loader = DataLoader(train_set, batch_size=args.batch_size, shuffle=True,
                                  num_workers=2, persistent_workers=True)
        test_loader = DataLoader(test_set, batch_size=512, num_workers=2, persistent_workers=True)

        model = Net().to(device)
        optimizer = torch.optim.Adam(model.parameters(), lr=1e-3)
        scheduler = torch.optim.lr_scheduler.StepLR(optimizer, step_size=1, gamma=0.7)

        step = 0
        samples = 0
        accuracy = 0.0
        for epoch in range(args.epochs):
            # The phase reads "epoch:N" while training; the epoch number is also a
            # custom metric so it stays visible during evaluation.
            run.log_epoch(epoch)
            run.log_custom("epoch", epoch)
            run.log_lr(optimizer.param_groups[0]["lr"])
            epoch_start, epoch_samples = time.time(), 0
            for batch, (x, y) in enumerate(train_loader):
                if args.max_batches and batch >= args.max_batches:
                    break
                x, y = x.to(device), y.to(device)
                optimizer.zero_grad()
                loss = F.cross_entropy(model(x), y)
                loss.backward()
                grad_norm = torch.nn.utils.clip_grad_norm_(model.parameters(), 10.0)
                optimizer.step()

                step += 1
                samples += len(y)
                epoch_samples += len(y)
                if step % 20 == 0:  # a marker every 20 steps is plenty at 1 s sampling
                    run.log_step(step)
                    run.log_loss(loss.item())
                    run.log_tokens(samples)  # samples processed so far
                    run.log_custom("grad_norm", float(grad_norm))
                    run.log_custom("samples_per_sec", epoch_samples / max(time.time() - epoch_start, 1e-6))

            run.mark("phase", "eval")
            val_loss, accuracy = evaluate(model, test_loader, device)
            run.log_accuracy(accuracy)
            run.log_custom("val_loss", val_loss)
            print(f"epoch {epoch}: step {step}, val_loss {val_loss:.4f}, accuracy {accuracy:.4f}, "
                  f"{epoch_samples / (time.time() - epoch_start):.0f} samples/s")

            if args.crash:
                raise RuntimeError("--crash: simulated training failure after epoch 0")

            run.mark("phase", "checkpoint_save")
            torch.save(model.state_dict(), Path(tempfile.gettempdir()) / "ecocode-mnist-cnn.pt")
            scheduler.step()

        run.mark("phase", "done")

    print(f"Finished in {time.time() - started:.0f} s: {step} steps, {samples} samples, "
          f"final accuracy {accuracy:.4f}, device {device}")


if __name__ == "__main__":  # required for DataLoader workers on macOS and Windows
    main()
