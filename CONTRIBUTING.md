# Contributing to EcoCode

Thank you for your interest in contributing to EcoCode! We welcome contributions of all kinds, including bug fixes, feature additions, documentation improvements, and feedback.

---

## Technical Stack & Layout

The project consists of two main components:
1. **CLI & Core Engine (`ecocode/` and `crates/core/`)**: Written in Rust, reads direct hardware counters (RAPL & NVML) and performs energy attribution.
2. **Python SDK (`python-sdk/`)**: Integrates with machine learning frameworks and communicates via OS pipes with the monitor process.

---

## Local Setup

### Prerequisites
- **Linux OS** (RAPL counters are Linux-specific).
- **Intel CPU** (for RAPL energy readings) and/or **NVIDIA GPU + Drivers** (for NVML readings).
- **Rust Toolchain** (1.75+).
- **Python** (3.8+) with `pip`.

### 1. Setting Up the Rust Engine & CLI
To compile the CLI:
```bash
cd ecocode
cargo build
```
To run the CLI, you will need read access to RAPL counters. The CLI automatically attempts to configure permissions at startup (requires `sudo` or manual permission configuration):
```bash
sudo chmod -R +r /sys/class/powercap/intel-rapl/
```

### 2. Setting Up the Python SDK
To install the SDK locally in editable mode:
```bash
cd python-sdk
pip install -e .
```
You can run integration tests:
```bash
python test_start_stop.py
```

---

## Submitting Pull Requests

1. **Fork the Repository:** Create your own branch from `main`.
2. **Coding Standards:**
   - **Rust:** Run `cargo fmt` and `cargo clippy` before committing.
   - **Python:** Format with standard pep8/black styling.
3. **Write Tests:** Add unit or integration tests for new features.
4. **Submit PR:** Write a descriptive pull request summary explaining what you changed and why.

If you have any questions or encounter issues, feel free to open a GitHub Issue!
