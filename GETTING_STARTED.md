# Getting Started with EcoCode

> **New to EcoCode?** This guide walks you through everything from installation to running your first energy-tracked experiment — whether you are using the CLI standalone or the Python SDK inside a training loop.

---

## Table of Contents

1. [Prerequisites](#1-prerequisites)
2. [Building and Installing the CLI](#2-building-and-installing-the-cli)
3. [Tutorial A — CLI: Monitor Any Process](#3-tutorial-a--cli-monitor-any-process)
   - [A1. Terminal output](#a1-terminal-output)
   - [A2. Save results to CSV or JSON](#a2-save-results-to-csv-or-json)
   - [A3. Expose Prometheus metrics](#a3-expose-prometheus-metrics)
4. [Tutorial B — Python SDK: Instrument a Training Loop](#4-tutorial-b--python-sdk-instrument-a-training-loop)
   - [B1. Install the SDK](#b1-install-the-sdk)
   - [B2. Minimal example](#b2-minimal-example)
   - [B3. Full training loop with ML markers](#b3-full-training-loop-with-ml-markers)
   - [B4. Decorator API](#b4-decorator-api)
   - [B5. Notebook / explicit start-stop API](#b5-notebook--explicit-start-stop-api)
5. [Tutorial C — CLI + SDK Together (Attached Mode)](#5-tutorial-c--cli--sdk-together-attached-mode)
6. [Tutorial D — ML Framework Callbacks](#6-tutorial-d--ml-framework-callbacks)
   - [D1. HuggingFace Transformers / TRL](#d1-huggingface-transformers--trl)
   - [D2. PyTorch Lightning](#d2-pytorch-lightning)
   - [D3. Keras / TensorFlow](#d3-keras--tensorflow)
   - [D4. DeepSpeed](#d4-deepspeed)
   - [D5. vLLM Inference](#d5-vllm-inference)
7. [Common Options Reference](#7-common-options-reference)
8. [Troubleshooting](#8-troubleshooting)

---

## 1. Prerequisites

| Requirement             | Notes                                                                                             |
| -------------------------| ---------------------------------------------------------------------------------------------------|
| **Linux x86_64**        | RAPL sysfs is Linux-specific (for now); macOS and Windows are not supported for energy collection |
| **Intel CPU**           | Required for CPU/DRAM energy via RAPL (`/sys/class/powercap/intel-rapl/`)                         |
| **NVIDIA GPU + driver** | Optional; enables GPU energy via NVML. CPU-only tracking works without it                         |
| **Rust 1.75+**          | Only needed to build from source                                                                  |
| **Python 3.9+**         | Only needed for the Python SDK                                                                    |
| **`sudo`**              | RAPL sysfs files require elevated permissions on first run                                        |

---

## 2. Building and Installing the CLI

```bash
git clone https://github.com/MohamedAziz-Khezami/EcoCode.git
cd EcoCode/ecocode

# Build the release binary (~30 s on first build)
cargo build --release

# Optional: make it available system-wide
sudo cp target/release/ecocode /usr/local/bin/
```

Verify the installation:

```bash
ecocode --version
# ecocode 0.1.0
```

---

## 3. Tutorial A — CLI: Monitor Any Process

The CLI wraps **any** Linux command and measures its energy footprint. No code changes required.

```
sudo ecocode [OPTIONS] -- <COMMAND> [ARGS...]
```

> [!IMPORTANT]
> `sudo` is required for RAPL access. The CLI automatically makes the RAPL sysfs files world-readable so subsequent SDK calls in the same session do not need root.

### A1. Terminal output

```bash
# Monitor a Python script — live table printed every second
sudo ecocode -- python train.py

# Faster 500 ms sampling
sudo ecocode --interval 0.5 -- python train.py

# Label the run
sudo ecocode --project "efficiency-study" --name "fp32-baseline" -- python train.py

# Monitor any process — works with any language
sudo ecocode -- ./my_simulation
sudo ecocode -- node server.js
sudo ecocode -- java -jar benchmark.jar
```

Example terminal output:

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│  EcoCode  │  Run: fp32-baseline  │  Project: efficiency-study  │  PID: 94321   │
├───────────┬───────────┬───────────┬───────────┬───────────┬───────────┬────────┤
│  Time (s) │  CPU %    │  CPU (W)  │  GPU %    │  GPU (W)  │  MEM %    │ Phase  │
├───────────┼───────────┼───────────┼───────────┼───────────┼───────────┼────────┤
│       1.0 │    42.3   │    8.21   │    78.1   │   95.4    │    18.5   │   —    │
│       2.0 │    44.1   │    8.57   │    79.3   │   97.2    │    18.7   │   —    │
│       3.0 │    41.8   │    8.12   │    77.6   │   94.8    │    18.6   │   —    │
└───────────┴───────────┴───────────┴───────────┴───────────┴───────────┴────────┘
```

On exit, EcoCode prints a run summary showing total duration, component-wise energy (Wh), carbon footprint (gCO₂), and cost (if `--price` was passed).

### A2. Save results to CSV or JSON

```bash
# CSV — one row per sampling interval, appended in real time
sudo ecocode -o csv -f results.csv -- python train.py

# JSON — complete array written on exit
sudo ecocode -o json -f results.json -- python train.py

# Human-readable log file
sudo ecocode -o logfile -f run.log -- python train.py
```

CSV columns: `ID, RunID, PID, Timestamp, CPU%, CPU(W), GPU%, GPU(W), MEM%, MEM(W), IGPU%, IGPU(W), Phase, Tokens, Step, Loss, LR, SeqLen`

You can post-process these files with pandas, R, or any data analysis tool:

```python
import pandas as pd

df = pd.read_csv("results.csv")
print(df[["CPU(W)", "GPU(W)", "MEM(W)"]].describe())
total_energy_wh = df["CPU(W)"].sum() / 3600  # approximate
print(f"Total CPU energy: {total_energy_wh:.4f} Wh")
```

### A3. Expose Prometheus metrics

```bash
sudo ecocode -o prometheus -- python train.py
# Metrics available at http://localhost:9091/metrics
```

Add this scrape target to your `prometheus.yml`:

```yaml
scrape_configs:
  - job_name: ecocode
    static_configs:
      - targets: ['localhost:9091']
```

Available metrics include `ecocode_cpu_energy_watts`, `ecocode_gpu_energy_watts`, `ecocode_mem_energy_watts`, and per-GPU / per-socket breakdowns.

---

## 4. Tutorial B — Python SDK: Instrument a Training Loop

The Python SDK integrates directly into your Python code, letting you log ML-specific markers (epoch, step, loss, phase) alongside energy data and save them to any local format.

### B1. Install the SDK

```bash
# Core SDK (zero compiled dependencies — pure Python)
pip install ecocode

# With GPU support (NVML bindings)
pip install "ecocode[gpu]"

# With all framework callbacks
pip install "ecocode[all]"
```

> [!TIP]
> The SDK spawns the `ecocode` CLI binary as a subprocess in standalone mode, so the binary must be on your `PATH`. If you installed it to `/usr/local/bin/ecocode` in step 2, you are good to go.

### B2. Minimal example

```python
import ecocode

with ecocode.run("my-first-run") as run:
    # Your code here — energy is measured for everything inside the `with` block
    result = train_one_epoch(model, loader)
    print(f"Loss: {result['loss']:.4f}")
```

By default the SDK writes results locally. Energy data is collected by the `ecocode` subprocess and saved to the terminal output.

### B3. Full training loop with ML markers

```python
import ecocode

with ecocode.run(
    "llama3-finetune",
    interval=1.0,               # seconds between energy samples
) as run:

    run.mark("phase", "data_loading")
    dataset = load_dataset("squad")
    loader = DataLoader(dataset, batch_size=32)

    for epoch in range(num_epochs):
        run.log_epoch(epoch)                   # marks phase "epoch:N"

        for step, batch in enumerate(loader):
            # --- Forward pass ---
            run.mark("phase", "forward")
            outputs = model(**batch)
            loss = outputs.loss

            # --- Backward pass ---
            run.mark("phase", "backward")
            loss.backward()

            # --- Optimiser step ---
            run.mark("phase", "optimizer_step")
            optimizer.step()
            scheduler.step()
            optimizer.zero_grad()

            # Log ML metrics (attached to the next energy sample)
            run.log_step(global_step)
            run.log_loss(loss.item())
            run.log_lr(scheduler.get_last_lr()[0])
            run.log_tokens(global_step * batch_size * seq_len)

            global_step += 1

        # Validation
        run.mark("phase", "eval")
        val_loss, val_acc = evaluate(model, val_loader)
        run.log_accuracy(val_acc * 100)

        # Checkpoint
        run.mark("phase", "checkpoint_save")
        model.save_pretrained(f"checkpoints/epoch_{epoch}")

    run.mark("phase", "done")
```

**All available marker methods:**

| Method | Description |
|---|---|
| `run.mark("phase", value)` | Set the current training phase label |
| `run.log_epoch(n)` | Mark epoch `n` start |
| `run.log_step(n)` | Current optimiser step |
| `run.log_loss(value)` | Training loss |
| `run.log_lr(value)` | Current learning rate |
| `run.log_tokens(count)` | Cumulative tokens processed |
| `run.log_sequence_length(n)` | Input sequence length |
| `run.log_accuracy(pct)` | Accuracy in 0–100 |
| `run.log_custom("key", value)` | Any custom numeric metric |

### B4. Decorator API

```python
import ecocode

@ecocode.track("resnet-cifar10")
def train():
    for epoch in range(30):
        for batch in loader:
            ...

train()   # the entire function body is wrapped in an EcoCodeRun context
```

### B5. Notebook / explicit start-stop API

For Jupyter notebooks or long-lived servers where a context manager does not fit:

```python
import ecocode

run = ecocode.run("notebook-experiment")
run.start()

# Cell 1
run.mark("phase", "data_loading")
dataset = load_data()

# Cell 2
run.log_epoch(0)
train_epoch(model, dataset)

# Cell N — when done
run.stop()
```

---

## 5. Tutorial C — CLI + SDK Together (Attached Mode)

In **attached mode**, the CLI acts as the energy monitor and the SDK only sends lightweight markers. This gives you all CLI output formats (CSV, JSON, Prometheus, terminal table) with full Python SDK instrumentation.

```bash
# Start the CLI — it spawns your script and injects IPC variables
sudo ecocode -o csv -f results.csv \
  --project "efficiency-study" \
  --name "attached-run" \
  -- python train.py
```

```python
# train.py — no configuration needed; the CLI handles energy collection
import ecocode

with ecocode.run() as run:         # auto-detects attached mode from env vars
    for epoch in range(10):
        run.log_epoch(epoch)
        for step, batch in enumerate(loader):
            run.mark("phase", "forward")
            loss = model(batch)
            run.log_loss(loss.item())
            run.log_step(step)
```

The CLI sees the markers via an OS pipe and merges them with sensor data into each record. The Python process has **zero native threads** — fully fork-safe for DataLoader workers and DDP.

> [!NOTE]
> **When to use attached mode vs standalone:**
> - **Standalone** (`ecocode.run(...)`) — simplest; the SDK manages subprocess spawning and output.
> - **Attached** (`sudo ecocode ... -- python script.py`) — when you want CLI output formats (CSV, JSON, logfile, Prometheus) or need the Ctrl+C `canceled` run status.

---

## 6. Tutorial D — ML Framework Callbacks

All callbacks layer on top of the Python SDK. Install extras as needed:

```bash
pip install "ecocode[hf]"        # HuggingFace Transformers
pip install "ecocode[trl]"       # TRL (SFT / DPO / GRPO)
pip install "ecocode[lightning]" # PyTorch Lightning
pip install "ecocode[all]"       # Everything
```

### D1. HuggingFace Transformers / TRL

```python
import ecocode
from ecocode.integrations.huggingface import EcoCodeCallback
from transformers import Trainer, TrainingArguments

training_args = TrainingArguments(output_dir="output", num_train_epochs=3, ...)

with ecocode.run("sft-llama3") as eco_run:
    trainer = Trainer(
        model=model,
        args=training_args,
        train_dataset=train_ds,
        eval_dataset=eval_ds,
        callbacks=[EcoCodeCallback(eco_run)],
    )
    trainer.train()
```

**TRL SFT / DPO / GRPO:**

```python
from ecocode.integrations.trl import EcoCodeSFTCallback   # SFTTrainer
from ecocode.integrations.trl import EcoCodeDPOCallback   # DPOTrainer
from ecocode.integrations.trl import EcoCodeGRPOCallback  # GRPOTrainer

with ecocode.run("grpo-qwen") as eco_run:
    trainer = GRPOTrainer(
        model=model,
        args=grpo_args,
        callbacks=[EcoCodeGRPOCallback(eco_run)],
    )
    trainer.train()
```

### D2. PyTorch Lightning

```python
import ecocode
import pytorch_lightning as pl
from ecocode.integrations.lightning import EcoCodeCallback

with ecocode.run("pretrain-vit") as eco_run:
    trainer = pl.Trainer(
        max_epochs=50,
        callbacks=[EcoCodeCallback(eco_run)],
    )
    trainer.fit(model, datamodule=dm)
```

### D3. Keras / TensorFlow

```python
import ecocode
from ecocode.integrations.keras import EcoCodeCallback

with ecocode.run("keras-resnet") as eco_run:
    model.fit(
        x_train, y_train,
        validation_data=(x_test, y_test),
        epochs=20,
        callbacks=[EcoCodeCallback(eco_run)],
    )
```

### D4. DeepSpeed

DeepSpeed does not expose a `TrainerCallback` protocol, so use the manual hook object:

```python
import ecocode
from ecocode.integrations.deepspeed import EcoCodeDeepSpeedCallback

with ecocode.run("pretrain-70b") as eco_run:
    ds_cb = EcoCodeDeepSpeedCallback(eco_run, zero_stage=3)
    # Immediately marks phase "deepspeed_zero3_init"

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

### D5. vLLM Inference

```python
import ecocode
from ecocode.integrations.vllm import EcoCodeVLLMMiddleware
from vllm import AsyncLLMEngine

engine = AsyncLLMEngine.from_engine_args(engine_args)

with ecocode.run("vllm-serving") as eco_run:
    eco_engine = EcoCodeVLLMMiddleware(engine, eco_run)

    # Use eco_engine exactly like engine
    async for output in eco_engine.generate(prompt, sampling_params, request_id="req-1"):
        yield output
```

---

## 7. Common Options Reference

```
ecocode run [OPTIONS] -- <COMMAND> [ARGS...]

Options:
  -p, --project <NAME>       Project name for grouping runs [default: ecocode]
  -n, --name <NAME>          Human-readable label for this run
      --collaborator <NAME>  Person running this experiment
  -o, --output <FORMAT>      terminal | csv | json | prometheus | logfile  [default: terminal]
  -f, --file <PATH>          Output file (required for csv / json / logfile)
  -i, --interval <SECS>      Sampling interval in seconds [default: 1.0]
      --intensity <G/KWH>    Override carbon intensity in gCO₂eq/kWh
      --price <COST>         Electricity cost per kWh (for cost calculations)
      --address <LOCATION>   City or address for automatic intensity lookup
  -h, --help                 Print help
  -V, --version              Print version
```

**SDK environment variables:**

| Variable | Description |
|---|---|
| `ECOCODE_MARKER_FD` | Injected by CLI — triggers attached mode automatically |
| `ECOCODE_RUN_ID` | Injected by CLI — run ID in attached mode |

---

## 8. Troubleshooting

### `Permission denied` on RAPL files

```bash
# Make RAPL files world-readable once per session (or add to /etc/rc.local)
sudo chmod o+r /sys/class/powercap/intel-rapl/*/energy_uj
sudo chmod o+r /sys/class/powercap/intel-rapl/*/*/energy_uj
```

The CLI (`sudo ecocode ...`) does this automatically. The Python SDK in standalone mode does **not** — run the commands above before using the SDK without the CLI wrapper.

### `cpu_energy` is always `0.0`

The RAPL sysfs files are not world-readable. See the permissions fix above.

### `gpu_energy` is always `0.0`

- Check that NVML is available: `nvidia-smi` should work.
- The process may not have GPU activity yet (energy attribution uses SM utilisation — if the GPU has no activity, the attribution is 0).

### Stale Unix socket error

If the CLI crashes, it leaves a socket at `/tmp/ecocode_<pid>.sock`. The SDK retries 3× with 50 ms spacing and then silently no-ops. Delete the stale file if needed:

```bash
rm /tmp/ecocode_*.sock
```

### CSV file has no data after the run

If you used `-o csv` with attached mode, make sure the CLI exited cleanly (not killed with `kill -9`). Use Ctrl+C for a clean stop — the CLI catches `SIGINT`, flushes the final record, and closes the file.

---

*For deeper reference, see the [CLI Reference](ecocode/README.md), [Python SDK Guide](python-sdk/README.md), and [Architecture Overview](ARCHITECTURE.md).*
