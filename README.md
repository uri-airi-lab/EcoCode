# EcoCode 🌍⚡

<div align="center">

<img src="ecocode-logo.png" alt="EcoCode Logo" width="150"/>

**High-precision, multi-GPU energy monitoring for sustainable software development**

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![Next.js](https://img.shields.io/badge/Next.js-15-black.svg)](https://nextjs.org/)

</div>

---

EcoCode monitors the real-time energy footprint of any process on your Linux machine — tracking **CPU, multiple GPUs, RAM, and integrated GPU** power consumption per process using RAPL and NVML. It includes a **web dashboard** (Rust/Axum backend + Next.js frontend) for visualizing historical runs, and supports **multiple export formats** including terminal, CSV, JSON, SQLite, Prometheus, and log files.

## ✨ Key Featuressource "$HOME/.cargo/env"


| Feature | Description |
|---------|-------------|
| **Multi-GPU Monitoring** | Automatically detects and monitors all NVIDIA GPUs via NVML, with per-GPU energy and utilization breakdown |
| **Multi-Socket CPU** | Per-socket RAPL energy readings across CPU, DRAM, and iGPU domains |
| **Process-Level Attribution** | Tracks energy usage per PID using RAPL + NVML process utilization stats |
| **7 Export Formats** | Terminal, CSV, JSON, SQLite (local), PostgreSQL (online), Prometheus metrics, and **log file** |
| **Web Dashboard** | Real-time SSE-powered dashboard with per-GPU/socket charts, energy breakdown, and run history |
| **< 1% Overhead** | Built in Rust with zero-cost abstractions and async I/O |

---

## 🏗️ Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        EcoCode Core (Rust)                      │
│  ┌─────────┐  ┌──────────────┐  ┌──────────────────────────┐   │
│  │  RAPL   │  │  NVML (GPU)  │  │     sysinfo (CPU/Mem)    │   │
│  │ Sensors │  │  Multi-GPU   │  │     Process Tracking     │   │
│  └────┬────┘  └──────┬───────┘  └───────────┬──────────────┘   │
│       └──────────────┼──────────────────────┘                   │
│                      ▼                                          │
│             ┌────────────────┐                                   │
│             │  Record Engine │─── gpu_metrics[]                  │
│             │                │─── socket_metrics[]               │
│             └───────┬────────┘                                   │
│                     │                                            │
│   ┌─────────────────┼──────────────────────────────────┐        │
│   ▼        ▼        ▼        ▼       ▼        ▼       ▼        │
│ Terminal  CSV     JSON    SQLite  Postgres Prometheus LogFile   │
└─────────────────────┬───────────────────────────────────────────┘
                      │ SQLite DB (shared)
┌─────────────────────┼───────────────────────────────────────────┐
│              Web Backend (Axum)                                  │
│  REST API + SSE  ── gpu_records / socket_records tables         │
└─────────────────────┬───────────────────────────────────────────┘
                      │ HTTP
┌─────────────────────┼───────────────────────────────────────────┐
│              Web Frontend (Next.js)                              │
│  Dashboard ── Charts ── Per-GPU/Socket visualization            │
└─────────────────────────────────────────────────────────────────┘
```

---

## 🚦 Getting Started

### Prerequisites

| Requirement | Details |
|-------------|---------|
| **OS** | Linux (kernel 3.10+) |
| **Rust** | 1.70+ (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh`) |
| **NVIDIA Drivers** | For GPU monitoring (NVML must be available) |
| **Node.js** | 18+ (for web dashboard) |
| **pnpm** | `npm install -g pnpm` (for web frontend) |
| **sudo** | Required to read RAPL energy counters |

### Step 1: Clone & Build the Core Tool

```bash
git clone https://github.com/MohamedAziz-Khezami/EcoCode.git
cd EcoCode

# Build the core monitoring tool
cd ecocode
cargo build --release
```

### Step 2: Run the Core Tool

The core tool wraps any command and monitors its energy consumption:

