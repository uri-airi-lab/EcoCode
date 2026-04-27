# EcoCode

<div align="center">

<img src="ecocode-logo.png" alt="EcoCode Logo" width="150"/>

**High-precision, multi-GPU energy monitoring for sustainable software development**

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)

</div>

---

EcoCode monitors the real-time energy footprint of any Linux process — tracking **CPU, multiple GPUs, RAM, and integrated GPU** power consumption per process using Intel RAPL and NVIDIA NVML. Results can be exported to the terminal, local files, a database, or a Prometheus endpoint. A hosted **web dashboard** with real-time charts, team collaboration, and carbon reporting is available as a premium feature.

---

## Key Features

| Feature | Description |
|---------|-------------|
| **Multi-GPU Monitoring** | Detects and monitors all NVIDIA GPUs via NVML, with per-GPU energy and utilization breakdown |
| **Multi-Socket CPU** | Per-socket RAPL energy readings across CPU core, DRAM, and integrated GPU domains |
| **Process-Level Attribution** | Allocates energy to the monitored PID using proportional CPU and memory usage fractions |
| **7 Export Formats** | Terminal, CSV, JSON, SQLite (local), PostgreSQL (online), Prometheus metrics, and log file |
| **Web Dashboard** | Premium hosted dashboard with real-time SSE-powered charts, per-GPU and per-socket visualisation, and run history |
| **Carbon Footprint** | CO2 estimation using a 50-country intensity table (IEA 2025) or live data from the Electricity Maps API |
| **Team Collaboration** | Premium workspace collaboration with admin and member roles, invitation by email, and workspace-scoped API keys |
| **Low Overhead** | Built in Rust with zero-cost abstractions and asynchronous I/O |

---

## Architecture

```
+------------------------------------------------------------------+
|                      EcoCode CLI (Rust)                          |
|  +----------+  +--------------+  +---------------------------+  |
|  |  RAPL    |  |  NVML (GPU)  |  |  sysinfo (CPU / Memory)   |  |
|  | /sys/... |  |  Multi-GPU   |  |  Process Tracking (PID)   |  |
|  +----+-----+  +------+-------+  +-----------+---------------+  |
|       +----------------+-----------------------+                 |
|                        v                                         |
|             +----------------------+                             |
|             |    Record Engine     |--- gpu_metrics[]            |
|             |  (energy + carbon)   |--- socket_metrics[]         |
|             +----------+-----------+                             |
|                        |                                         |
|  +-----------+---------+----------+----------+----------+------+ |
|  v           v         v          v          v          v      | |
| Terminal    CSV      JSON      SQLite   PostgreSQL  Prometheus  | |
|                                                      LogFile   | |
+------------------------------------------------------------------+
                         | SQLite or HTTP (online exporter)
+------------------------------------------------------------------+
|              Web Backend (Axum + PostgreSQL)                     |
|  REST API + SSE  --  gpu_records / socket_records tables         |
|  Auth (JWT + API keys)  --  Workspaces  --  Carbon config        |
+------------------------------------------------------------------+
                         | HTTP
+------------------------------------------------------------------+
|              Web Frontend (Next.js)                              |
|  Dashboard  --  Charts  --  Per-GPU / Per-Socket visualisation   |
|  Authentication  --  Workspace management  --  Carbon settings   |
+------------------------------------------------------------------+
```

---

## Getting Started

### Prerequisites

