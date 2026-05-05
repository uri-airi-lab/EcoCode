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
  - [`src/lib.rs` — The Rust extension (PyO3)](#srclibrs--the-rust-extension-pyo3)
  - [`src/sampler.rs` — High-precision sampling thread](#srcsamplerrs--high-precision-sampling-thread)
  - [`ecocode-core` — Shared energy sensor logic](#ecocode-core--shared-energy-sensor-logic)
  - [How Python calls into Rust](#how-python-calls-into-rust)
- [Installation](#installation)
- [Quick Start](#quick-start)
  - [Standalone Mode (no sudo required)](#standalone-mode-no-sudo-required)
  - [Attached Mode (CLI + sudo)](#attached-mode-cli--sudo)
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

The SDK is implemented as a native Rust extension compiled with [PyO3](https://pyo3.rs/) and [Maturin](https://maturin.rs/). There is no pure-Python measurement code — all sensor reads, threading, and HTTP calls happen in Rust.

---

## How It Works

### Two Operating Modes

The SDK operates in one of two mutually exclusive modes, **auto-detected at construction time** based on environment variables.

#### Standalone Mode

> **No CLI, no sudo, no root required.**

The SDK manages the entire run lifecycle itself:

1. Creates a run record on the EcoCode backend via HTTP (`POST /api/ingest/run`).
2. Collects system info (CPU, RAM, GPU names) via `sysinfo` and sends it via HTTP.
3. Starts two **daemon background threads** in Rust:
   - **Sampler thread** (`ecocode-sampler`): wakes up every `interval` seconds and reads hardware sensors.
   - **Reporter thread** (`ecocode-reporter`): receives completed records from the sampler via an in-process channel and posts them to the backend over HTTP. Blocking HTTP is isolated here so the sampler thread is never stalled.
4. The sampler reads:
   - **GPU energy** from NVIDIA's NVML library (`nvmlDeviceGetTotalEnergyConsumption`) — real joule deltas, no root needed.
   - **CPU energy** directly from RAPL sysfs (`/sys/class/powercap/intel-rapl/*/energy_uj`). These files are **world-readable on most Linux distributions**; no root required. If they are not readable, `cpu_energy` silently falls back to `0.0`.
   - **DRAM energy** from the RAPL `dram` sub-domain, if the CPU exposes it.
   - **CPU % and process memory %** via the `sysinfo` crate (Rust-native, replaces `psutil`).
   - ML marker state set by `log_*()` calls from the Python thread, snapshotted under a `Mutex`.
5. On `__exit__`, the sampler and reporter threads are stopped and the run is marked `finished` or `failed`.

> **Why no sudo?** The EcoCode CLI, which does require root, runs `sudo chmod -R +r /sys/class/powercap/intel-rapl/` once at startup. This makes the RAPL sysfs files world-readable for the session. The Python SDK (and any other unprivileged process) can then read them directly. On systems where the CLI was never run or where the files remain unreadable, CPU energy falls back to `0.0` without interrupting training.

#### Attached Mode

> **Requires the EcoCode CLI running with `sudo`.**

```bash
sudo ecocode -o online --api-key eco_... -- python train.py
```

The CLI (a separate Rust binary):
- Reads RAPL registers with root privileges (exact, unattributed machine-level energy) and NVML for GPU.
- Creates the run record in the backend.
- Binds a **Unix domain socket** and exports its path + the run ID as environment variables: `ECOCODE_SOCKET` and `ECOCODE_RUN_ID`.
- Spawns the user's Python process as a child.

The SDK sees these variables and switches to attached mode:
- **No background threads.** All heavy measurement is done by the CLI process.
- **No HTTP calls.** The SDK only sends lightweight semantic markers (phase, step, tokens…) through the Unix socket as newline-delimited JSON.
- Full CPU + GPU energy accuracy (root-level RAPL reads).
- Zero overhead on the training process beyond the socket writes.

**When to use standalone:**

- You are running on a cloud VM, a Jupyter notebook, a university cluster, or any environment where you do not have root access or cannot install system services.
- You want the simplest possible setup: `pip install ecocode`, add three lines of code, and go.
- You are prototyping or running one-off experiments where operational simplicity matters more than anything else.
- You are running on a machine where RAPL sysfs files are already world-readable (common on Ubuntu, Fedora, and most cloud VM images).

**When to use attached mode:**

- You are running production or long-duration training jobs on a machine where you control the environment and have root access.
- You need the RAPL files to be reliably readable regardless of OS configuration — the CLI's `chmod` guarantees this.
- You want zero overhead on the training process itself — the SDK does nothing except send sub-millisecond socket writes; all measurement runs in a separate root process.
- You need the `canceled` run status — if the job is killed with Ctrl+C, the CLI catches the signal, saves all data collected so far, and marks the run `canceled` cleanly. In standalone mode a SIGKILL kills the sampler threads instantly and the run is left open.
- You are running multi-node or multi-GPU jobs where a single CLI process monitoring the root launcher is cleaner than embedding SDK threads inside each worker.

**Mode comparison:**

| | Standalone | Attached |
|---|---|---|
| Who reads RAPL | Rust sampler thread in Python process | CLI process running as root |
| RAPL files used | Same `/sys/class/powercap/intel-rapl/*/energy_uj` sysfs files | Same `/sys/class/powercap/intel-rapl/*/energy_uj` sysfs files |
| CPU energy attribution | Proportional by process CPU usage fraction (same algorithm as CLI) | Proportional by process CPU usage fraction (same algorithm as SDK) |
| File access guaranteed | Requires CLI to have run `chmod -R +r` first, or a permissive distro | Yes — CLI runs as root |
| Background threads | Two (sampler + reporter) in-process | None — CLI does all sampling |
| HTTP calls | From the Python process | From the CLI |
| Training process overhead | Small (native Rust threads, minimal) | Negligible — only tiny socket writes |
| Run lifecycle management | SDK creates, records, and completes runs | CLI creates, records, and completes runs |
| `canceled` status | Not applicable — SDK only knows `finished` / `failed` | Produced by CLI on Ctrl+C |

---

### Mode Detection

```
EcoCodeRun.__init__()
       │
       ├─ ECOCODE_SOCKET set? ──── yes ──► Attached mode
       │  ECOCODE_RUN_ID  set?            (SocketClient → CLI process)
       │
       └─ no ──────────────────────────► Standalone mode
                                         (requires api_key)
```

This detection happens in the Rust `EcoCodeRun` constructor (`src/lib.rs`):

```rust
let socket_path = std::env::var("ECOCODE_SOCKET").ok();
let env_run_id  = std::env::var("ECOCODE_RUN_ID").ok();

if let (Some(path), Some(rid_str)) = (&socket_path, &env_run_id) {
    // Attached mode
} else {
    // Standalone mode
}
```

---

### Execution Flow

#### Standalone Mode — Step by Step

```
Python script                    Rust sampler/reporter threads     EcoCode backend
────────────────────────────────────────────────────────────────────────────────────
ecocode.run().__enter__()
  │
  ├─ POST /api/ingest/run  ──────────────────────────────────────► creates run row
  ├─ POST /api/ingest/run/{id}/system-info  ────────────────────► stores CPU/GPU info
  │
  ├─ SamplerHandle::start()
  │       │
  │       │  ecocode-sampler thread (every interval seconds):
  │       ├─ nvmlDeviceGetTotalEnergyConsumption()
  │       ├─ sysinfo::System::refresh_cpu_usage()
  │       ├─ sysinfo::Process::memory()   (full process tree)
  │       ├─ rapl::get_all_energies()     (RAPL sysfs)
  │       ├─ Mutex::lock() → snapshot ML marker state
  │       └─ mpsc::send(record) → ecocode-reporter thread
  │                                   │
  │                                   └─ POST /api/ingest/run/{id}/record ──────► stores metric row
  │
run.mark("phase", "forward")     # acquires Mutex, updates sampler marker state
run.log_step(100)                # acquires Mutex, updates sampler marker state
run.log_loss(1.45)               # acquires Mutex, updates sampler marker state
  │
ecocode.run().__exit__()
  ├─ SamplerHandle::stop()       # sends stop signal, joins both threads
  └─ POST /api/ingest/run/{id}/complete ──────────────────────────────────────► marks run done/failed
```

#### Attached Mode — Step by Step

```
CLI (sudo, Rust)                 Python script              EcoCode backend
─────────────────────────────────────────────────────────────────────────────
ecocode CLI starts
  ├─ reads RAPL + NVML on its own timer
  ├─ POST /api/ingest/run ─────────────────────────────────► creates run
  ├─ binds Unix socket at /tmp/ecocode-<pid>.sock
  ├─ export ECOCODE_SOCKET=/tmp/ecocode-<pid>.sock
  ├─ export ECOCODE_RUN_ID=42
  └─ exec python train.py

                                 ecocode.run().__enter__()
                                   └─ SocketClient::connect()

                                 run.mark("phase", "forward")
                                   └─ socket.send({"event":"phase","value":"forward"}\n)

CLI receives marker event ──────►
  └─ attaches marker to next metric record sent to backend

                                 ecocode.run().__exit__()
                                   └─ socket.close()

CLI detects EOF on socket, marks run "canceled" (Ctrl+C) or "finished"/"failed"
```

---

## Architecture Deep Dive

### `__init__.py` — Public API surface

Exposes two functions: `run()` and `track()`.

| Symbol | Type | Description |
|--------|------|-------------|
| `run(name, *, api_key, project_id, base_url, interval)` | Context manager factory | Returns an `EcoCodeRun`. Used with `with`. |
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

### `src/lib.rs` — The Rust extension (PyO3)

The central object `EcoCodeRun` is defined here as a `#[pyclass]`. Its `#[pymethods]` implement:

- `__new__`: mode detection, client initialisation.
- `__enter__`: in standalone mode, creates the run via HTTP, saves system info, spawns `SamplerHandle`.
- `__exit__`: stops the sampler, completes the run. Returns `False` — exceptions from the training code are **never suppressed**. The run status is `"finished"` (no exception) or `"failed"` (exception propagated). `"canceled"` is only produced by the CLI.
- All `log_*()` / `mark()` methods: in standalone mode, they acquire a `Mutex` and update the shared `MarkerState`; in attached mode, they serialise a JSON event and write it to the Unix socket.

---

### `src/sampler.rs` — High-precision sampling thread

Spawns two Rust threads:

**Sampler thread (`ecocode-sampler`)** — hardware reads only:

```rust
loop {
    match stop_rx.recv_timeout(wait_time) {
        Ok(()) => break,                           // stop signal
        Err(RecvTimeoutError::Timeout) => {}       // normal tick
        Err(RecvTimeoutError::Disconnected) => break,
    }
    // ... read RAPL, NVML, sysinfo ...
    let _ = reporter_tx.send(record);              // non-blocking
}
```

Using `recv_timeout` instead of `thread::sleep` means the thread wakes up immediately when `stop()` is called and always produces a final sample before exiting.

**Reporter thread (`ecocode-reporter`)** — blocking HTTP, isolated from the sampler:

```rust
while let Ok(record) = reporter_rx.recv() {
    client.add_record(run_id, &record)?;
}
```

This two-thread design ensures that a slow network never stalls the hardware sensor reads.

**Process tree tracking**

The sampler calls `get_process_tree_pids_from_sys()` to find the root PID and all its descendants (data-loader workers, `torch.distributed` ranks, etc.). CPU usage is summed across the whole tree; memory is the max-of-tree to avoid double-counting shared pages. The tree is refreshed every second.

**Thread-safety**

All `set_*()` methods (called from the Python main thread) acquire a `Mutex<MarkerState>`. The sampler also acquires the same `Mutex` to snapshot state before building each record. The Python thread and sampler thread never race.

**The payload shape posted every tick:**

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

> `cpu_energy` is non-zero when RAPL sysfs files are world-readable (either after the CLI's `chmod` or on distributions that expose them by default).
> `gpu_usage` reflects per-process GPU utilisation attributed from NVML process utilisation stats.

---

### `ecocode-core` — Shared energy sensor logic

`ecocode-core` is a Rust library crate at `ecocode/crates/core/`. Both the CLI binary and the Python SDK `.so` link against it statically. It provides:

- `sensor::rapl` — RAPL sysfs file handles, energy reads, delta computation with wrap-around correction.
- `sensor::gpu` — NVML device enumeration, energy delta computation, per-process GPU utilisation.
- `sensor::get_process_tree_pids_from_sys` — recursive PID-tree walker using `sysinfo`.
- `http_client::EcoCodeHttpClient` — blocking HTTP client wrapping `ureq` for the four ingest endpoints.

---

### How Python calls into Rust

Maturin compiles `python-sdk/src/lib.rs` into a native shared library:

```
_core.cpython-3xx-linux-gnu.so
```

The `#[pymodule]` macro on `_core()` and `#[pyclass]`/`#[pymethods]` on `EcoCodeRun` generate the CPython ABI glue automatically at compile time. When Python executes `from ecocode._core import EcoCodeRun`, it calls `dlopen()` on that `.so` — there is no subprocess, no IPC, no marshalling overhead. It is a native Python extension, identical in mechanism to NumPy or PyTorch's C extensions. The `ecocode-core` crate is linked as a static library directly into the `.so`.

```
Python: import ecocode
           └── ecocode/__init__.py
                   └── from ecocode._core import EcoCodeRun
                               │
                        _core.cpython-3xx.so  (PyO3, compiled by Maturin)
                               │
                    ┌──────────┴──────────────────────────┐
                    │  python-sdk/src/lib.rs               │
                    │  python-sdk/src/sampler.rs           │
                    │  python-sdk/src/socket_client.rs     │
                    │                                      │
                    │  → links ecocode-core (static)       │
                    │       sensor::rapl                   │
                    │       sensor::gpu  (NVML)            │
                    │       http_client                    │
                    └──────────────────────────────────────┘
```

---

## Installation

```bash
# Core (CPU + memory metrics, no GPU)
pip install -e .

# With GPU energy tracking (requires NVIDIA GPU + driver)
pip install -e ".[gpu]"

# With HuggingFace Trainer callback
pip install -e ".[hf]"

# With TRL callback (SFT / DPO / GRPO)
pip install -e ".[trl]"

# With PyTorch Lightning callback
pip install -e ".[lightning]"

# Everything
pip install -e ".[all]"

# Development (includes pytest, responses, pytest-mock)
pip install -e ".[dev]"
```

**Build requirement**: [Maturin](https://maturin.rs/) and a Rust toolchain (`rustup`) must be installed. `pip install -e .` invokes Maturin automatically.

**Optional runtime extras**:

| Extra | Package | Why |
|-------|---------|-----|
| `gpu` | `nvml-wrapper` (Rust) | NVIDIA GPU energy via NVML |
| `hf` | `transformers >= 4.30` | HuggingFace `TrainerCallback` base class |
| `trl` | `trl >= 0.7` | TRL SFT / DPO / GRPO trainers |
| `lightning` | `pytorch-lightning >= 2.0` | PyTorch Lightning `Callback` base class |
| `keras` | `keras >= 3.0` | Keras 3 / tf.keras `Callback` base class |
| `onnx` | `onnxruntime >= 1.16` | ONNX Runtime session wrapper |

---

## Quick Start

> **Not sure which mode to use?**
> - No root access, cloud VM, notebook, or just getting started → use **Standalone**.
> - Own the machine, need guaranteed RAPL reads, or running production jobs → use **Attached**.

### Standalone Mode (no sudo required)

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

### Attached Mode (CLI + sudo)

Start the CLI first, then your script can use a minimal import:

```bash
sudo ecocode -o online --api-key eco_... -- python train.py
```

```python
# train.py — no api_key needed; CLI injects ECOCODE_SOCKET + ECOCODE_RUN_ID
import ecocode

with ecocode.run() as run:
    run.mark("phase", "forward")
    run.log_step(100)
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
| `ECOCODE_API_URL` | Backend base URL | `http://localhost:3001` |
| `ECOCODE_SOCKET` | Unix socket path (set by CLI) | — (triggers attached mode) |
| `ECOCODE_RUN_ID` | Integer run ID (set by CLI) | — (triggers attached mode) |

---

## What Gets Measured

| Metric | Standalone | Attached (CLI) | Notes |
|--------|-----------|----------------|-------|
| GPU energy (J) | Real (NVML) | Real (NVML) | Requires `gpu` feature, NVIDIA GPU |
| CPU energy (J) | RAPL sysfs, proportional attribution | RAPL sysfs, proportional attribution | Same sysfs files, same algorithm. SDK requires files to be world-readable (CLI chmod or permissive distro). `0.0` if unreadable. |
| RAM energy (J) | Real if DRAM domain present | Real (RAPL) | RAPL DRAM domain; `null` if not exposed by hardware |
| CPU usage (%) | `sysinfo` (Rust) | `sysinfo` (Rust) | Full process tree, normalised to 0–100% of machine |
| Memory usage (%) | `sysinfo` RSS (max of tree) | `sysinfo` (max of tree) | Max across process tree to avoid double-counting |
| Training phase | Set via `run.mark("phase", ...)` | Set via `run.mark("phase", ...)` | — |
| Loss | Set via `run.log_loss(...)` | Set via `run.log_loss(...)` | — |
| Learning rate | Set via `run.log_lr(...)` | Set via `run.log_lr(...)` | — |
| Gradient step | Set via `run.log_step(...)` | Set via `run.log_step(...)` | — |
| Token count | Set via `run.log_tokens(...)` | Set via `run.log_tokens(...)` | — |
| Sequence length | Set via `run.log_sequence_length(...)` | Set via `run.log_sequence_length(...)` | — |
| Accuracy | Set via `run.log_accuracy(...)` | Set via `run.log_accuracy(...)` | — |
| Custom metrics | Set via `run.log_custom(key, value)` | Set via `run.log_custom(key, value)` | — |
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
| Version | `0.1.0` |
| Python | ≥ 3.9 |
| License | MIT |
| Build backend | [Maturin](https://maturin.rs/) |
| Keywords | energy, carbon, ml, llm, monitoring, sustainability, rlhf, training |