```bash
# Basic usage with terminal output
sudo ./target/release/ecocode -- python train.py

# Save to SQLite (used by the web dashboard)
sudo ./target/release/ecocode -o local --collaborator "your-name" -- python train.py

# Save to CSV file
sudo ./target/release/ecocode -o csv -f results.csv -- python train.py

# Save to log file
sudo ./target/release/ecocode -o logfile -f run.log -- python train.py

# Save as JSON
sudo ./target/release/ecocode -o json -f results.json -- python train.py

# Prometheus metrics endpoint (port 9091)
sudo ./target/release/ecocode -o prometheus -- python train.py

# Custom measurement interval (default: 1 second)
sudo ./target/release/ecocode -i 2 -o terminal -- sleep 10
```

**CLI options:**

| Flag | Description | Default |
|------|-------------|---------|
| `-o, --output` | Export format: `terminal`, `csv`, `json`, `local`, `online`, `prometheus`, `logfile` | `terminal` |
| `-f, --file` | Output file path (required for `csv`, `json`, `logfile`) | — |
| `-p, --project` | Project name for grouping runs | `ecocode` |
| `--collaborator` | Collaborator name (required for `local`) | — |
| `-i, --interval` | Measurement interval in seconds | `1` |

### Step 3: Run the Web Dashboard

The web dashboard consists of a **Rust backend** (Axum) and a **Next.js frontend**:

```bash
# Terminal 1: Start the backend (reads from the same SQLite DB)
cd web/backend
cargo run
# → Backend starts at http://localhost:3001

# Terminal 2: Start the frontend
cd web/frontend
pnpm install   # first time only
pnpm dev
# → Frontend starts at http://localhost:3000
```

Open **http://localhost:3000** in your browser to see your runs, energy charts, and per-GPU/socket breakdowns.

---

## 📊 What Gets Monitored

| Metric | Source | Granularity |
|--------|--------|-------------|
| CPU Usage (%) | `sysinfo` | Per-process |
| CPU Energy (W) | RAPL `/sys/class/powercap/intel-rapl/` | Per-socket, per-domain (cpu, dram, igpu) |
| GPU Usage (%) | NVML `process_utilization_stats` | Per-GPU, per-process |
| GPU Energy (W) | NVML `total_energy_consumption` | Per-GPU |
| Memory Usage (%) | `sysinfo` | Per-process |
| Memory Energy (W) | RAPL DRAM domain | Per-socket |
| iGPU Energy (W) | RAPL iGPU domain | Per-socket |

### Multi-GPU Support

EcoCode automatically detects all NVIDIA GPUs and monitors each independently:

- **Aggregate metrics** (`gpu_usage`, `gpu_energy`) are stored as totals for backward-compatible summaries
- **Per-GPU metrics** are stored in a separate `gpu_records` table with `device_index`, `gpu_usage`, and `gpu_energy` per record
- Charts in the web dashboard dynamically render per-GPU lines/bars when multiple GPUs are present

### Multi-Socket Support

RAPL scanning already handles multiple CPU sockets. Per-socket energy deltas are stored in a `socket_records` table with `socket`, `domain` (cpu/dram/igpu), and `energy` per record.

---

## 🗃️ Database Schema

When using the `local` (SQLite) exporter, EcoCode creates these tables:

| Table | Purpose |
|-------|---------|
| `projects` | Project grouping with collaborator management |
| `runs` | Individual monitoring runs with timestamps and status |
| `records` | Per-second aggregate measurements (cpu/gpu/mem/igpu usage + energy) |
| `gpu_records` | Per-GPU breakdown per record (device_index, usage, energy) |
| `socket_records` | Per-socket RAPL breakdown per record (socket, domain, energy) |
| `runsummary` | Total energy sums per run |

---

## 🤝 Contributing

We welcome contributions! Key areas:

- 🪟 Windows RAPL/PCM integration
- 🎨 Dashboard UI improvements
- 🧪 Testing on multi-GPU setups
- 📚 Documentation

```bash
# Dev workflow
cargo fmt          # format code
cargo clippy       # lint
cargo build        # build
```

---

## 📄 License

MIT License — see [LICENSE](LICENSE) for details.

---

<div align="center">

**Building a sustainable future for AI, one watt at a time** 💚

</div>