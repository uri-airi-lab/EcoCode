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

## How EcoCode Differs from Existing Tools

Most energy monitoring tools for software rely on estimation: they multiply hardware TDP by CPU utilisation and treat the result as consumed energy. EcoCode takes a different approach — it reads what the hardware actually reports.

### Measurement accuracy

CodeCarbon, Carbontracker, and similar Python libraries estimate power from manufacturer TDP figures. EcoCode reads Intel RAPL sysfs counters (`/sys/class/powercap/intel-rapl/`) and NVIDIA NVML directly. These are the same interfaces used by hardware performance engineers and power-management firmware — they reflect real joules drawn, not modelled approximations.

### Per-process GPU attribution

Most tools that claim GPU monitoring report whole-device power and divide it evenly, or skip it entirely. EcoCode uses NVML `process_utilization_stats` to attribute GPU energy to the specific monitored PID proportionally, across every detected GPU independently.

### Language and runtime agnostic

CodeCarbon requires Python and instruments code through decorators or context managers. EcoCode wraps any executable — `ecocode -- python train.py`, `ecocode -- ./simulation`, `ecocode -- node server.js` — with no changes to the target program.

### Comparison table

| Capability | EcoCode | CodeCarbon | Scaphandre | Carbontracker |
|---|:---:|:---:|:---:|:---:|
| Real hardware energy (RAPL) | Yes | No | Yes | No |
| Real GPU energy (NVML) | Yes | No | No | No |
| Per-process GPU attribution | Yes | No | No | No |
| Multi-GPU per-device breakdown | Yes | No | No | No |
| Multi-socket per-domain breakdown | Yes | No | No | No |
| Any language / any process | Yes | Python only | Yes | Python only |
| Live real-time dashboard (SSE) | Yes | No | No | No |
| Team workspaces and collaboration | Yes | No | No | No |
| Prometheus export | Yes | No | Yes | No |
| Live grid carbon intensity (API) | Yes | Partial | No | No |
| Counter wrap-around correction | Yes | No | No | No |

Scaphandre is the closest alternative on the measurement side — it is also RAPL-based — but it has no GPU support, no web dashboard, no carbon estimation, and no developer workflow integration. It is designed as a production infrastructure exporter, not a per-experiment tool.

---

## Architecture

The system has three independent layers: a Rust CLI that wraps any process and reads hardware energy counters, an Axum REST backend that stores and serves measurements, and a Next.js frontend that visualises runs in real time via Server-Sent Events.

