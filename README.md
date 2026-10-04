# EcoCode

<div align="center">

<img src="docs/assets/ecocode-logo.png" alt="EcoCode Logo" width="140"/>

**High-precision, multi-GPU energy monitoring for sustainable software development**

[![License](https://img.shields.io/badge/License-AGPL--3.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.99%2B-orange.svg)](https://www.rust-lang.org/)
[![Platforms](https://img.shields.io/badge/platforms-Linux%20%7C%20macOS%20%7C%20Windows-lightgrey.svg)](#platform-support)

</div>

---

EcoCode is an open-source toolchain that monitors the real-time energy footprint of processes on **Linux, macOS and Windows**. It tracks **CPU, multiple GPUs, RAM, and integrated GPU** power consumption.

Unlike other tools that rely on TDP (thermal design power) estimations, EcoCode reads **direct hardware sensors** — RAPL on Linux, Apple's energy counters on macOS, the Energy Meter Interface on Windows, and NVIDIA NVML for GPUs — in real time. It attributes energy to a monitored process and its child processes based on their CPU/GPU/memory utilisation, or uses per-process energy directly where the OS measures it (Apple Silicon).

---

## Live Dashboard Preview

EcoCode comes with a premium-grade collaborative web dashboard. When you run your experiments, metrics are streamed in real time via Server-Sent Events (SSE).

![EcoCode Live Run Dashboard](docs/assets/live-run-dashboard.png)

Every completed run is also graded with a **leaf rating** from 0.5 to 5 🍃 — a weighted blend of energy-normalized utilization (50%), grid carbon cleanliness (35%), and infrastructure overhead via PUE (15%), so runs of any length or size can be compared on equal footing.

---

## Why EcoCode?

* **Real Hardware Measurements:** Reads actual energy counters (RAPL, Apple IOReport, Windows EMI, NVIDIA NVML) rather than estimating from manufacturer TDP specifications. `ecocode doctor` shows exactly what your machine can measure.
* **Per-Process GPU Attribution:** Attributes GPU energy proportionally to the specific monitored process across multiple GPUs independently.
* **Language & Runtime Agnostic:** Run any process out-of-the-box (e.g. C++, Node.js, Python, simulation scripts) using the CLI wrapper, or use the native Python SDK for ML training loops.
* **Team Collaboration:** Group runs under projects, invite team members, manage developer API keys, and track grid carbon intensity.
* **Leaf Sustainability Score:** A single 0.5–5 leaf rating per run, project, or workspace — blending hardware utilization, grid cleanliness, and PUE into one comparable number, so a short script and a long training job are judged fairly.
* **Ultra-Low Overhead:** Written in Rust; uploads happen in the background, so sampling never waits on the network.

---

## Quick Start in 2 Minutes

EcoCode can be used as a standalone terminal CLI wrapper or integrated directly into Python scripts.

### Option 1: CLI (Zero Code Changes)
Monitor any process or script from the outside. Install with `pip install ecocode` (the package includes the `ecocode` binary) or build from source (`cargo build --release`), then run:

```bash
# 0. See what this machine can measure
ecocode doctor

# 1. Save your dashboard API key (optional)
ecocode config set-api-key eco_your_api_key_here

# 2. Run and stream to the web dashboard
ecocode run -o online --project "efficiency-study" -- python train.py
```

### Option 2: Python SDK (In-Code Control)
Perfect for ML training loops where you want to log loss, steps, and epochs alongside energy data.

```bash
pip install ecocode
```

```python
import ecocode

with ecocode.run("llama3-finetune", exporter="online", api_key="eco_...") as run:
    for epoch in range(epochs):
        run.log_epoch(epoch)
        for batch in loader:
            run.mark("phase", "forward")
            loss = model(batch)
            run.log_loss(loss.item())
            run.log_step(global_step)
```

---

## Platform Support

| | Linux (x86_64, arm64) | macOS (Apple Silicon, Intel) | Windows (x86_64) |
|---|---|---|---|
| CPU energy | ✅ RAPL (Intel, AMD); hwmon on ARM servers | ✅ Apple Silicon; ✅ Intel via SMC (estimated) | ✅ where the PC has an Energy Meter Interface (many laptops); ❌ most desktops |
| DRAM energy | ✅ where RAPL exposes it | ✅ Apple Silicon | ✅ where EMI exposes it |
| GPU energy | ✅ NVIDIA (NVML) | ✅ Apple GPU | ✅ NVIDIA (NVML) |
| Per-process CPU / memory | ✅ | ✅ | ✅ |
| Python SDK + markers | ✅ | ✅ | ✅ |
| Needs admin/root | Once, to install the RAPL udev rule (or never, if RAPL is already readable) | No | No |

How each value is measured and attributed: [Linux](docs/platforms/linux.md) · [macOS](docs/platforms/macos.md) · [Windows](docs/platforms/windows.md).
To check a machine end to end, run `scripts/selftest.sh` (Linux/macOS) or `scripts\selftest.ps1` (Windows).

---

## Documentation Hub

For advanced configurations, integration guides, and platform setup, refer to the following sub-folder guides:

* **[Getting Started Tutorial](docs/tutorial.md)** — step-by-step guide: install the CLI, run your first experiment, and use the Python SDK with ML framework callbacks.
* **[CLI Reference & Installation Guide](docs/cli.md)** — detailed usage, CLI flags, CSV/JSON/logfile/Prometheus exporters, and permissions.
* **[Python SDK & ML Integrations Guide](python/README.md)** — callback references for HuggingFace, PyTorch Lightning, TRL, Keras, DeepSpeed, vLLM, and ONNX Runtime.
* **[Architecture](docs/architecture.md)** — crates, the sampling loop, energy attribution, and how each OS is handled.
* **Platform guides** — [Linux](docs/platforms/linux.md), [macOS](docs/platforms/macos.md), [Windows](docs/platforms/windows.md): sensors, permissions, accuracy notes.
* **[Packaging](packaging/README.md)** — pip wheels, the Linux install scripts, Homebrew, winget.
* **[Contributing](CONTRIBUTING.md)** — building, testing on each OS, and submitting changes.

---

## License

EcoCode (CLI and Python SDK) is licensed under the [GNU Affero General Public License v3.0](LICENSE) (AGPL-3.0-only).

You are free to use, modify, and distribute this software under the terms of the AGPL-3.0. If you run a modified version as a network service, you must make the complete corresponding source code available to users of that service.

See the [LICENSE](LICENSE) file for the full license text.

