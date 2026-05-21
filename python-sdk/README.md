# EcoCode Python SDK

> Track energy consumption, carbon footprint, and ML training metrics for any Python ML workload — with or without root access.

---

## Table of Contents

- [Overview](#overview)
- [How It Works](#how-it-works)
  - [Two Operating Modes](#two-operating-modes)
  - [Mode Detection](#mode-detection)
  - [Execution Flow](#execution-flow)
- [Architecture Deep Dive](#architecture-deep-dive)
  - [`__init__.py` — Public API surface](#initpy--public-api-surface)
  - [`_run.py` — EcoCodeRun class](#_runpy--ecoCoderun-class)
  - [`_pipe.py` — Pipe IPC writers](#_pipepy--pipe-ipc-writers)
  - [`_http.py` — HTTP client](#_httppy--http-client)
  - [`ecocode monitor` — Headless monitor subprocess](#ecocode-monitor--headless-monitor-subprocess)
  - [`ecocode-core` — Shared energy sensor logic](#ecocode-core--shared-energy-sensor-logic)
  - [How the SDK communicates with the monitor](#how-the-sdk-communicates-with-the-monitor)
- [Installation](#installation)
- [Quick Start](#quick-start)
  - [Standalone Mode (no sudo required)](#standalone-mode-no-sudo-required)
  - [Attached Mode (CLI + SDK)](#attached-mode-cli--sdk)
  - [Explicit Start/Stop](#explicit-startstop)
  - [Decorator API](#decorator-api)
- [ML Marker API](#ml-marker-api)
  - [Phase Labels](#phase-labels)
  - [Available Methods](#available-methods)
- [Integrations](#integrations)
  - [HuggingFace Transformers](#huggingface-transformers)
  - [TRL (SFT / DPO / GRPO)](#trl-sft--dpo--grpo)
  - [PyTorch Lightning](#pytorch-lightning)
  - [DeepSpeed](#deepspeed)
  - [vLLM Inference](#vllm-inference)
  - [Keras / TensorFlow](#keras--tensorflow)
  - [ONNX Runtime](#onnx-runtime)
  - [Axolotl / Unsloth](#axolotl--unsloth)
- [Environment Variables](#environment-variables)
- [What Gets Measured](#what-gets-measured)
- [Environmental Impact Equivalents](#environmental-impact-equivalents)
- [Running the Demo](#running-the-demo)
- [Package Metadata](#package-metadata)

---

## Overview

The EcoCode Python SDK lets you instrument any ML training or inference script to measure real-time energy consumption and send structured telemetry to the [EcoCode backend](../web). It is designed to **never crash your training loop**: all HTTP failures and measurement errors are caught and printed to stderr.

```python
import ecocode

with ecocode.run("llama-finetune", api_key="eco_...", project_id=42) as run:
    run.mark("phase", "forward")
    run.log_step(100)
    run.log_loss(1.45)
```

The SDK is implemented as a pure Python library with zero compiled dependencies. Energy monitoring is handled by the `ecocode` CLI binary spawned as a subprocess — the Python process contains no native threads, making it fully fork-safe for PyTorch DataLoader and DDP workloads.

---

## How It Works

### Two Operating Modes

The SDK operates in one of two mutually exclusive modes, **auto-detected at construction time** based on environment variables.

#### Standalone Mode

> **No CLI wrapper needed. Install with `pip install ecocode` and go.**

The SDK manages the entire run lifecycle itself:

1. Creates a run record on the EcoCode backend via HTTP (`POST /api/ingest/run`).
2. Collects system info (CPU, RAM, GPU names) and sends it via HTTP.
3. Spawns the `ecocode monitor` binary as a **subprocess** (not a thread). This subprocess:
   - Reads **GPU energy** from NVIDIA NVML (`nvmlDeviceGetTotalEnergyConsumption`) — real joule deltas.
   - Reads **CPU energy** from RAPL sysfs (`/sys/class/powercap/intel-rapl/*/energy_uj`).
   - Reads **DRAM energy** from the RAPL `dram` sub-domain, if the CPU exposes it.
   - Reads **CPU % and memory PSS** from `/proc/[pid]/stat` and `/proc/[pid]/smaps_rollup`.
   - Reads markers from its stdin pipe, merges them with sensor data, and POSTs combined records to the backend.
4. `run.log_loss(1.45)` writes `{"event":"loss","value":1.45}\n` to the subprocess's stdin pipe — a single kernel buffer write taking ~1 microsecond. The Python process has **zero native threads**, making it fully fork-safe.
5. On `__exit__` (or `.stop()`), the SDK closes the pipe, waits for the subprocess to flush its final record and exit, then marks the run `finished` or `failed`.

> [!TIP]
> **CPU Energy Permissions:** GPU energy (NVML) is always available without root. However, CPU energy requires read access to the RAPL sysfs interface. On systems where these files are not world-readable by default, run the following commands once to enable CPU energy tracking:
> ```bash
> sudo chmod o+r /sys/class/powercap/intel-rapl/*/energy_uj
> sudo chmod o+r /sys/class/powercap/intel-rapl/*/*/energy_uj
> ```

#### Attached Mode

> **Uses the CLI as the monitor instead of spawning a subprocess.**

```bash
ecocode run -o online --api-key eco_... -- python train.py
```

The CLI:
- Creates the run record in the backend.
- Creates an OS pipe and exports `ECOCODE_MARKER_FD` (write-end FD) + `ECOCODE_RUN_ID` into the child process environment.
- Spawns the user's Python process as a child.
- Handles all energy sampling and record reporting.

The SDK sees these variables and switches to attached mode:
- **No subprocess spawned.** The CLI is the monitor.
- **No HTTP calls from Python.** The SDK only sends lightweight semantic markers through the inherited pipe as newline-delimited JSON.
- Zero overhead on the training process beyond pipe writes.

**When to use standalone:**

- You are running on a cloud VM, a Jupyter notebook, a university cluster, or any environment where simplicity matters most.
- You want the simplest possible setup: `pip install ecocode`, add three lines of code, and go.
- You are using explicit `start()`/`stop()` across notebook cells or in a long-lived server.

**When to use attached mode:**

- You want the CLI's terminal output, export format selection (CSV, JSON, Prometheus, etc.), and run lifecycle features.
- You need the `canceled` run status — if the job is killed with Ctrl+C, the CLI catches the signal, saves all data collected so far, and marks the run `canceled` cleanly.
- You are running multi-node or multi-GPU jobs where a single CLI process monitoring the root launcher is cleaner.

**Mode comparison:**

| | Standalone | Attached |
|---|---|---|
| Who reads sensors | `ecocode monitor` subprocess (spawned by SDK) | CLI process (parent) |
| Marker IPC | Pipe (subprocess stdin) | Pipe (inherited FD) |
| Native threads in Python | None (fork-safe) | None (fork-safe) |
| HTTP calls from Python | Run create/complete only | None |
| Run lifecycle management | SDK creates and completes runs | CLI creates and completes runs |
| `canceled` status | Not applicable — SDK only knows `finished` / `failed` | Produced by CLI on Ctrl+C |

---

### Mode Detection

```
EcoCodeRun.__enter__()
       │
       ├─ RANK / LOCAL_RANK != 0? ──── yes ──► No-op mode (DDP non-zero rank)
       │
       ├─ ECOCODE_MARKER_FD set? ──── yes ──► Attached mode
       │  ECOCODE_RUN_ID    set?              (write markers to inherited pipe FD)
       │
       └─ no ──────────────────────────────► Standalone mode
                                              (requires api_key, spawns monitor subprocess)
```

This detection happens in `_run.py`:

```python
# DDP rank detection
rank = os.environ.get("RANK") or os.environ.get("LOCAL_RANK")
if rank is not None and int(rank) != 0:
    self._mode = "noop"
    return self

# Attached mode
env_run_id = os.environ.get("ECOCODE_RUN_ID")
env_marker_fd = os.environ.get("ECOCODE_MARKER_FD")
if env_run_id and env_marker_fd:
    self._mode = "attached"
    ...

# Standalone mode
self._mode = "standalone"
...
```

---

### Execution Flow

#### Standalone Mode — Step by Step

```
Python script                    ecocode monitor subprocess        EcoCode backend
────────────────────────────────────────────────────────────────────────────────────
ecocode.run().__enter__()
  │
  ├─ POST /api/ingest/run  ──────────────────────────────────────► creates run row
  ├─ POST /api/ingest/run/{id}/system-info  ────────────────────► stores CPU/GPU info
  │
  ├─ subprocess.Popen("ecocode monitor --run-id 42 --pid $$", stdin=PIPE)
  │                                   │
  │                                   │  monitor subprocess (every interval seconds):
  │                                   ├─ NVML: GPU energy + utilisation
  │                                   ├─ /proc/[pid]/stat: CPU ticks
  │                                   ├─ /proc/[pid]/smaps_rollup: PSS memory
  │                                   ├─ RAPL sysfs: CPU + DRAM energy
  │                                   ├─ read markers from stdin pipe
  │                                   └─ POST /api/ingest/run/{id}/record ──────► stores metric row
  │
run.mark("phase", "forward")     # writes JSON to subprocess stdin pipe (~1μs)
run.log_step(100)                # writes JSON to subprocess stdin pipe (~1μs)
run.log_loss(1.45)               # writes JSON to subprocess stdin pipe (~1μs)
  │
ecocode.run().__exit__()
  ├─ pipe.close()                # EOF signal → monitor flushes final record and exits
  ├─ waitpid()                   # wait for subprocess to finish
  └─ POST /api/ingest/run/{id}/complete ──────────────────────────────────────► marks run done/failed
```

#### Attached Mode — Step by Step

```
CLI (Rust)                       Python script              EcoCode backend
─────────────────────────────────────────────────────────────────────────────
ecocode run starts
  ├─ reads RAPL + NVML on its own timer
  ├─ POST /api/ingest/run ─────────────────────────────────► creates run
  ├─ creates OS pipe (read_fd, write_fd)
  ├─ export ECOCODE_MARKER_FD=<write_fd>
  ├─ export ECOCODE_RUN_ID=42
  └─ exec python train.py

                                 ecocode.run().__enter__()
                                   └─ InheritedFdWriter(fd)

                                 run.mark("phase", "forward")
                                   └─ pipe.write({"event":"phase","value":"forward"}\n)

CLI reads marker from pipe ─────►
  └─ attaches marker to next metric record sent to backend

                                 ecocode.run().__exit__()
                                   └─ pipe.close()

CLI detects EOF on pipe, marks run "canceled" (Ctrl+C) or "finished"/"failed"
```

---

## Architecture Deep Dive

### `__init__.py` — Public API surface

Exposes three symbols: `run()`, `track()`, and `EcoCodeRun`.

| Symbol | Type | Description |
|--------|------|-------------|
| `run(name, *, api_key, project_id, base_url, interval)` | Context manager factory | Returns an `EcoCodeRun`. Used with `with`, or call `.start()`/`.stop()` manually. |
| `track(name, *, api_key, project_id, base_url, interval)` | Decorator factory | Wraps a function in `EcoCodeRun`. |
| `EcoCodeRun` | Class | Re-exported for type-checking convenience. |

`track` is sugar over `run`. Internally it does:

```python
@wraps(fn)
def wrapper(*args, **kwargs):
    with EcoCodeRun(run_name, ...):
        return fn(*args, **kwargs)
```

---

### `_run.py` — EcoCodeRun class

The central object that manages the run lifecycle. Implements:

- `__enter__` / `start()`: DDP rank detection, mode detection (attached vs standalone), run creation via HTTP, subprocess spawning.
- `__exit__` / `stop()`: closes the marker pipe, waits for the subprocess to finish, completes the run. Returns `False` — exceptions from the training code are **never suppressed**. The run status is `"finished"` (no exception) or `"failed"` (exception propagated).
- All `log_*()` / `mark()` methods: write a JSON event to the pipe (subprocess stdin in standalone mode, inherited FD in attached mode).

---

### `_pipe.py` — Pipe IPC writers

Three writer implementations:

| Class | Used in | Mechanism |
|-------|---------|-----------|
| `PipeWriter` | Standalone mode | Writes to subprocess stdin (`subprocess.PIPE`) |
| `InheritedFdWriter` | Attached mode | Writes to inherited FD via `os.fdopen()` |
| `NullWriter` | DDP non-zero ranks, pre-enter state | Silent no-op |

All writers serialise markers as newline-delimited JSON and silently catch `BrokenPipeError`.

---

### `_http.py` — HTTP client

Pure Python HTTP client using `urllib.request` (zero external dependencies). Used only by the SDK for run lifecycle management:

- `create_run()` — `POST /api/ingest/run`
- `save_system_info()` — `POST /api/ingest/run/{id}/system-info`
- `complete_run()` — `POST /api/ingest/run/{id}/complete`

Record reporting is handled by the monitor subprocess, not the SDK.

---

### `ecocode monitor` — Headless monitor subprocess

The `ecocode monitor` subcommand (`monitor_headless.rs`) is the subprocess spawned by the SDK in standalone mode. It:

- Reads markers from stdin (newline-delimited JSON)
- Reads RAPL, NVML, `/proc` sensors on every tick
- Merges markers with energy data into a `Record`
- POSTs combined records to the backend via `EcoCodeHttpClient`
- Exits on stdin EOF (pipe closed by SDK) or target PID death

This runs as a separate OS process, not a thread — the Python process has zero native threads, making it safe for `os.fork()` (DataLoader workers, DDP).

---

### `ecocode-core` — Shared energy sensor logic

`ecocode-core` is a Rust library crate at `ecocode/crates/core/`. The CLI binary links against it statically. It provides:

- `sensor::rapl` — RAPL sysfs file handles, energy reads, delta computation with wrap-around correction.
- `sensor::gpu` — NVML device enumeration, energy delta computation, per-process GPU utilisation.
- `sensor::get_process_tree_pids_from_sys` — recursive PID-tree walker using `sysinfo`.
- `http_client::EcoCodeHttpClient` — blocking HTTP client for the four ingest endpoints.

---

### How the SDK communicates with the monitor

```
Python: import ecocode
           └── ecocode/__init__.py
                   └── from ecocode._run import EcoCodeRun  (pure Python)
                               │
                   ┌───────────┴───────────────────────────┐
                   │  _run.py  — run lifecycle, mode detect │
                   │  _pipe.py — JSON pipe writers          │
                   │  _http.py — stdlib HTTP client         │
                   └───────────┬───────────────────────────┘
                               │ subprocess.Popen (stdin=PIPE)
                               ▼
                   ┌───────────────────────────────────────┐
                   │  ecocode monitor (Rust binary)        │
                   │    monitor_headless.rs                │
                   │    → links ecocode-core (static)      │
                   │         sensor::rapl                  │
                   │         sensor::gpu  (NVML)           │
                   │         http_client                   │
                   └───────────────────────────────────────┘
```

**The payload shape posted every tick by the monitor:**

```json
{
  "pid": 12345,
  "timestamp": "2026-05-04T12:00:00.000Z",
  "cpu_usage": 87.3,
  "cpu_energy": 1.42,
  "gpu_usage": 0.0,
  "gpu_energy": 2.14,
  "mem_usage": 12.5,
  "mem_energy": 0.18,
  "igpu_usage": null,
  "igpu_energy": null,
  "gpu_metrics": [{"device_index": 0, "gpu_usage": 0.0, "gpu_energy": 2.14}],
  "socket_metrics": [{"socket": 0, "domain": "cpu", "energy": 1.42}],
  "phase": "forward",
  "tokens_processed": 102400,
  "training_step": 500,
  "loss": 1.23,
  "learning_rate": 0.0003,
  "sequence_length": 2048
}
```

---

## Installation

```bash
# Core SDK (pure Python, zero compiled dependencies)
pip install ecocode

# With HuggingFace Trainer callback
pip install ecocode[hf]

# With TRL callback (SFT / DPO / GRPO)
pip install ecocode[trl]

# With PyTorch Lightning callback
pip install ecocode[lightning]

# Everything
pip install ecocode[all]

# Development from source
git clone https://github.com/MohamedAziz-Khezami/EcoCode.git
cd EcoCode/python-sdk
pip install -e ".[dev]"
```

The SDK is a pure Python package with zero compiled dependencies. No Rust toolchain or Maturin needed.

**Optional runtime extras**:

| Extra | Package | Why |
|-------|---------|-----|
| `hf` | `transformers >= 4.30` | HuggingFace `TrainerCallback` base class |
| `trl` | `trl >= 0.7` | TRL SFT / DPO / GRPO trainers |
| `lightning` | `pytorch-lightning >= 2.0` | PyTorch Lightning `Callback` base class |
| `keras` | `keras >= 3.0` | Keras 3 / tf.keras `Callback` base class |
| `onnx` | `onnxruntime >= 1.16` | ONNX Runtime session wrapper |

---

## Quick Start

> **Not sure which mode to use?**
> - Most users → use **Standalone** (just `pip install ecocode` and go).
> - Want CLI terminal output, export formats, or run lifecycle control → use **Attached**.

### Standalone Mode

```python
import ecocode

with ecocode.run(
    "my-experiment",
    api_key="eco_...",          # or set ECOCODE_API_KEY env var
    project_id=42,              # optional if API key is project-scoped
    base_url="http://localhost:3001",  # default; override with ECOCODE_API_URL
    interval=1.0,               # seconds between metric samples
) as run:

    run.mark("phase", "data_loading")
    # ... load data ...

    for epoch in range(10):
        run.log_epoch(epoch)
        for step, batch in enumerate(loader):
            run.mark("phase", "forward")
            loss = model(batch)

            run.mark("phase", "backward")
            loss.backward()

            run.mark("phase", "optimizer_step")
            optimizer.step()

            run.log_step(step)
            run.log_loss(loss.item())
            run.log_lr(scheduler.get_last_lr()[0])
            run.log_tokens(tokens_processed_so_far)

    run.mark("phase", "done")
```

### Attached Mode (CLI + SDK)

Start the CLI first, then your script can use a minimal import:

```bash
ecocode run -o online --api-key eco_... -- python train.py
```

```python
# train.py — no api_key needed; CLI injects ECOCODE_MARKER_FD + ECOCODE_RUN_ID
import ecocode

with ecocode.run() as run:
    run.mark("phase", "forward")
    run.log_step(100)
```

### Explicit Start/Stop

For Jupyter notebooks or long-lived servers where a context manager does not fit:

```python
import ecocode

run = ecocode.run("notebook-experiment", api_key="eco_...")
run.start()

# ... training across multiple cells or a long-lived process ...

run.stop()
```

### Decorator API

```python
import ecocode

@ecocode.track("my-training-run", api_key="eco_...", project_id=42)
def train():
    # The whole function body runs inside an EcoCodeRun context
    for batch in loader:
        ...

train()
```

---

## ML Marker API

### Phase Labels

Use these consistently across runs to enable cross-run energy comparisons on the dashboard:

| Paradigm | Phases |
|----------|--------|
| **Pretraining / SFT** | `data_loading`, `tokenization`, `forward`, `backward`, `optimizer_step`, `eval`, `checkpoint_save`, `done` |
| **RLHF / DPO / GRPO** | `reward_model`, `policy_rollout`, `reference_forward`, `ppo_update`, `dpo_update`, `grpo_update`, `kl_penalty` |
| **Inference (LLM)** | `model_load`, `prefill`, `decode`, `kv_cache_ops`, `done` |
| **DeepSpeed** | `deepspeed_zero0_init`, `deepspeed_zero1_init`, `deepspeed_zero2_init`, `deepspeed_zero3_init` |

### Available Methods

| Method | Description |
|--------|-------------|
| `run.mark("phase", value)` | Set the current training phase label |
| `run.mark("epoch", value)` | Alias for `log_epoch(value)` |
| `run.log_epoch(epoch: int)` | Mark the start of an epoch. Stored as phase `epoch:<n>` |
| `run.log_step(step: int)` | Record the current optimizer step (gradient update count) |
| `run.log_loss(loss: float)` | Record the current training loss |
| `run.log_lr(lr: float)` | Record the current learning rate |
| `run.log_tokens(count: int)` | Record the cumulative number of tokens processed |
| `run.log_sequence_length(seq_len: int)` | Record the input sequence length (attention cost scales as O(n²)) |
| `run.log_accuracy(accuracy: float)` | Record the current model accuracy (0-100) |
| `run.log_custom(key: str, value: float)` | Record any custom numeric metric (gradient norm, perplexity, BLEU, reward score, batch size, etc.) |

---

## Integrations

All integration callbacks are **lazily imported** — the framework package is only required when you explicitly import the callback class. This prevents import errors when using only a subset of frameworks.

### HuggingFace Transformers

```python
import ecocode
from ecocode.integrations.huggingface import EcoCodeCallback
from transformers import Trainer

with ecocode.run("sft-llama3", project_id=42, api_key="eco_...") as eco_run:
    trainer = Trainer(
        model=model,
        args=training_args,
        callbacks=[EcoCodeCallback(eco_run)],
    )
    trainer.train()
```

**Hooks fired:**

| Trainer hook | Phase marked | Extra logging |
|---|---|---|
| `on_train_begin` | `data_loading` | `sequence_length` (if `max_seq_length` in args) |
| `on_epoch_begin` | — | `log_epoch(state.epoch)` |
| `on_step_begin` | `forward` | — |
| `on_optimizer_step` | `optimizer_step` | `step`, `loss`, `learning_rate` from log history |
| `on_evaluate` | `eval` | — |
| `on_save` | `checkpoint_save` | — |
| `on_train_end` | `done` | `tokens` from `state.num_input_tokens_seen` |

---

### TRL (SFT / DPO / GRPO)

TRL provides separate callbacks because each alignment paradigm has a fundamentally different energy profile:

```python
from ecocode.integrations.trl import EcoCodeSFTCallback   # for SFTTrainer
from ecocode.integrations.trl import EcoCodeDPOCallback   # for DPOTrainer
from ecocode.integrations.trl import EcoCodeGRPOCallback  # for GRPOTrainer
```

| Callback | Trainer | Energy profile |
|----------|---------|----------------|
| `EcoCodeSFTCallback` | `SFTTrainer` | `data_loading → forward → backward → optimizer_step → eval` |
| `EcoCodeDPOCallback` | `DPOTrainer` | `forward` = policy + reference forward together; `dpo_update` on optimizer step — ~2× energy vs SFT |
| `EcoCodeGRPOCallback` | `GRPOTrainer` | `policy_rollout` (autoregressive generation) → `grpo_update` (gradient step). Used by DeepSeek-R1, Qwen reasoning models. |

---

### PyTorch Lightning

```python
import ecocode
import pytorch_lightning as pl
from ecocode.integrations.lightning import EcoCodeCallback

with ecocode.run("pretrain", project_id=42, api_key="eco_...") as eco_run:
    trainer = pl.Trainer(
        callbacks=[EcoCodeCallback(eco_run)],
    )
    trainer.fit(model, datamodule=dm)
```

**Hooks:** `on_train_start` → `on_train_epoch_start` (logs epoch + `forward`) → `on_before_optimizer_step` (logs step, loss, lr) → `on_validation_epoch_start` (`eval`) → `on_save_checkpoint` → `on_train_end` (`done`).

---

### DeepSpeed

DeepSpeed does not have a `TrainerCallback` protocol, so the integration is a **manual hook object** you call explicitly at the right points in your loop:

```python
import ecocode
from ecocode.integrations.deepspeed import EcoCodeDeepSpeedCallback

with ecocode.run("pretrain-70b", project_id=42, api_key="eco_...") as eco_run:
    ds_cb = EcoCodeDeepSpeedCallback(eco_run, zero_stage=3)
    # → immediately marks phase "deepspeed_zero3_init"

    for step, batch in enumerate(loader):
        ds_cb.on_step_begin(step)          # phase = "forward"

        outputs = engine(batch)
        loss = outputs.loss

        ds_cb.on_backward_begin()          # phase = "backward"
        engine.backward(loss)
        ds_cb.on_backward_end()

        engine.step()
        ds_cb.on_optimizer_step(step, loss.item())  # phase = "optimizer_step"

    ds_cb.on_train_end()                   # phase = "done"
```

The `zero_stage` parameter is logged as a one-time marker so the dashboard can show communication overhead differences across ZeRO stages (0 → 3).

---

### vLLM Inference

Two helpers are available for inference energy tracking:

#### `EcoCodeVLLMMiddleware` — for AsyncLLMEngine

```python
import ecocode
from ecocode.integrations.vllm import EcoCodeVLLMMiddleware

engine = AsyncLLMEngine.from_engine_args(engine_args)

with ecocode.run("vllm-serving", project_id=42, api_key="eco_...") as eco_run:
    eco_engine = EcoCodeVLLMMiddleware(engine, eco_run)

    async for output in eco_engine.generate(prompt, sampling_params, request_id):
        yield output
```

Automatically marks `prefill` when generation starts, switches to `decode` on the first generated token, and logs cumulative token counts.

#### `InferencePhaseTracker` — for any inference loop

```python
from ecocode.integrations.vllm import InferencePhaseTracker

tracker = InferencePhaseTracker(eco_run)
tracker.on_model_load()                        # phase = "model_load"
tracker.on_prefill_begin(prompt_tokens=1024)   # phase = "prefill", logs seq_len
tracker.on_decode_begin()                      # phase = "decode"
for token in decode_loop():
    pass
tracker.on_generation_end(total_tokens=1124)   # logs token count
```

> **Why the prefill/decode split matters:** Prefill processes all prompt tokens in parallel (compute-bound, energy proportional to prompt_length²). Decode generates one token at a time (memory-bandwidth-bound). Tracking both separately helps optimise prompt length, batching strategies, and speculative decoding.

---

### Keras / TensorFlow

```python
import ecocode
from ecocode.integrations.keras import EcoCodeCallback

with ecocode.run("keras-train", project_id=42, api_key="eco_...") as eco_run:
    model.fit(
        x_train, y_train,
        validation_data=(x_test, y_test),
        epochs=10,
        callbacks=[EcoCodeCallback(eco_run)],
    )
```

**Hooks fired:**

| Keras hook | Phase marked | Extra logging |
|---|---|---|
| `on_train_begin` | `data_loading` | — |
| `on_epoch_begin` | — | `log_epoch(epoch)` |
| `on_train_batch_begin` | `forward` | — |
| `on_train_batch_end` | `optimizer_step` | `step`, `loss`, `lr`, `accuracy`, plus any other numeric logs via `log_custom()` |
| `on_test_begin` | `eval` | — |
| `on_test_end` | — | Validation metrics via `log_custom("val_*")` |
| `on_train_end` | `done` | — |

The Keras callback automatically forwards **unknown numeric metrics** from `logs` through `log_custom()`, so custom Keras metrics (e.g., precision, recall) are tracked without additional code.

---

### ONNX Runtime

```python
import onnxruntime as ort
import ecocode
from ecocode.integrations.onnxruntime import EcoCodeORTWrapper

session = ort.InferenceSession("model.onnx")

with ecocode.run("ort-inference", api_key="eco_...") as eco_run:
    eco_session = EcoCodeORTWrapper(session, eco_run)
    result = eco_session.run(None, {"input": data})
```

The wrapper:
- Marks `model_load` → `prefill` → `done` phases per `.run()` call.
- Estimates `batch_size` from the first input tensor shape.
- Logs `latency_ms` for each inference call.
- Proxies all other attributes to the underlying `InferenceSession`.

---

### Axolotl / Unsloth

Both Axolotl (≥ 0.4.0) and Unsloth use HuggingFace Trainer internally, so the existing `EcoCodeCallback` works without modification:

```python
# Axolotl — inject via Python API
from ecocode.integrations.huggingface import EcoCodeCallback

with ecocode.run("axolotl-sft", api_key="eco_...") as eco_run:
    do_train(config="config.yml", extra_callbacks=[EcoCodeCallback(eco_run)])
```

```python
# Unsloth — works with TRL's SFTTrainer
from ecocode.integrations.trl import EcoCodeSFTCallback

with ecocode.run("unsloth-sft", api_key="eco_...") as eco_run:
    trainer = SFTTrainer(
        model=model,
        callbacks=[EcoCodeSFTCallback(eco_run)],
        ...
    )
    trainer.train()
```

See `demo_axolotl.py` and `demo_unsloth.py` for complete examples.

---

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `ECOCODE_API_KEY` | API key for standalone mode | — (required if not passed to `run()`) |
| `ECOCODE_API_URL` | Backend base URL | `https://ecocode.app` |
| `ECOCODE_MARKER_FD` | Pipe file descriptor (set by CLI) | — (triggers attached mode) |
| `ECOCODE_RUN_ID` | Integer run ID (set by CLI) | — (triggers attached mode) |
| `RANK` / `LOCAL_RANK` | DDP rank (set by torchrun/deepspeed) | — (non-zero ranks use no-op mode) |

---

## What Gets Measured

| Metric | Standalone | Attached (CLI) | Notes |
|--------|-----------|----------------|-------|
| GPU energy (J) | Real (NVML) | Real (NVML) | Requires NVIDIA GPU + driver |
| CPU energy (J) | RAPL sysfs, proportional attribution | RAPL sysfs, proportional attribution | Same sysfs files, same algorithm. Requires files to be world-readable. `0.0` if unreadable. |
| RAM energy (J) | Real if DRAM domain present | Real (RAPL) | RAPL DRAM domain; `null` if not exposed by hardware |
| CPU usage (%) | `/proc/[pid]/stat` ticks | `/proc/[pid]/stat` ticks | Full process tree, normalised to 0-100% of machine |
| Memory usage (%) | PSS from `/proc/[pid]/smaps_rollup` | PSS from `/proc/[pid]/smaps_rollup` | Proportional Set Size avoids double-counting shared pages |
| Training phase | Set via `run.mark("phase", ...)` | Set via `run.mark("phase", ...)` | -- |
| Loss | Set via `run.log_loss(...)` | Set via `run.log_loss(...)` | -- |
| Learning rate | Set via `run.log_lr(...)` | Set via `run.log_lr(...)` | -- |
| Gradient step | Set via `run.log_step(...)` | Set via `run.log_step(...)` | -- |
| Token count | Set via `run.log_tokens(...)` | Set via `run.log_tokens(...)` | -- |
| Sequence length | Set via `run.log_sequence_length(...)` | Set via `run.log_sequence_length(...)` | -- |
| Accuracy | Set via `run.log_accuracy(...)` | Set via `run.log_accuracy(...)` | -- |
| Custom metrics | Set via `run.log_custom(key, value)` | Set via `run.log_custom(key, value)` | -- |
| CPU name / cores | `sysinfo` | `sysinfo` | Sent once at run start |
| RAM total (GB) | `sysinfo` | `sysinfo` | Sent once at run start |
| GPU names | NVML | NVML | Sent once at run start |

---

## Environmental Impact Equivalents

The dashboard converts each run's total CO2 figure into three everyday equivalents to make the impact tangible:

| Equivalent | Conversion factor | Source |
|---|---|---|
| Car distance | 170 gCO2/km | IEA Global EV Outlook 2023 — global average passenger car fleet |
| Smartphone charges | 8.22 gCO2/charge | 16.5 Wh battery (iPhone 15 Pro Max) × world-average grid intensity (498 gCO2/kWh, IEA 2022) |
| Trees needed (1 day) | 22 kg CO2/year per tree | One Tree Planted / EcoTree species-averaged absorption rate |

---

## Running the Demo

`demo_training.py` trains a ResNet-18 on CIFAR-10 and fully demonstrates the SDK API.

```bash
# 1. Start the EcoCode backend
cd ../web && docker compose up

# 2. Generate a project-scoped API key from the dashboard
#    Settings → Developer → select project → Generate Key

# 3. Install dependencies
pip install -e ".[gpu]"
pip install torch torchvision --index-url https://download.pytorch.org/whl/cu121

# 4. Run
API_KEY=eco_... python demo_training.py

# Tunable env vars
EPOCHS=30 BATCH_SIZE=256 LR=3e-4 API_KEY=eco_... python demo_training.py
```

**What you'll see on the dashboard:**

| Dimension | Value |
|-----------|-------|
| Phases | `data_loading → warmup → forward → backward → optimizer_step → eval → checkpoint_save → done` |
| Loss | Drops from ~2.3 → ~0.4 over 20 epochs |
| Val accuracy | ~70–75% on CPU; ~92%+ with GPU + more epochs |
| LR | Linear warmup for 5 steps, then cosine decay to 1e-6 |
| Tokens | Cumulative samples processed (~1M for 20 epochs × 50k samples) |
| GPU energy | Real NVML joules/second when CUDA is available |

---

## Package Metadata

| Field | Value |
|-------|-------|
| Name | `ecocode` |
| Version | `0.2.0` |
| Python | >= 3.9 |
| License | MIT |
| Build backend | [Hatchling](https://hatch.pypa.io/) |
| Dependencies | None (pure Python, stdlib only) |
| Keywords | energy, carbon, ml, llm, monitoring, sustainability, rlhf, training |

---

## Third-Party Dependencies

The SDK itself has zero runtime dependencies (pure Python, stdlib only). Optional framework extras (`transformers`, `trl`, `pytorch-lightning`, `keras`, `onnxruntime`) are all licensed under Apache-2.0 or MIT. The build backend (`hatchling`) is MIT. All are permissive licenses that allow commercial use.

See the [root README](../README.md#third-party-dependencies) for the full dependency audit.
