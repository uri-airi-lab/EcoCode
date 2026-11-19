# EcoCode CLI

`ecocode` is a Rust binary that runs any command and samples the energy its process tree uses,
on Linux, macOS and Windows. It needs no changes to the program being measured. When the program
uses the Python SDK, the CLI also picks up its ML markers (phase, step, loss, tokens) and stores
them alongside each energy sample.

## How it works

```
ecocode run [OPTIONS] -- <COMMAND> [ARGS...]
```

`ecocode run` creates a run in the selected exporter, detects the energy sensors on the machine,
starts the command as a child process and then samples on a fixed interval (default 1 s). Each
sample reads the hardware energy counters, the CPU and memory use of the command's process tree,
and the latest marker values, attributes a share of the energy to the process tree, and hands a
record to the exporter. When the command exits (or on Ctrl+C) the exporter is flushed and a
summary is printed. A warm-up pass of every sensor before the command starts keeps start-up costs
such as CUDA context creation out of the first interval.

The child inherits two environment variables: `ECOCODE_RUN_ID` and a marker pipe
(`ECOCODE_MARKER_FD` on Linux and macOS, `ECOCODE_MARKER_HANDLE` on Windows). The Python SDK
detects them, skips its own monitoring and writes markers to the pipe as newline-delimited JSON.

The sampling loop, the attribution rules and the per-OS sensors are described in
[architecture.md](architecture.md) and [platforms/](platforms/).

## Install

The simplest route is the Python package, whose wheels bundle the binary:

```bash
pip install ecocodes
ecocode --version
```

To build from source you need Rust (`rust-toolchain.toml` pins the version via rustup):

```bash
cargo build --release
sudo cp target/release/ecocode /usr/local/bin/   # optional
```

Native installers per OS are described in [packaging/README.md](../packaging/README.md).

NVIDIA GPU energy comes from NVML, loaded at runtime from the driver. Without an NVIDIA driver
the CLI still measures CPU and memory. On Linux the RAPL counters are root-only by default; run
`sudo packaging/linux/install-permissions.sh` once to make them readable by your user (see
[platforms/linux.md](platforms/linux.md)). The CLI never runs `sudo` itself.

Run `ecocode doctor` first to see which sensors work on your machine.

## Usage

```bash
# Live table in the terminal
ecocode run -- python train.py

# Sample every 0.5 s and label the run
ecocode run --interval 0.5 --project efficiency-study --name fp32-baseline -- python train.py

# Write every sample to a file
ecocode run -o csv -f results.csv -- python train.py
ecocode run -o json -f results.json -- python train.py
ecocode run -o logfile -f run.log -- python train.py

# Send to the web dashboard (carbon intensity and price come from the project in the web app)
ecocode run -o online --api-key ecocode_... --project my-project -- python train.py

# Prometheus gauges on http://localhost:9091/metrics
ecocode run -o prometheus -- python train.py

# Local outputs: set the grid carbon intensity (gCO2eq/kWh) and electricity price (per kWh)
ecocode run --intensity 52 --price 0.12 -- python train.py

# Any language works
ecocode run -- ./my_simulation
ecocode run -- node server.js
```

### Options

| Option | Default | Meaning |
|---|---|---|
| `-p, --project <NAME>` | `ecocode` | Project that groups runs |
| `-n, --name <NAME>` | timestamp | Label for this run |
| `--collaborator <NAME>` | | Person running the experiment |
| `-o, --output <FORMAT>` | `terminal` | `terminal`, `csv`, `json`, `online`, `prometheus` or `logfile` |
| `-f, --file <PATH>` | | Output file, required for `csv`, `json` and `logfile` |
| `-i, --interval <SECS>` | `1.0` | Sampling interval; sub-second values are allowed |
| `--intensity <G/KWH>` | 445 | Grid carbon intensity in gCO2eq/kWh (local outputs only) |
| `--price <COST>` (alias `--cost`) | | Electricity price per kWh, in your currency (local outputs only) |
| `--api-key <KEY>` | | API key for the `online` exporter |
| `-h, --help`, `-V, --version` | | |

