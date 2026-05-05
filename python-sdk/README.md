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
  - [`src/lib.rs` — The Rust extension (PyO3)](#src_librs--the-rust-extension-pyo3)
  - [`src/sampler.rs` — High-precision sampling thread](#src_samplerrs--high-precision-sampling-thread)
  - [`ecocode-core` — Shared energy sensor logic](#ecocode-core--shared-energy-sensor-logic)
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
- [Running the Demo](#running-the-demo)
- [Package Metadata](#package-metadata)

---

## Overview

The EcoCode Python SDK lets you instrument any ML training or inference script to measure real-time energy consumption and send structured telemetry to the [EcoCode backend](../web). It is designed to **never crash your training loop**: all HTTP failures and measurement errors are caught and emitted as Python warnings.

```python
import ecocode

with ecocode.run("llama-finetune", api_key="eco_...", project_id=42) as run:
    run.mark("phase", "forward")
    run.log_step(100)
    run.log_loss(1.45)
```

---

## How It Works

### Two Operating Modes

The SDK operates in one of two mutually exclusive modes, **auto-detected at construction time** based on environment variables.

#### Standalone Mode

> **No CLI, no sudo, no root required.**

The SDK manages the entire run lifecycle itself:

1. Creates a run record on the EcoCode backend via HTTP (`POST /api/ingest/run`).
2. Collects system info (CPU, RAM, GPU names) and sends it via HTTP.
3. Starts a **daemon background thread** (`Sampler`) that wakes up every `interval` seconds and:
   - Reads **GPU energy** from NVIDIA's NVML library (real joule deltas, no root needed).
   - Reads **CPU energy** directly from RAPL sysfs (`/sys/class/powercap/intel-rapl/*/energy_uj`). These files are **world-readable on most Linux distributions** — no root or sudo required. Energy is attributed to the process proportionally by its CPU usage %.
   - Reads **DRAM energy** from the RAPL `dram` sub-domain, if your CPU exposes it.
   - Reads **CPU % and process memory %** from `psutil`.
   - Snapshots any ML state set by `log_*()` calls from the main thread.
   - Posts a single structured JSON record to the backend.
4. On `__exit__`, the sampler is stopped and the run is marked `finished` or `failed`.

> If `/sys/class/powercap/intel-rapl/` is not readable (some hardened environments), `cpu_energy` silently falls back to `0.0` — training is never interrupted.

#### Attached Mode

> **Requires the EcoCode CLI running with `sudo`.**

```bash
sudo ecocode -o online --api-key eco_... -- python train.py
```

The CLI (a separate Rust binary):
- Reads RAPL registers (root = real CPU energy) and NVML for GPU.
- Creates the run record in the backend.
- Binds a **Unix domain socket** and exports its path + the run ID as environment variables: `ECOCODE_SOCKET` and `ECOCODE_RUN_ID`.

The SDK sees these variables and switches to attached mode:
- **No background thread.** All heavy measurement is done by the CLI.
- **No HTTP calls.** The SDK only sends lightweight semantic markers (phase, step, tokens…) through the Unix socket as newline-delimited JSON.
- Full CPU + GPU energy accuracy (root reads RAPL MSRs).

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

This detection happens in the Rust `EcoCodeRun` constructor:

```rust
let socket_path = std::env::var("ECOCODE_SOCKET").ok();
let env_run_id = std::env::var("ECOCODE_RUN_ID").ok();

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
Python script                    Sampler thread             EcoCode backend
─────────────────────────────────────────────────────────────────────────────
ecocode.run().__enter__()
  │
  ├─ POST /api/ingest/run  ────────────────────────────────► creates run row
  ├─ POST /api/ingest/run/{id}/system-info ───────────────► stores CPU/GPU info
  │
  ├─ Sampler.start()
  │       │
  │       │          (every interval seconds)
  │       ├─ nvmlDeviceGetTotalEnergyConsumption()
  │       ├─ psutil.Process.cpu_percent()
  │       ├─ psutil.virtual_memory() / proc.memory_info()
  │       ├─ acquire lock → snapshot ML state
  │       └─ POST /api/ingest/run/{id}/record ──────────────► stores metric row
  │
run.mark("phase", "forward")     # updates sampler._current_phase (thread-safe)
run.log_step(100)                # updates sampler._training_step  (thread-safe)
run.log_loss(1.45)               # updates sampler._loss           (thread-safe)
  │
ecocode.run().__exit__()
  ├─ Sampler.stop()              # sets stop event, joins thread
  └─ POST /api/ingest/run/{id}/complete ─────────────────────► marks run done/failed
```

#### Attached Mode — Step by Step

```
CLI (sudo, Rust)                 Python script              EcoCode backend
─────────────────────────────────────────────────────────────────────────────
ecocode CLI starts
  ├─ reads RAPL + NVML on its own timer
  ├─ POST /api/ingest/run ─────────────────────────────────► creates run
  ├─ binds Unix socket at /tmp/ecocode-XXXX.sock
  ├─ export ECOCODE_SOCKET=/tmp/ecocode-XXXX.sock
  ├─ export ECOCODE_RUN_ID=42
  └─ exec python train.py

                                 ecocode.run().__enter__()
                                   └─ SocketClient.connect()

                                 run.mark("phase", "forward")
                                   └─ socket.send({"event":"phase","value":"forward"}\n)

CLI receives marker event ──────►
  └─ attaches marker to next metric record sent to backend

                                 ecocode.run().__exit__()
                                   └─ socket.close()

CLI detects EOF on socket, marks run complete
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

### `_run.py` — The EcoCodeRun context manager

The central object. Holds references to all sub-systems and routes every public method to the correct backend (socket vs sampler).

**Constructor (`__init__`)**
- Reads `ECOCODE_SOCKET` and `ECOCODE_RUN_ID` from the environment to decide mode.
- In standalone mode: resolves `api_key` (arg > env `ECOCODE_API_KEY`), creates `EcoCodeClient`.
- In attached mode: creates `SocketClient` with the socket path.

**`__enter__`** (standalone only)
- POSTs to `/api/ingest/run` → receives `run_id`.
- Calls `_save_system_info()` → POSTs CPU/RAM/GPU metadata.
- Creates and starts the `Sampler` thread.

**`__exit__`**
- Standalone: stops the sampler, POSTs `complete` with status `"finished"` or `"failed"` depending on whether an exception propagated.
- Attached: closes the socket (signals EOF to the CLI).
- Returns `False` — exceptions from the training code are **never suppressed**.

**Public marker methods** all follow the same pattern:

```python
def log_loss(self, loss: float) -> None:
    if self._mode == "attached":
        self._socket.send_event({"event": "loss", "value": loss})
    elif self._mode == "standalone":
        self._sampler.set_loss(loss)
```

In attached mode the value is sent over the socket to the CLI.  
In standalone mode the sampler's in-memory state is updated (thread-safe via lock) and included in the next record POST.

---

### `_sampler.py` — Background measurement thread

A **daemon thread** (`threading.Thread(daemon=True)`) that runs a tight loop:

```python
while not self._stop_event.wait(timeout=self._interval):
    self._sample()
```

Using `stop_event.wait(timeout=N)` instead of `time.sleep(N)` means the thread wakes up immediately when `stop()` is called.

**NVML energy measurement (GPU)**

On init, the sampler:
1. Calls `pynvml.nvmlInit()`.
2. Iterates all GPUs, reads the initial `nvmlDeviceGetTotalEnergyConsumption()` value (in millijoules) and stores it as a baseline.

On each sample:
1. Reads the new cumulative energy counter.
2. Computes delta: `delta_mj = current - previous` (handles counter-reset with `if curr >= prev`).
3. Converts to joules: `delta_j = delta_mj / 1000.0`.
4. Updates the stored baseline.

**CPU / Memory measurement**

Uses `psutil.Process(pid)`:
- `cpu_percent(interval=None)` — non-blocking, returns % since last call (primed in `__init__`).
- `memory_info().rss / virtual_memory().total * 100` — process RSS as % of total RAM.

**Thread-safety**

All `set_*()` methods (called from the main training thread) acquire `self._lock`.  
The `_sample()` method also acquires `self._lock` to snapshot state before building the payload.

This means the main thread and sampler thread never race on the ML state variables.

**The payload shape posted every tick:**

```json
{
  "pid": 12345,
  "timestamp": "2026-05-04T12:00:00.000Z",
  "cpu_usage": 87.3,
  "cpu_energy": 0.0,
  "gpu_usage": 0.0,
  "gpu_energy": 2.14,
  "mem_usage": 12.5,
  "mem_energy": null,
  "igpu_usage": null,
  "igpu_energy": null,
  "gpu_metrics": [{"device_index": 0, "gpu_usage": 0.0, "gpu_energy": 2.14}],
  "socket_metrics": [],
  "phase": "forward",
  "tokens_processed": 102400,
  "training_step": 500,
  "loss": 1.23,
  "learning_rate": 0.0003,
  "sequence_length": 2048
}
```

> `cpu_energy` is always `0.0` — RAPL requires root.  
> `gpu_usage` (per-process utilisation) is always `0.0` — NVML process-level util stats require extra privileges; only total GPU energy (which is process-agnostic) is reported.

---

### `_http_client.py` — REST API client

A thin wrapper over `requests.Session` with four methods mapping directly to the four EcoCode ingest endpoints:

| Method | Endpoint | Description |
|--------|----------|-------------|
| `create_run(project_id, name, ...)` | `POST /api/ingest/run` | Creates a run, returns its integer `id` |
| `save_system_info(run_id, payload)` | `POST /api/ingest/run/{id}/system-info` | Stores CPU/GPU hardware metadata |
| `add_record(run_id, payload)` | `POST /api/ingest/run/{id}/record` | Stores a single time-series metric point |
| `complete_run(run_id, status, ended_at)` | `POST /api/ingest/run/{id}/complete` | Marks a run finished or failed |

All requests use `Bearer <api_key>` authentication and set `Content-Type: application/json`. Errors raise `requests.HTTPError` — caught upstream by `EcoCodeRun.__exit__` and the sampler loop, which emit `warnings.warn` instead of crashing.

---

### `_socket_client.py` — Unix socket for attached mode

A thread-safe UNIX domain socket (`AF_UNIX / SOCK_STREAM`) writer.

**Connection**: On construction, tries to connect up to 3 times with 50 ms between attempts. This handles the race condition where the CLI needs a moment to bind the socket after injecting the environment variable.

**Sending**: Each event is serialised as JSON + `\n` and sent with `socket.sendall()` under a `threading.Lock()`. If any send fails, the socket is set to `None` and future calls silently no-op (training is never interrupted).

**Protocol**: Newline-delimited JSON (NDJSON). The CLI reads lines from the socket and associates each marker event with the next hardware metric record it collects.

```json
{"event": "phase", "value": "forward"}
{"event": "step", "value": 42}
{"event": "loss", "value": 1.23}
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

**Core runtime dependencies** (always installed):

| Package | Why |
|---------|-----|
| `requests >= 2.28` | HTTP calls to the EcoCode backend |
| `psutil >= 5.9` | Process CPU % and memory usage |

**Optional**:

| Extra | Package | Why |
|-------|---------|-----|
| `gpu` | `pynvml >= 11.5` | NVIDIA GPU energy via NVML |
| `hf` | `transformers >= 4.30` | HuggingFace `TrainerCallback` base class |
| `trl` | `trl >= 0.7` | TRL SFT / DPO / GRPO trainers |
| `lightning` | `pytorch-lightning >= 2.0` | PyTorch Lightning `Callback` base class |
| `keras` | `keras >= 3.0` | Keras 3 / tf.keras `Callback` base class |
| `onnx` | `onnxruntime >= 1.16` | ONNX Runtime session wrapper |

---

## Quick Start

### Standalone Mode (no sudo required)

```python
import ecocode

with ecocode.run(
    "my-experiment",
    api_key="eco_...",          # or set ECOCODE_API_KEY env var
    project_id=42,              # optional if API key is project-scoped
    base_url="http://localhost:3001",  # default
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

> **Why the prefill/decode split matters:** Prefill processes all prompt tokens in parallel (compute-bound, energy ∝ prompt_length²). Decode generates one token at a time (memory-bandwidth-bound). Tracking both separately helps optimise prompt length, batching strategies, and speculative decoding.

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
| GPU energy (J) | ✅ Real (NVML) | ✅ Real (NVML) | Requires `pynvml`, NVIDIA GPU |
| CPU energy (J) | ✅ Real (RAPL sysfs) | ✅ Real (RAPL) | Readable without root on most Linux distros. `0.0` if `/sys/class/powercap/intel-rapl/` is unreadable. |
| RAM energy (J) | ✅ Real if DRAM domain present | ✅ Real (RAPL) | RAPL DRAM domain; `null` if not exposed by hardware |
| CPU usage (%) | ✅ psutil | ✅ CLI | Process-level |
| Memory usage (%) | ✅ psutil RSS | ✅ CLI | Process RSS / total RAM |
| Training phase | ✅ | ✅ | Set via `run.mark("phase", ...)` |
| Loss | ✅ | ✅ | Set via `run.log_loss(...)` |
| Learning rate | ✅ | ✅ | Set via `run.log_lr(...)` |
| Gradient step | ✅ | ✅ | Set via `run.log_step(...)` |
| Token count | ✅ | ✅ | Set via `run.log_tokens(...)` |
| Sequence length | ✅ | ✅ | Set via `run.log_sequence_length(...)` |
| Accuracy | ✅ | ✅ | Set via `run.log_accuracy(...)` |
| Custom metrics | ✅ | ✅ | Set via `run.log_custom(key, value)` |
| CPU name / cores | ✅ (platform + psutil) | ✅ | Sent once at run start |
| RAM total (GB) | ✅ (psutil) | ✅ | Sent once at run start |
| GPU names | ✅ (pynvml) | ✅ | Sent once at run start |

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

---

## TODO / Next Steps

These are the highest-impact improvements to make the Python SDK more complete.

### High Priority

- **`cpu_energy` in standalone mode:** RAPL sysfs files are world-readable on most Linux distributions, but some hardened environments (`/sys/class/powercap/intel-rapl/` not accessible) silently fall back to `0.0`. Add a clear startup warning when RAPL is unavailable, and document how to fix it (`sudo chmod -R +r /sys/class/powercap/intel-rapl/`).

- **Per-process GPU attribution in standalone mode:** The current implementation uses NVML `total_energy_consumption()` for GPU energy (whole-device, not per-process). The per-process GPU attribution path uses `process_utilization_stats()` which requires specific NVML privileges. Add a runtime check and fallback path that degrades gracefully with a clear log message.

- **PyPI distribution:** The SDK is currently installed via `pip install -e .` from source, which requires a local Rust toolchain and Maturin. Set up GitHub Actions CI to build `manylinux` wheels for Linux x86_64 and aarch64 and publish them to PyPI so users can install with `pip install ecocode` without a Rust build environment.

- **macOS support:** The current sampler uses RAPL (Linux-only) and NVML (NVIDIA-only). For Apple Silicon Macs, CPU/GPU power can be read from the `powermetrics` command or the `IOPMPowerSource` framework. Add a macOS sensor backend that reads from these sources.

### Medium Priority

- **`log_custom()` in attached mode:** In standalone mode, `log_custom(key, value)` stores the value in a `HashMap` that is included with each record. In attached mode, it sends a `{"event": "custom", "key": ..., "value": ...}` JSON event via the socket, but the CLI monitor's `MarkerState` struct does not currently have a `custom_metrics` field — the event is silently dropped. Add `custom_metrics: Option<HashMap<String, f64>>` to `MarkerState` in `monitor.rs` and handle the `"custom"` event type in the socket listener.

- **Decorator with return value:** The `@ecocode.track()` decorator currently calls the wrapped function but discards its return value. Fix the wrapper to return whatever the decorated function returns:

  ```python
  # Current behavior (broken for non-None return values):
  @ecocode.track("my-run", api_key="eco_...")
  def evaluate() -> float:
      return 0.95   # ← this is currently lost
  ```

- **Context manager nesting:** Using two `with ecocode.run()` blocks nested inside each other will create two independent runs. Document this behavior explicitly and add a warning when nested instantiation is detected (e.g. `ECOCODE_RUN_ID` already set in the environment when a new `EcoCodeRun` is created in standalone mode).

- **`interval` sub-second validation:** Passing `interval=0.05` to standalone mode will cause the sampling loop to warn on nearly every tick since hardware reads (RAPL + NVML + sysinfo) typically take 50–100 ms. Add a minimum interval check and a clearer warning message with a suggested minimum.

### Lower Priority

- **`ecocode.integrations.axolotl` dedicated callback:** The README recommends using `EcoCodeCallback` (HuggingFace) with Axolotl, which works because Axolotl uses HuggingFace Trainer internally. However, Axolotl has its own plugin protocol. A dedicated `EcoCodeAxolotlPlugin` would be more idiomatic and resilient to Axolotl API changes.

- **Async context manager support:** Some users run async training loops (e.g. with `asyncio`). Add `__aenter__` / `__aexit__` support to `EcoCodeRun` so it can be used as `async with ecocode.run(...) as run:`.

- **Sampling interval jitter measurement:** The sampler loop already logs a warning when a tick is slower than the interval. Add a jitter metric (`actual_elapsed - target_interval`) to the record payload so users can see sampling precision in the dashboard.

- **Windows support:** The sampler uses RAPL (Linux-only) for CPU energy. On Windows, consider integrating with the Intel Power Gadget SDK or Windows `ENERGY_ESTIMATION_PROVIDER` for CPU energy estimation. GPU energy via NVML already works cross-platform.