```mermaid
flowchart TD
    User(["User\nsudo ecocode -- python train.py"])

    subgraph CLI["EcoCode CLI — ecocode/src/"]
        direction TB
        cli["cli.rs\nDefines all CLI flags via clap:\n--output  --interval  --project\n--api-key  --intensity  --price"]
        main_cli["main.rs\nEntry point: routes to monitor loop\nor config subcommands\n(set-api-key / clear-api-key / show)"]
        monitor["monitor.rs\nSpawns target process\nRuns per-tick sampling loop\nAttributes energy to monitored PID\nWrites final run summary"]
        carbon_tbl["carbon.rs\n50-country static CO2 table\n(IEA 2025 gCO2eq/kWh)\nLookup by ISO 3166-1 alpha-2 code"]

        subgraph Sensors["sensor/"]
            rapl["rapl.rs\nReads /sys/class/powercap/intel-rapl/\nPer-socket energy in μJ:\ncpu · dram · uncore (iGPU domains)\nHandles counter wrap-around"]
            nvml["gpu.rs\nNVML wrapper — all NVIDIA GPUs\nPer-GPU total energy + utilisation\nPer-process GPU attribution"]
        end

        exp_mod["exporter/mod.rs\nDefines the Record struct:\ncpu_energy · gpu_energy · mem_energy\nper_socket[] · per_gpu[]\nco2_g · timestamp · pid"]

        subgraph Exporters["exporter/"]
            exp_t["terminal.rs\nFormatted table to stdout"]
            exp_c["csv.rs\nAppend rows to CSV file"]
            exp_j["json.rs\nWrite JSON array to file"]
            exp_l["local.rs\nInsert into local SQLite database"]
            exp_o["online.rs\nHTTP POST to /api/ingest\nAuthenticated with Bearer API key"]
            exp_p["prometheus.rs\nExpose /metrics on HTTP port 9091"]
            exp_f["logfile.rs\nWrite formatted log file"]
        end
    end

    subgraph Backend["Web Backend — web/backend/src/"]
        direction TB
        main_be["main.rs\nAxum server on :3001\nConnects PostgreSQL pool\nRuns schema migrations\nRegisters CORS + all routes\nSpawns SSE broadcast task"]

        subgraph BAuth["auth/"]
            a_mod["mod.rs\nRoutes: POST /register · POST /login\nGET/POST /api-keys\nSets 30-day HttpOnly JWT cookie"]
            a_jwt["jwt.rs\nSign and verify JWT tokens"]
            a_mw["middleware.rs\nAuthUser extractor:\naccepts JWT cookie or Bearer API key"]
            a_pw["password.rs\nArgon2id password hashing\nSHA-256 API key hashing"]
        end

        subgraph BAPI["api/"]
            ingest["ingest.rs\nPOST /api/ingest/run — create run\nPOST /run/{id}/record — append tick\nPOST /run/{id}/system-info — hardware\nPOST /run/{id}/complete — finalise"]
            runs_api["runs.rs\nGET runs · records · summaries\nfor a workspace or project"]
            proj_api["projects.rs\nCRUD endpoints for projects\n(workspace-scoped)"]
            sse_h["sse.rs\nGET /api/sse\nBroadcasts each new Record\nto all connected clients"]
            carbon_h["carbon.rs\nFetch / save carbon config\nProxy to Electricity Maps API\nfor live grid intensity"]
            admin_h["admin.rs\nAdmin-only: list runs\nacross all workspaces"]
        end

        subgraph BDB["db/"]
            schema_db["schema.rs\nCreates all tables on startup:\nusers · workspaces · workspace_members\napi_keys · projects · runs · records\nrecord_gpu_metrics · record_socket_metrics\nrun_summaries"]
            rec_db["records.rs\nInsert measurement records\nwith GPU and socket breakdowns\nQuery records for a run"]
            run_db["runs.rs\nRun CRUD: create · update status\nfetch by id / workspace / project"]
        end
    end

    subgraph Frontend["Web Frontend — web/frontend/"]
        direction TB
        subgraph FPages["app/ — Pages (Next.js App Router)"]
            run_pg["runs/[id]/page.tsx\nLive run dashboard:\ncharts + system info + carbon summary"]
            runs_list["runs/page.tsx\nPaginated run list with filters"]
            proj_pg["projects/[id]/page.tsx\nProject detail and run history"]
            auth_pg["login/page.tsx + register/page.tsx\nJWT authentication forms"]
            ws_pg["[workspaceSlug]/page.tsx\nWorkspace home: recent runs\nand summary stats"]
            settings_pg["settings/developer/page.tsx\nCreate and revoke API keys"]
        end

        subgraph FCharts["components/charts/"]
            ec["EnergyChart.tsx\nCumulative energy over time\n(Recharts line chart)"]
            pd["PowerDistributionChart.tsx\nCPU vs GPU vs RAM split\n(stacked area)"]
            sc["SocketEnergyChart.tsx\nPer-socket RAPL breakdown"]
            uc["UtilizationChart.tsx\nCPU + GPU utilisation % over time"]
            cc["CumulativePowerChart.tsx\nRunning total power draw"]
        end

        sse_hook["hooks/use-sse.ts\nSubscribes to GET /api/sse\nAppends incoming records to\nchart state in real time"]

        api_lib["lib/api/\nruns.ts · projects.ts · carbon.ts · admin.ts\nTyped fetch wrappers — all calls\nsend the JWT cookie automatically"]
    end

    DB[("PostgreSQL — Neon / TimescaleDB\nruns · records\nrecord_gpu_metrics\nrecord_socket_metrics\nrun_summaries\nworkspaces · users · api_keys · projects")]

    User --> cli --> main_cli --> monitor
    monitor --> rapl & nvml
    monitor --> carbon_tbl
    rapl & nvml --> exp_mod
    carbon_tbl --> exp_mod
    exp_mod --> exp_t & exp_c & exp_j & exp_l & exp_o & exp_p & exp_f

    exp_o -- "POST /api/ingest/{id}/record\nBearer API key" --> ingest
    ingest --> rec_db --> DB
    ingest --> run_db --> DB

    schema_db -. "runs once at startup" .-> DB

    sse_h -- "SSE stream\n(new records)" --> sse_hook
    sse_hook --> run_pg
    run_pg --> ec & pd & sc & uc & cc

    api_lib -- "REST + JWT cookie" --> runs_api & proj_api & carbon_h
    runs_api --> run_db
    proj_api --> run_db

    auth_pg --> a_mod --> a_jwt & a_pw
    a_mw -. "validates every\nprotected request" .-> BAPI
```

### Data flow summary

1. The user runs `ecocode -- <command>`. `cli.rs` parses flags; `monitor.rs` spawns the child process and starts sampling.
2. On every tick, `rapl.rs` reads RAPL sysfs counters and `gpu.rs` queries NVML. `exporter/mod.rs` assembles a `Record` struct and optionally enriches it with CO2 via `carbon.rs`.
3. The selected exporter writes the record. With `--output online`, `online.rs` sends it to the backend via an authenticated HTTP POST.
4. The Axum backend (`ingest.rs`) inserts the record and its per-GPU and per-socket breakdowns into PostgreSQL, then broadcasts it to `sse.rs`.
5. The Next.js frontend holds an open SSE connection (`use-sse.ts`). Each incoming record is appended to chart state, so the run dashboard updates live without polling.

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