| Requirement | Details |
|-------------|---------|
| **OS** | Linux (kernel 3.10+) |
| **Rust** | 1.70+ (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh`) |
| **NVIDIA Drivers** | Required for GPU monitoring (NVML must be available) |
| **sudo** | Required to read RAPL energy counters |

---

### Step 1: Build the CLI Tool

```bash
git clone https://github.com/MohamedAziz-Khezami/EcoCode.git
cd EcoCode/ecocode
cargo build --release
```

### Step 2: Run the CLI Tool

The tool wraps any command and monitors its energy consumption for the duration of execution.

```bash
# Live output to terminal
sudo ./target/release/ecocode -- python train.py

# Save to local SQLite database
sudo ./target/release/ecocode -o local --collaborator "alice" -- python train.py

# Save to CSV file
sudo ./target/release/ecocode -o csv -f results.csv -- python train.py

# Save to JSON file
sudo ./target/release/ecocode -o json -f results.json -- python train.py

# Write a formatted log file
sudo ./target/release/ecocode -o logfile -f run.log -- python train.py

# Expose a Prometheus /metrics endpoint on port 9091
sudo ./target/release/ecocode -o prometheus -- python train.py

# Send data to the premium web platform
sudo ./target/release/ecocode -o online --api-key <KEY> -- python train.py

# Custom sampling interval (default: 1 second)
sudo ./target/release/ecocode -i 0.5 -o terminal -- python train.py
```

**CLI reference:**

| Flag | Description | Default |
|------|-------------|---------|
| `-o, --output` | Export format: `terminal`, `csv`, `json`, `local`, `online`, `prometheus`, `logfile` | `terminal` |
| `-f, --file` | Output file path (required for `csv`, `json`, `logfile`) | — |
| `-p, --project` | Project name for grouping runs | `ecocode` |
| `--collaborator` | Name of the person running the experiment | — |
| `-i, --interval` | Sampling interval in seconds (supports decimals) | `1.0` |
| `--intensity` | Carbon intensity override in gCO2eq/kWh | project default |
| `--price, --cost` | Electricity cost per kWh for financial estimation | — |
| `--address` | Location for automatic carbon intensity lookup | — |
| `--api-key` | API key for the `online` exporter | — |

**API key management:**

```bash
# Save your API key once (avoids passing it on every run)
ecocode config set-api-key <KEY>

# View current configuration
ecocode config show

# Remove saved API key
ecocode config clear-api-key
```

---

## What Gets Monitored

| Metric | Source | Granularity |
|--------|--------|-------------|
| CPU Usage (%) | `sysinfo` | Per-process |
| CPU Energy (J) | Intel RAPL `/sys/class/powercap/intel-rapl/` | Per-socket, per-domain (cpu, dram, igpu) |
| GPU Usage (%) | NVML `process_utilization_stats` | Per-GPU, per-process |
| GPU Energy (J) | NVML `total_energy_consumption` | Per-GPU |
| Memory Usage (%) | `sysinfo` | Per-process |
| Memory Energy (J) | RAPL DRAM domain | Per-socket |
| iGPU Energy (J) | RAPL uncore domain | Per-socket |

Energy is stored in Joules (1 Wh = 3600 J).

### Multi-GPU Support

EcoCode detects all NVIDIA GPUs at startup and monitors each independently:

- **Aggregate metrics** (`gpu_usage`, `gpu_energy`) are stored as totals for backward-compatible summaries.
- **Per-GPU metrics** are stored in a separate `gpu_records` table keyed by `device_index`.

### Multi-Socket CPU Support

RAPL scanning handles multiple CPU sockets automatically. Per-socket energy deltas are stored in a `socket_records` table with `socket`, `domain` (`cpu`, `dram`, `igpu`), and `energy` per record.

### Counter Wrap-Around Handling

Both RAPL and NVML counters are monotonically increasing integers that reset to zero on hardware events (driver reload, GPU reset). EcoCode tracks the running maximum for each counter and applies a correction when a backward jump is detected, ensuring no energy is lost across counter resets.

---

## Carbon Footprint Estimation

The formula used is:

```
CO2 (g) = energy_kWh * intensity_gCO2_per_kWh * PUE
```

Where:
- `energy_kWh` = total energy (CPU + GPU + Memory + iGPU) converted from Joules
- `intensity_gCO2_per_kWh` = grid carbon intensity for the configured region
- `PUE` = Power Usage Effectiveness factor (1.0 for workstations, ~1.58 for average data centres)

Carbon intensity can be set in three ways, in order of precedence:
1. **Live API** — real-time data from [Electricity Maps](https://electricitymaps.com/) when a project is configured with a location and API key.
2. **Static table** — 50+ country values sourced from IEA 2025 data, selectable by country code.
3. **Global average** — 445 gCO2eq/kWh (IEA Emissions Factors 2025, fallback when no configuration is present).

Environmental equivalents derived from the run's CO2 figure:

| Equivalent | Factor | Source |
|---|---|---|
| Car distance | 170 gCO2/km | IEA global passenger car fleet average |
| Smartphone charges | 8.2 gCO2/charge | ~16 Wh battery at world-average grid intensity |
| Trees (1-day absorption) | 22 kg CO2/year per tree | One Tree Planted / EcoTree species average |

---

## Contributing

Contributions are welcome. Key areas for improvement:

- AMD GPU support via ROCm SMI
- Windows RAPL / Intel PCM integration
- Extended test coverage for multi-socket systems
- Containerised deployment (Docker Compose)

```bash
# Development workflow
cargo fmt       # format code
cargo clippy    # lint
cargo build     # compile
```

---

## License

MIT License — see [LICENSE](LICENSE) for details.

---

<div align="center">

**Building a sustainable future for AI, one watt at a time**

</div>
