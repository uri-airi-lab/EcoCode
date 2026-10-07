# Contributing to EcoCode

Bug reports, fixes, new sensors, framework integrations and documentation improvements are all
welcome. Open an issue first for anything large so we can agree on the approach.

## Layout

```
crates/
  ecocode-sensors   hardware sensors and per-process stats (the only OS-specific sensor code)
  ecocode-monitor   the sampling loop, energy attribution, markers, the Record type
  ecocode-api       HTTP client and wire types for the web backend
  ecocode-cli       the `ecocode` binary: commands, exporters, platform glue
python/             the Python SDK (bundles the binary in its wheels)
packaging/          Linux install scripts, Homebrew formula, winget manifests
scripts/            self-test scripts that exercise a whole machine
docs/               user and architecture docs
```

[docs/architecture.md](docs/architecture.md) explains how the pieces fit together.

## Prerequisites

- Linux, macOS or Windows. Everything builds and tests on all three. Real energy readings depend
  on the machine's sensors; `ecocode doctor` tells you what works.
- Rust. `rust-toolchain.toml` pins the version and rustup picks it up automatically.
- Python 3.9 or newer with pip.
- Optional: an NVIDIA GPU and drivers for GPU energy on Linux and Windows.

## Building and testing the CLI

```bash
cargo build
cargo test --workspace
cargo run -- doctor
```

Tests that need no hardware use fake sensors (`--sensors fake` or `ECOCODE_SENSORS=fake`).
Tests against the machine's real sensors are `#[ignore]`d; run them with
`cargo test --workspace -- --ignored`.

On Linux, RAPL counters are root-only by default. To read them as your user:

```bash
sudo packaging/linux/install-permissions.sh
```

See [docs/platforms/linux.md](docs/platforms/linux.md) for details.

## Building and testing the Python SDK

Install the SDK in editable mode. This also builds the `ecocode` binary with maturin and
installs it into your environment:

```bash
cd python
pip install -e ".[dev]"
```

The tests come in three tiers:

| Tier | Command | Needs |
|---|---|---|
| unit + e2e | `pytest` | the binary (`cargo build` first), fake sensors |
| frameworks | `pytest tests/frameworks -v` | real HF / TRL / Lightning / Keras / ONNX Runtime on tiny CPU models, no network |
| heavy | `pytest -m heavy tests/integration -v -s` | real GPT-2 / ResNet workloads; CUDA recommended |

The framework and heavy tiers drive real trainers through the real callbacks and the real
binary, then check the markers that were emitted and the records the monitor wrote. They need a
Python the frameworks support (3.10 to 3.12), ideally in its own venv:

```bash
python3.12 -m venv .venv-frameworks && source .venv-frameworks/bin/activate
pip install -e ".[test-frameworks]"
pytest tests/frameworks -v
```

The heavy tier downloads GPT-2 from the Hub. Without `ECOCODE_API_KEY` it records locally with
fake sensors; with a key it reports online. On CPU the workloads are scaled down by
`ECOCODE_HEAVY_SCALE` (default `0.05`). `tests/integration/run_tests.sh` runs each file in its
own process. The vLLM and DeepSpeed tests need Linux with CUDA and skip elsewhere.

## Checking a whole machine

One command builds everything, runs every test, measures a real workload with the machine's own
sensors and writes `selftest-report.txt`:

```bash
scripts/selftest.sh
```

```powershell
powershell -ExecutionPolicy Bypass -File scripts\selftest.ps1
```

CI (`.github/workflows/ci.yml`) runs the tests on Linux x86_64/arm64, macOS arm64/x86_64 and
Windows x86_64, and builds the wheels.

## Pull requests

- Branch from `main`.
- Rust: run `cargo fmt` and `cargo clippy` before committing. Python: keep to PEP 8.
- Add or update tests for what you change. New sensors should work with fake sensors in tests
  and report themselves in `ecocode doctor`.
- Describe what changed and why in the PR. If you measured something, include the numbers.
