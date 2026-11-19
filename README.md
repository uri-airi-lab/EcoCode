# EcoCode

<div align="center">

<img src="docs/assets/ecocode-quill.svg" alt="EcoCode logo" width="140"/>

**Per-process energy monitoring for Linux, macOS and Windows**

[![License](https://img.shields.io/badge/License-AGPL--3.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.99%2B-orange.svg)](https://www.rust-lang.org/)
[![Platforms](https://img.shields.io/badge/platforms-Linux%20%7C%20macOS%20%7C%20Windows-lightgrey.svg)](#platform-support)

</div>

EcoCode measures how much energy a program uses while it runs. It reads the machine's hardware
energy counters (RAPL on Linux, Apple's energy counters on macOS, the Energy Meter Interface on
Windows, NVIDIA NVML for GPUs) and attributes that energy to the monitored process and its
children based on their share of CPU, memory and GPU. It does not estimate from TDP.

It ships as a CLI that can wrap any command, and as a Python SDK for ML training loops that
also records steps, loss and training phases alongside the energy data.

![EcoCode live run dashboard](docs/assets/live-run-dashboard.png)

Runs can be streamed to the EcoCode web dashboard, where each run gets a leaf rating from 0.5
to 5 based on energy-normalised utilisation, grid carbon intensity and data-centre overhead
(PUE), so runs of different length and size can be compared.

## Install

```bash
pip install ecocodes
```

The package is called `ecocodes` on PyPI because `ecocode` was already taken; you still
`import ecocode` and run `ecocode`. The wheel includes the `ecocode` binary. To build from source
instead, see [CONTRIBUTING.md](CONTRIBUTING.md).

## Usage

Check what this machine can measure:

```bash
ecocode doctor
```

Monitor any command and print a live table:

```bash
ecocode run -- python train.py
```

Send results to the web dashboard:

```bash
ecocode config set-api-key eco_your_key
ecocode run -o online --project efficiency-study -- python train.py
```

From Python, with ML markers:

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

Other exporters: `terminal` (default), `csv`, `json`, `logfile` and `prometheus`. See
[docs/cli.md](docs/cli.md) and [python/README.md](python/README.md).

## Platform support

| | Linux (x86_64, arm64) | macOS (Apple Silicon, Intel) | Windows (x86_64) |
|---|---|---|---|
| CPU energy | ✅ RAPL (Intel, AMD); hwmon on ARM servers | ✅ Apple Silicon; ✅ Intel via SMC (estimated) | ✅ where the PC has an Energy Meter Interface (many laptops); ❌ most desktops |
| DRAM energy | ✅ where RAPL exposes it | ✅ Apple Silicon | ✅ where EMI exposes it |
| GPU energy | ✅ NVIDIA (NVML) | ✅ Apple GPU | ✅ NVIDIA (NVML) |
| Per-process CPU / memory | ✅ | ✅ | ✅ |
| Python SDK + markers | ✅ | ✅ | ✅ |
| Needs admin/root | Once, to install the RAPL udev rule (or never, if RAPL is already readable) | No | No |

How each value is measured: [Linux](docs/platforms/linux.md), [macOS](docs/platforms/macos.md),
[Windows](docs/platforms/windows.md).

## Documentation

- [Tutorial](docs/tutorial.md): install, first run, Python SDK and framework callbacks.
- [CLI reference](docs/cli.md): commands, flags, exporters.
- [Python SDK](python/README.md): API, integrations (HuggingFace, TRL, Lightning, DeepSpeed, vLLM, Keras, ONNX Runtime).
- [Architecture](docs/architecture.md): crates, the sampling loop, attribution, per-OS sensors.
- [Packaging](packaging/README.md): wheels, Linux install scripts, Homebrew, winget.
- [Contributing](CONTRIBUTING.md): building, testing, submitting changes.

## License

EcoCode is licensed under the [GNU Affero General Public License v3.0](LICENSE). If you run a
modified version as a network service, you must make the source available to its users.

If the AGPL does not fit your use, for example inside a closed-source product or service, contact
us about a commercial license: Mohamed Aziz Khezami (mohamedaziz.khezami@uri.edu) or Professor
Abdeltawab Hendawi (hendawi@uri.edu). See [COMMERCIAL_LICENSE.md](COMMERCIAL_LICENSE.md).
