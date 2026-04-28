# EcoCode CLI — Architecture

```mermaid
flowchart TD
    User(["sudo ecocode -- &lt;command&gt;"])

    subgraph Entry["Entry"]
        cli["cli.rs\nParses all CLI flags with clap\n--output  --interval  --project\n--api-key  --intensity  --price"]
        main["main.rs\nRoutes to the monitoring loop\nor config subcommands\nset-api-key · clear-api-key · show"]
    end

    subgraph Core["Core — monitor.rs"]
        monitor["monitor.rs\nSpawns the target process\nRuns the per-tick sampling loop\nAttributes energy proportionally to the PID\nWrites the final run summary on exit"]
    end

    subgraph Sensors["Sensors — sensor/"]
        rapl["rapl.rs\nReads /sys/class/powercap/intel-rapl/\nPer-socket energy in microjoules\nacross cpu · dram · uncore domains\nDetects and corrects counter wrap-around"]
        gpu["gpu.rs\nNVML wrapper for all NVIDIA GPUs\nPer-GPU total energy and utilisation\nPer-process GPU attribution via NVML"]
    end

    subgraph RecordEngine["Record Assembly"]
        carbon["carbon.rs\nStatic lookup table — 50 countries\n(IEA 2025 gCO2eq / kWh)\nIndexed by ISO 3166-1 alpha-2 code"]
        exp_mod["exporter/mod.rs\nBuilds the Record struct each tick:\ncpu_energy · gpu_energy · mem_energy\nper_socket[] · per_gpu[]\nco2_g · timestamp · pid"]
    end

    subgraph Exporters["Exporters — exporter/"]
        exp_t["terminal.rs\nPrints a formatted table to stdout"]
        exp_c["csv.rs\nAppends a row to a CSV file"]
        exp_j["json.rs\nWrites a JSON array to a file"]
        exp_l["local.rs\nInserts records into a local SQLite database"]
        exp_o["online.rs\nHTTP POST to the web backend\nAuthenticated with a Bearer API key"]
        exp_p["prometheus.rs\nExposes a /metrics endpoint on port 9091"]
        exp_f["logfile.rs\nWrites a human-readable log file"]
    end

    User --> cli --> main --> monitor
    monitor --> rapl & gpu
    rapl & gpu --> exp_mod
    monitor --> carbon --> exp_mod
    exp_mod --> exp_t & exp_c & exp_j & exp_l & exp_o & exp_p & exp_f
```

## How it works

1. `cli.rs` parses the command-line flags and `main.rs` decides whether to enter the monitoring loop or handle a `config` subcommand.
2. `monitor.rs` spawns the target process and enters a tick loop. On every interval it calls both sensor modules in parallel.
3. `rapl.rs` reads the Linux RAPL sysfs files and returns per-socket, per-domain energy deltas in joules. `gpu.rs` queries NVML for per-GPU energy and attributes a fraction to the monitored PID based on utilisation.
4. `exporter/mod.rs` assembles a `Record` from the sensor readings. `carbon.rs` enriches it with a CO2 estimate derived from the configured grid intensity.
5. The selected exporter writes or transmits the record. Multiple exporters can be active at once. When the target process exits, `monitor.rs` writes a final summary.

## File reference

| File | Responsibility |
|------|----------------|
| `cli.rs` | All CLI flag definitions (clap) |
| `main.rs` | Entry point and subcommand dispatch |
| `monitor.rs` | Process spawning, sampling loop, PID energy attribution, run summary |
| `carbon.rs` | Static 50-country CO2 intensity table (IEA 2025) |
| `sensor/mod.rs` | Sensor module hub, defines `RAPL_PATH` |
| `sensor/rapl.rs` | Intel RAPL sysfs reader — CPU, DRAM, iGPU energy per socket |
| `sensor/gpu.rs` | NVML wrapper — per-GPU and per-process energy and utilisation |
| `exporter/mod.rs` | `Record` struct definition and exporter trait |
| `exporter/terminal.rs` | Stdout table output |
| `exporter/csv.rs` | CSV file output |
| `exporter/json.rs` | JSON file output |
| `exporter/local.rs` | SQLite local storage |
| `exporter/online.rs` | HTTP POST to the web backend (premium) |
| `exporter/prometheus.rs` | Prometheus `/metrics` endpoint |
| `exporter/logfile.rs` | Formatted log file output |
