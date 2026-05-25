"""
Test: EcoCode SDK — Explicit start()/stop() API
Trains a CNN with the explicit lifecycle pattern (for Jupyter / long-lived servers).

Usage:
    export ECOCODE_API_KEY=eco_...
    python test_start_stop.py
"""

import os
import sys

os.environ.setdefault("ECOCODE_API_URL", "http://localhost:3001")

import torch
import torch.nn as nn
import torch.optim as optim
from torch.utils.data import DataLoader, TensorDataset

import ecocode


def main():
    api_key = os.environ.get("ECOCODE_API_KEY")
    if not api_key:
        print("ERROR: Set ECOCODE_API_KEY first.")
        sys.exit(1)

    base_url = os.environ.get("ECOCODE_API_URL", "http://localhost:3001")
    print(f"[test] start()/stop() API test")
    print(f"[test] Backend: {base_url}")
    print()

    # Simple CNN
    model = nn.Sequential(
        nn.Conv2d(1, 32, 3, padding=1), nn.ReLU(), nn.MaxPool2d(2),
        nn.Conv2d(32, 64, 3, padding=1), nn.ReLU(), nn.MaxPool2d(2),
        nn.Flatten(),
        nn.Linear(64 * 7 * 7, 128), nn.ReLU(),
        nn.Linear(128, 10),
    )
    criterion = nn.CrossEntropyLoss()
    optimizer = optim.Adam(model.parameters(), lr=1e-3)

    # Fake MNIST-like data
    images = torch.randn(1000, 1, 28, 28)
    labels = torch.randint(0, 10, (1000,))
    loader = DataLoader(TensorDataset(images, labels), batch_size=64, shuffle=True)

    # =============================================
    # EXPLICIT START/STOP (no context manager)
    # =============================================
    print("=" * 60)
    print("  START/STOP API TEST")
    print("=" * 60 + "\n")

    run = ecocode.run(
        "start-stop-test",
        api_key=api_key,
        base_url=base_url,
        interval=1.0,
    )

    # --- Simulate "Cell 1" in a notebook ---
    run.start()
    print(f"[ecocode] Mode: {run._mode}")
    print(f"[ecocode] Run ID: {run._run_id}")
    print("[notebook] Cell 1: run.start() called\n")

    # --- Simulate "Cell 2" ---
    run.mark("phase", "training")
    for epoch in range(1000000):
        run.log_epoch(epoch)
        model.train()
        for batch_idx, (imgs, lbls) in enumerate(loader):
            outputs = model(imgs)
            loss = criterion(outputs, lbls)
            optimizer.zero_grad()
            loss.backward()
            optimizer.step()
            run.log_step(epoch * len(loader) + batch_idx)
            run.log_loss(loss.item())

        print(f"  Epoch {epoch+1} done — loss={loss.item():.4f}")

    print("[notebook] Cell 2: training done\n")

    # --- Simulate "Cell 3" ---
    run.mark("phase", "done")
    run.stop()
    print("[notebook] Cell 3: run.stop() called")

    print("\n[test] start()/stop() API test PASSED.")


if __name__ == "__main__":
    main()
