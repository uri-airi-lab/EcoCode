# EcoCode CLI

The EcoCode CLI is a Rust binary that wraps any Linux command and continuously samples
CPU, GPU, and memory energy in real time. It is the core measurement engine of the
EcoCode platform.

---

## Table of Contents

- [How It Works](#how-it-works)
- [Architecture](#architecture)
  - [Source Layout](#source-layout)
  - [Measurement Pipeline](#measurement-pipeline)
  - [Python SDK Integration (Attached Mode)](#python-sdk-integration-attached-mode)
- [Setup](#setup)
  - [Prerequisites](#prerequisites)
  - [Build](#build)
- [Usage](#usage)
  - [Basic Examples](#basic-examples)
  - [Output Formats](#output-formats)
  - [Options Reference](#options-reference)
  - [Config Subcommand](#config-subcommand)
- [Exporters In Depth](#exporters-in-depth)
- [Energy Attribution Model](#energy-attribution-model)
- [Carbon Footprint Calculation](#carbon-footprint-calculation)
- [Sensor Details](#sensor-details)
  - [RAPL (CPU + DRAM + iGPU)](#rapl-cpu--dram--igpu)
  - [NVML (GPU)](#nvml-gpu)
---

## How It Works

```
sudo ecocode [OPTIONS] -- <COMMAND> [ARGS...]
```

1. **CLI parsing** (`cli.rs`): clap parses all flags and subcommands.
2. **Process spawn** (`monitor.rs`): the target command is launched as a child process. Two environment variables are injected: `ECOCODE_MARKER_FD` (file descriptor of the write-end of the OS pipe) and `ECOCODE_RUN_ID` (integer run ID), so any Python SDK running inside the child can attach to this run.
3. **Warm-up** (`monitor.rs`): one pass of RAPL and NVML reads primes the counter baselines. This eliminates the startup burst (CUDA context init, library loading) from the first measurement interval.
4. **Sampling loop**: on every tick (`--interval`, default 1 s):
   - Read RAPL energy counters (μJ → J delta per socket, per domain).
   - Query NVML for GPU energy delta and per-process utilization stats.
   - Read CPU% and memory% from `sysinfo` for the root PID and its full process tree.
   - Attribute energy to the process by its resource-use fraction.
   - Read marker state from the OS pipe (phase, step, loss, etc. sent by the Python SDK).
   - Assemble a `Record` and pass it to the selected exporter.
5. **Finalisation**: on process exit or Ctrl+C, `complete_run()` and `run_summary()` are called, and the pipe is closed.

---

## Architecture

### Source Layout

```
ecocode/
├── Cargo.toml             # Workspace manifest (depends on ecocode-core)
└── src/
    ├── main.rs            # Entry point — parses CLI, dispatches to monitor or config
    ├── cli.rs             # All clap flag/subcommand definitions
    ├── monitor.rs         # Core loop: process spawning, sampling, exporter dispatch
    ├── monitor_headless.rs # Headless monitor spawned by Python SDK for online exports
    ├── monitor_local.rs   # Headless monitor spawned by Python SDK for local exports
    ├── carbon.rs          # CO2 formula + static intensity table (50 countries, IEA 2025)
    ├── configs.rs         # `ecocode config` handler (set/clear/show API key)
    └── exporter/
        ├── mod.rs         # Exporter trait + Record, GpuMetric, SocketMetric types
        ├── terminal.rs    # Print table to stdout
        ├── csv.rs         # Append rows to CSV
        ├── json.rs        # Write JSON array file
        ├── online.rs      # HTTP POST to backend (reqwest, Bearer auth)
        ├── prometheus.rs  # Prometheus gauge metrics on :9091 (warp HTTP server)
        └── logfile.rs     # Write formatted log file (BufWriter)
```

The sensors live in `crates/core/` and are shared with the Python SDK:

```
crates/core/
└── src/
    ├── sensor/
    │   ├── mod.rs         # get_process_tree_pids_from_sys() — builds child PID set
    │   ├── rapl.rs        # RaplFile, scan_rapl_files(), get_all_energies(), delta_cpu_energy_per_pid_j()
    │   └── gpu.rs         # get_all_gpu_devices(), get_all_gpu_energies(), get_all_gpu_energies_by_pid()
    └── http_client.rs     # Blocking HTTP client (used by Python SDK sampler thread)
```

### Measurement Pipeline

```
  Every interval (default 1 s)
  ─────────────────────────────────────────────────────────────

  sysinfo::System::refresh_processes()
    └─ For each PID in process tree:
         cpu_usage()  → sum → total_cpu_pct  (normalised by num_cores)
         memory()     → max → max_mem_pct    (peak RSS across tree)

  rapl::get_all_energies(&mut rapl_readers)
    └─ For each open RaplFile (socket × domain):
         seek(0) → read_line() → parse f64 (μJ)
         → RaplEnergy { socket, domain, energy, max_energy }

  rapl::delta_cpu_energy_per_pid_j(prev, curr, elapsed, cpu_pct, mem_pct, igpu_pct)
    └─ For each domain:
         raw_delta = curr - prev          # handles wrap-around via max_energy
         fraction  = cpu_pct  (domain "cpu")
                   | mem_pct  (domain "dram")
                   | cpu_pct  (domain "igpu" — proxy, no per-process sensor)
         delta_j   = raw_delta_uj / 1_000_000 * fraction / 100

  nvml::get_all_gpu_energies(&devices)     → total energy counter (mJ) per GPU
  nvml::get_all_gpu_energies_by_pid(...)   → per-GPU energy delta + process util %

  Record::new(...)
    ├─ cpu_energy_j    (sum of "cpu" domain deltas across all sockets)
    ├─ mem_energy_j    (sum of "dram" domain deltas)
    ├─ igpu_energy_j   (sum of "igpu" domain deltas)
    ├─ gpu_energy_j    (sum across all GPUs)
    ├─ gpu_metrics[]   (per-GPU device_index, gpu_usage%, gpu_energy_j)
    ├─ socket_metrics[](per-socket, per-domain energy_j)
    └─ ML fields       (from MarkerState — set by Python SDK via OS pipe)

  exporter.add_record(record)
  exporter.export_line()       ← live output (table row / HTTP POST / gauge update)

  ─────────────────────────────────────────────────────────────
  On exit:
    exporter.export()          ← finalise (summary table, flush file, etc.)
    exporter.complete_run()    ← mark status in backend
    exporter.run_summary()     ← write / request aggregate summary
```

### Python SDK Integration (Attached Mode)

When the CLI starts, it:
1. Creates an OS pipe for IPC between parent and child.
2. Clears the `CLOEXEC` flag on the write-end file descriptor so the child inherits it across `exec`.
3. Spawns a background thread that reads newline-delimited JSON events from the pipe's read-end and updates a shared `MarkerState`.
4. Injects `ECOCODE_MARKER_FD` (the write-end file descriptor) and `ECOCODE_RUN_ID=<id>` into the child process environment.
5. On every sampling tick, reads `MarkerState` and attaches current values to the `Record`.

The Python SDK detects these environment variables and switches to attached mode — it writes lightweight marker events directly to the inherited file descriptor without doing any hardware sampling itself.

**Marker event protocol (newline-delimited JSON):**

```json
{"event": "phase",           "value": "forward"}
{"event": "epoch",           "value": 3}
{"event": "step",            "value": 150}
{"event": "loss",            "value": 1.23}
{"event": "learning_rate",   "value": 0.0003}
{"event": "tokens",          "count": 204800}
{"event": "sequence_length", "value": 2048}
```

---

## Setup

### Prerequisites

| Requirement | Notes |
|-------------|-------|
| Linux x86_64 | RAPL sysfs is Linux-specific |
| Intel CPU | Required for RAPL energy readings |
| NVIDIA GPU + drivers | Optional; needed for GPU energy via NVML |
| Rust 1.75+ | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| `sudo` | Required to make RAPL counters world-readable at startup |

### Build

```bash
cd ecocode
cargo build --release
# Binary: target/release/ecocode

# Optional: install globally
sudo cp target/release/ecocode /usr/local/bin/
```

The build automatically links NVML at runtime (dynamic link). If NVML is unavailable, NVML initialization will fail with a descriptive error but the binary still runs for CPU/DRAM monitoring.

---

## Usage

### Basic Examples

```bash
# Live table to terminal
sudo ecocode -- python train.py

# 0.5 s sampling interval
sudo ecocode --interval 0.5 -- python train.py

# Label the run and group under a project
sudo ecocode --project "efficiency-study" --name "fp32-baseline" -- python train.py

# Write to CSV
sudo ecocode -o csv -f results.csv -- python train.py

# Send to web dashboard
sudo ecocode -o online --api-key eco_... -p my-project -- python train.py

# Prometheus metrics on :9091 (scrape with Prometheus)
sudo ecocode -o prometheus -- python train.py

# Override carbon intensity (useful for on-prem GPU clusters)
sudo ecocode --intensity 52.0 --price 0.12 -o online -- python train.py
```

### Output Formats

| Format | Flag | Requires | Description |
|--------|------|----------|-------------|
| `terminal` | `-o terminal` | — | Live formatted table to stdout (default) |
| `csv` | `-o csv -f out.csv` | `--file` | Append one row per tick to a CSV file |
| `json` | `-o json -f out.json` | `--file` | Write a JSON array of records |
| `online` | `-o online` | API key | POST records to the EcoCode web backend |
| `prometheus` | `-o prometheus` | — | Expose Prometheus gauges on `http://localhost:9091/metrics` |
| `logfile` | `-o logfile -f run.log` | `--file` | Write human-readable log file |
### Local Carbon & Cost Calculations

On completion of a run, all exporters write/display a summary of total duration, component-wise energy consumption, carbon footprint, and financial cost.

- **Energy Units**: All component and total energy metrics in the summary are converted and shown in **Watt-hours (Wh)** and **Kilowatt-hours (kWh)** for high readability.
- **Carbon Footprint Calculation**: Computed using the grid carbon intensity ($gCO_2eq/kWh$). By default, it falls back to the IEA global average of $445.0\text{ g/kWh}$. You can override this using the `--intensity <G/KWH>` flag.
- **Financial Cost Calculation**: Computed if the `--price <COST>` flag is supplied (in currency units per kWh).
- **Validation**: Financial cost and price inputs are only required when using the `online` exporter (which syncs config with the web platform database). Local runs (`terminal`, `csv`, `json`, `logfile`, `prometheus`) start instantly without requiring price configuration parameters.

### Options Reference

```
ecocode run [OPTIONS] -- <COMMAND> [ARGS...]

Options:
  -p, --project <NAME>       Project name for grouping runs [default: ecocode]
  -n, --name <NAME>          Human-readable label for this run
      --collaborator <NAME>  Person running this experiment
  -o, --output <FORMAT>      Export format [default: terminal]
  -f, --file <PATH>          Output file path (required for csv/json/logfile)
  -i, --interval <SECS>      Sampling interval in seconds [default: 1.0]
      --intensity <G/KWH>    Override carbon intensity in gCO2eq/kWh
      --price <COST>         Electricity cost per kWh
      --address <LOCATION>   Address or city for automatic intensity lookup
      --use-api-cost         Fetch live electricity prices from API
      --api-key <KEY>        API key for the online exporter
  -h, --help                 Print help
  -V, --version              Print version
```

### Config Subcommand

```bash
# Save API key to ~/.ecocode/config.json (avoids passing it on every run)
ecocode config set-api-key eco_...

# Show current configuration (API key is masked)
ecocode config show

# Remove the saved API key
ecocode config clear-api-key
```

API key resolution order: `--api-key` flag → `ECOCODE_API_KEY` env var → `~/.ecocode/config.json`.

---

## Exporters In Depth

### Terminal (`terminal.rs`)

Prints a header row on the first record and appends one row per interval. Indented sub-rows show per-GPU and per-socket breakdowns. `export()` (called once on exit) re-prints the full table as a run summary.

### CSV (`csv.rs`)

Appends rows to a CSV file with these columns:
`ID, RunID, PID, Timestamp, CPU%, CPU(W), GPU%, GPU(W), MEM%, MEM(W), IGPU%, IGPU(W), [GPU_N_usage, GPU_N_power, ...], Phase, Tokens, Step, Loss, LR, SeqLen`

Energy fields are stored in Joules in `Record` but displayed as Watts (`J / elapsed_secs`) in all text-based outputs.

### Online (`online.rs`)

Posts each `Record` as JSON to `POST /api/ingest/run/{id}/record` with `Authorization: Bearer <api_key>`. Failures are returned as `Err` and bubble up through the sampling loop (causing exit with a non-zero status). The run is created at startup via `POST /api/ingest/run` and finalised via `POST /api/ingest/run/{id}/complete`.

### Prometheus (`prometheus.rs`)

Spawns a `warp` HTTP server in a background `tokio::spawn` task. Metrics are updated via `gauge.set()` on every `add_record()` call. Available metrics:

| Metric | Labels | Description |
|--------|--------|-------------|
| `ecocode_cpu_usage` | — | CPU usage % |
| `ecocode_cpu_energy_watts` | — | CPU power draw (W) |
| `ecocode_gpu_usage` | — | Aggregate GPU usage % |
| `ecocode_gpu_energy_watts` | — | Aggregate GPU power (W) |
| `ecocode_mem_usage` | — | Memory usage % |
| `ecocode_mem_energy_watts` | — | DRAM power (W) |
| `ecocode_igpu_usage` | — | iGPU usage % |
| `ecocode_igpu_energy_watts` | — | iGPU power (W) |
| `ecocode_gpu_usage_per_device` | `device_index` | Per-GPU usage % |
| `ecocode_gpu_energy_watts_per_device` | `device_index` | Per-GPU power (W) |
| `ecocode_socket_energy_watts_per_domain` | `socket`, `domain` | Per-socket RAPL power (W) |



## Energy Attribution Model

EcoCode attributes a share of each hardware domain's total energy draw to the monitored process and its children:

| Domain | Attribution factor |
|--------|--------------------|
| CPU core | `cpu_usage %` / 100 |
| DRAM | `mem_usage %` / 100 |
| iGPU (RAPL uncore) | `cpu_usage %` / 100 (proxy — no per-process iGPU sensor exists) |
| GPU (NVML) | Per-process SM utilization from `process_utilization_stats()` |

The process tree (root PID + all descendants, found recursively via `sysinfo`) is used so that data-loader workers, `torch.distributed` sub-processes, and forked interpreters are included in the attribution.

CPU% is normalised by the number of logical cores so that a single-threaded process on a 128-core machine correctly reports ~0.78% rather than 100%.

---

## Carbon Footprint Calculation

```
CO2 (g) = total_energy_wh / 1000 × intensity_gCO2_per_kwh × PUE
```

- `total_energy_wh = (cpu_j + gpu_j + mem_j + igpu_j) / 3600`
- `intensity_gCO2_per_kwh`: resolved from project config → static table → global average (445 g/kWh, IEA 2025)
- `PUE`: Power Usage Effectiveness (1.0 for workstations, ~1.58 for average data centres)

The static table in `carbon.rs` covers 50+ countries with IEA 2025 data, looked up by ISO 3166-1 alpha-2 country code.

---

## Sensor Details

### RAPL (CPU + DRAM + iGPU)

RAPL counters are exposed by the Linux kernel at:

```
/sys/class/powercap/intel-rapl/
├── intel-rapl:0/           ← socket 0
│   ├── energy_uj           ← total CPU energy (μJ, monotonically increasing)
│   ├── max_energy_range_uj ← counter ceiling (used for wrap-around detection)
│   ├── intel-rapl:0:0/     ← socket 0, "core" sub-domain
│   │   └── energy_uj
│   ├── intel-rapl:0:1/     ← socket 0, "uncore" / iGPU sub-domain (if present)
│   │   └── energy_uj
│   └── intel-rapl:0:2/     ← socket 0, "dram" sub-domain (if present)
│       └── energy_uj
└── intel-rapl:1/           ← socket 1 (dual-socket systems)
    └── ...
```

`scan_rapl_files()` discovers all available sockets and domains at startup and opens a persistent `BufReader<File>` per counter, avoiding repeated `open()` syscalls. The file is seeked to position 0 before each read because the kernel updates it in-place.

**Wrap-around correction:** The counters are 32-bit or 64-bit integers that reset to 0 when they reach `max_energy_range_uj`. EcoCode tracks the running maximum and adds the ceiling when a backward jump is detected: `delta = (max - prev) + curr`.

> [!IMPORTANT]
> **Root & Permission Requirements:** RAPL counters are typically readable only by root. At startup, the CLI automatically runs `sudo chmod -R +r /sys/class/powercap/intel-rapl/` to make them world-readable for the duration of the monitoring session.

### NVML (GPU)

NVML (`nvml_wrapper` Rust crate) provides:

- `device.total_energy_consumption()` — cumulative GPU board energy in mJ. Like RAPL, this is a monotonically increasing counter that wraps.
- `device.process_utilization_stats(last_timestamp)` — SM utilisation % per process since `last_timestamp`. The timestamp is stateful: passing the timestamp from the previous call returns only new samples.

GPU energy is attributed proportionally: if a process has 40% SM utilisation on a GPU that consumed 100 J in the last second, it is attributed 40 J.

---

## Third-Party Dependencies

All Rust crates used by this component (`clap`, `tokio`, `serde`, `sqlx`, `reqwest`, `warp`, `prometheus`, `sysinfo`, `nvml-wrapper`, and others) are licensed under MIT and/or Apache-2.0. These are permissive licenses that allow commercial use, modification, and distribution without restriction.

**NVIDIA NVML:** The `nvml-wrapper` crate is MIT / Apache-2.0. The underlying NVML shared library (`libnvidia-ml.so`) is part of the NVIDIA driver and is loaded dynamically at runtime. EcoCode does not ship or redistribute this library — it must be present on the host as part of the user's NVIDIA driver installation.

See the [root README](../README.md#third-party-dependencies) for the full dependency audit.

---