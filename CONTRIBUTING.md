# Contributing to EcoCode

Thank you for your interest in contributing to EcoCode! We welcome contributions of all kinds, including bug fixes, feature additions, documentation improvements, and feedback.

---

## Technical Stack & Layout

The project consists of two main components:
1. **CLI & Core Engine (`crates/`)**: A Rust workspace that reads hardware energy counters and performs energy attribution:
   - `ecocode-sensors` — hardware sensors (the only crate with OS-specific sensor code)
   - `ecocode-monitor` — the measurement engine (attribution, markers)
   - `ecocode-api` — client for the web backend
   - `ecocode-cli` — the `ecocode` binary (commands and exporters)
2. **Python SDK (`python/`)**: Integrates with machine learning frameworks and communicates via OS pipes with the monitor process.

---

## Local Setup

### Prerequisites
- **Linux, macOS or Windows.** Everything builds and tests on all three; real energy readings depend on
  the machine's sensors (see the [platform guides](docs/platforms/) and `ecocode doctor`).
- **Optional: NVIDIA GPU + drivers** for GPU energy on Linux and Windows.
- **Rust Toolchain** — `rust-toolchain.toml` pins 1.99.0; rustup selects it automatically.
- **Python** (3.9+) with `pip`.

### 1. Setting Up the Rust Engine & CLI
To compile the CLI and run the Rust tests (from the repository root):
```bash
cargo build
cargo test --workspace
```
Check which energy sensors work on your machine (and how to fix the ones that don't):
```bash
cargo run -- doctor
```
On Linux, RAPL counters are root-only by default since kernel 5.10. To read them as your user permanently (see [docs/platforms/linux.md](docs/platforms/linux.md)):
```bash
sudo packaging/linux/install-permissions.sh
```
Tests that need no hardware use fake sensors (`--sensors fake` / `ECOCODE_SENSORS=fake`).

### 2. Setting Up the Python SDK
To install the SDK locally in editable mode (this also builds the `ecocode` binary with maturin and installs it into your environment):
```bash
cd python
pip install -e ".[dev]"
```
Run the unit and end-to-end tests (no hardware or ML frameworks needed; the end-to-end tests use the binary from `target/debug` with fake sensors, so run `cargo build` first):
```bash
pytest
```
Integration tests need PyTorch and other frameworks (see `tests/integration/requirements.txt`):
```bash
python tests/integration/test_start_stop.py
```

### 3. Checking a whole machine
One command builds everything, runs every test, measures a real workload with the machine's own
sensors, and writes `selftest-report.txt`:
```bash
scripts/selftest.sh                                        # Linux, macOS
powershell -ExecutionPolicy Bypass -File scripts\selftest.ps1   # Windows
```
CI (`.github/workflows/ci.yml`) runs the tests on Linux x86_64/arm64, macOS arm64/x86_64 and
Windows x86_64.

---

## Submitting Pull Requests

1. **Fork the Repository:** Create your own branch from `main`.
2. **Coding Standards:**
   - **Rust:** Run `cargo fmt` and `cargo clippy` before committing.
   - **Python:** Format with standard pep8/black styling.
3. **Write Tests:** Add unit or integration tests for new features.
4. **Submit PR:** Write a descriptive pull request summary explaining what you changed and why.

If you have any questions or encounter issues, feel free to open a GitHub Issue!