The sensor set can be switched with the hidden `--sensors` flag or the `ECOCODE_SENSORS`
variable: `auto` (default), `fake` (synthetic sensors for tests on machines without hardware
access) or `none`. `fake` is also accepted by `doctor`.

### Other commands

| Command | Purpose |
|---|---|
| `ecocode doctor [--json]` | Report which sensors work on this machine and how to fix the others |
| `ecocode config set-api-key <KEY>` | Store an API key in `~/.ecocode/config.json` |
| `ecocode config clear-api-key` | Remove the stored key |
| `ecocode config show` | Print the configuration with the key masked |
| `ecocode monitor`, `ecocode monitor-local` | Headless monitors started by the Python SDK; not meant to be run by hand |

The `online` exporter resolves its key in this order: `--api-key`, then `ECOCODE_API_KEY`, then
the config file.

## Exporters

Energy is sampled in joules and shown as watts (joules divided by the interval) in all text
outputs. Every exporter prints the same run summary at the end: duration, CPU, GPU and memory
energy in Wh and kWh, carbon footprint in gCO2eq and, when a price is known, electricity cost.

`terminal` prints a header and one row per sample to stdout, with indented sub-rows for each GPU
and each CPU socket. It is the default and needs no configuration.

`csv` appends one row per sample to `--file` and flushes after every row, so the file is usable
while the run is still going. Columns: `ID`, `Run ID`, `PID`, `Timestamp`, `CPU%`, `CPU(W)`,
`GPU%`, `GPU(W)`, `MEM%`, `MEM(W)`, `IGPU%`, `IGPU(W)`, then `GPU<N>%` and `GPU<N>(W)` per
device, then `Phase`, `TokensProcessed`, `TrainingStep`, `Loss`, `LearningRate` and
`SequenceLength`.

`json` writes the records to `--file` as one JSON array, which is complete once the run ends.

`logfile` writes a human-readable log to `--file`, one block per sample.

`online` posts each record to https://ecocode.harmonyhpc.io with `Authorization: Bearer <key>`.
The backend address is fixed and cannot be changed by flags, environment variables or the config
file. Records are queued and uploaded by a background thread so a slow backend never delays
sampling; if more than 300 records are waiting the oldest are dropped and the count is reported at
the end. The run is created at start-up and marked finished, failed or canceled after the queue
is flushed (up to 10 s).

`prometheus` serves gauges on `0.0.0.0:9091/metrics` from a background thread and fails at
start-up if the port is taken. Gauges cover CPU, GPU, memory and iGPU usage and power, per-device
GPU values (label `device_index`) and per-socket RAPL power (labels `socket`, `domain`), all
prefixed `ecocode_`.

## Carbon and cost

```
CO2 (g) = total energy (kWh) x intensity (gCO2eq/kWh) x PUE
```

Total energy is the sum of CPU, GPU, memory and iGPU energy. For local exporters the intensity
is `--intensity` if given, otherwise the IEA global average of 445 gCO2eq/kWh, and the PUE is
1.0. Cost is shown only when `--price` is given.

With the `online` exporter the intensity, price and PUE come from the project's Environmental
settings in the web app, and `--intensity` and `--price` are rejected. There, a project uses the
Ember grid intensity of its country or US state (picked, or detected from an address) or a value
you enter yourself. Projects are created in the web app too; a run whose project has no price yet
still records energy and carbon, without cost. `--address` and `--use-api-cost` are not supported;
passing either stops the run with a message that points to the project settings.

## Third-party dependencies

The Rust crates the CLI depends on (`clap`, `serde`, `reqwest`, `rustls`, `tiny_http`,
`prometheus`, `sysinfo`, `nvml-wrapper` and others) are licensed under MIT and/or Apache-2.0.
NVML itself (`libnvidia-ml.so`, `nvml.dll`) is part of the NVIDIA driver; EcoCode loads it at
runtime and does not redistribute it. See the [root README](../README.md) for the licence of
EcoCode itself.
