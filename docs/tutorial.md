# EcoCode tutorial

This walkthrough takes you from an empty machine to an energy-tracked training run: first with
the CLI alone, then with the Python SDK, then with both together. Reference material lives
elsewhere and is linked rather than repeated: [cli.md](cli.md) for every flag and exporter,
[python/README.md](../python/README.md) for the SDK API and framework callbacks, and
[architecture.md](architecture.md) for how the measurements are made.

## Prerequisites

| Requirement | Notes |
|---|---|
| Linux, macOS or Windows | What each OS can measure is in the [platform support table](../README.md#platform-support); `ecocode doctor` shows it for your machine |
| NVIDIA GPU and driver | Optional. Enables GPU energy via NVML on Linux and Windows; CPU-only tracking works without it |
| Rust | Only to build from source; `rust-toolchain.toml` selects the version through rustup |
| Python 3.9+ | Only for the Python SDK |
| Linux: RAPL access | Linux 5.10+ makes the CPU energy counters root-only; run `sudo packaging/linux/install-permissions.sh` once (see [platforms/linux.md](platforms/linux.md)) |

## Install

The Python package bundles the CLI binary, so one install covers both:

```bash
pip install ecocodes
ecocode --version
ecocode doctor
```

`ecocode doctor` lists each energy sensor as `[ok]`, `[--]` (not present on this hardware) or
`[!!]` (present but not readable), with a fix for the last case. Do this before anything else; a
column of zeros later is almost always a sensor reported here.

To build from source instead:

```bash
git clone https://github.com/uri-airi-lab/EcoCode.git
cd EcoCode
cargo build --release
sudo cp target/release/ecocode /usr/local/bin/      # or add target/release to PATH

cd python
pip install -e .            # SDK, built against the local checkout
pip install -e ".[all]"     # with every framework callback
```

The SDK looks for the binary in the environment's `bin/` (or `Scripts\`) directory first, then on
`PATH`. Installing it under `/usr/local/bin`, `~/.local/bin` or the active virtualenv avoids the
"resolved from an unexpected path" warning the SDK prints for other locations.

## Monitor any command with the CLI

The CLI wraps a command and measures it, with no changes to the program:

```bash
ecocode run -- python train.py
ecocode run --interval 0.5 -- python train.py
ecocode run --project efficiency-study --name fp32-baseline -- python train.py

ecocode run -- ./my_simulation
ecocode run -- node server.js
ecocode run -- java -jar benchmark.jar
```

The default `terminal` exporter prints one row per second with CPU, GPU and memory usage and
power, then a summary at exit with total energy in Wh, carbon footprint in gCO2eq and, if you
passed `--price`, electricity cost.

### Save the samples

```bash
ecocode run -o csv -f results.csv -- python train.py       # one row per sample, written live
ecocode run -o json -f results.json -- python train.py     # a JSON array, written at exit
ecocode run -o logfile -f run.log -- python train.py       # human-readable log
```

The CSV columns are listed in [cli.md](cli.md#exporters). Power columns are in watts, so
energy in watt-hours is the per-interval sum divided by 3600:

```python
import pandas as pd

df = pd.read_csv("results.csv")
print(df[["CPU(W)", "GPU(W)", "MEM(W)"]].describe())
interval_s = 1.0
print(f"CPU energy: {(df['CPU(W)'] * interval_s).sum() / 3600:.4f} Wh")
```

### Expose Prometheus metrics

```bash
ecocode run -o prometheus -- python train.py
# gauges at http://localhost:9091/metrics while the run lasts
```

Add the target to `prometheus.yml`:

```yaml
scrape_configs:
  - job_name: ecocode
    static_configs:
      - targets: ['localhost:9091']
```

### Send to the dashboard

```bash
ecocode config set-api-key eco_...        # stored in ~/.ecocode/config.json
ecocode run -o online --project efficiency-study -- python train.py
```

Create the project in the web app first. Its Environmental settings hold the grid region (or your
own carbon intensity) and the electricity price that online runs use; `--intensity` and `--price`
are only for local outputs.

## Instrument a training loop with the SDK

The SDK adds ML markers to the energy samples: which phase the program is in, the epoch, step,
loss, learning rate and token count. With no arguments it behaves like `ecocode run` with the
terminal exporter.

```python
import ecocode

with ecocode.run("my-first-run") as run:
    result = train_one_epoch(model, loader)
```

A full loop, with every marker in its natural place:

```python
import ecocode

with ecocode.run("llama3-finetune", exporter="csv", output_file="energy.csv") as run:
    run.mark("phase", "data_loading")
    loader = DataLoader(load_dataset("squad"), batch_size=32)
    global_step = 0

    for epoch in range(num_epochs):
        run.log_epoch(epoch)

        for batch in loader:
            run.mark("phase", "forward")
            loss = model(**batch).loss

            run.mark("phase", "backward")
            loss.backward()

            run.mark("phase", "optimizer_step")
            optimizer.step()
            scheduler.step()
            optimizer.zero_grad()

            run.log_step(global_step)
            run.log_loss(loss.item())
            run.log_lr(scheduler.get_last_lr()[0])
            run.log_tokens(global_step * batch_size * seq_len)
            global_step += 1

        run.mark("phase", "eval")
        val_loss, val_acc = evaluate(model, val_loader)
        run.log_accuracy(val_acc * 100)

        run.mark("phase", "checkpoint_save")
        model.save_pretrained(f"checkpoints/epoch_{epoch}")

    run.mark("phase", "done")
```

Markers take effect on the next sample, so a phase shorter than the sampling interval may not
show up on its own; lower `interval=` if you need finer resolution.

The decorator form wraps a whole function, and the explicit form suits notebooks where the work
spans several cells:

```python
@ecocode.track("resnet-cifar10")
def train():
    ...

run = ecocode.run("notebook-experiment")
run.start()
...          # several cells later
run.stop()
```

The marker methods, the recommended phase labels and the `ecocode.run()` parameters are tabulated
in [python/README.md](../python/README.md#markers).

## CLI and SDK together

When `ecocode run` starts a script that uses the SDK, the CLI does the measuring and the SDK only
sends markers. Nothing in the script changes; `ecocode.run()` notices the environment the CLI
sets up and ignores its own exporter arguments.

```bash
ecocode run -o csv -f results.csv --project efficiency-study --name attached-run -- python train.py
```

This is the way to get the CLI's exporters (including `prometheus`) together with markers, and
the way to get a run marked `canceled` rather than `failed` when you stop it with Ctrl+C. Running
the SDK on its own is simpler when you have no need for those.

## Framework callbacks

Each supported framework has a callback or wrapper that places the markers for you, so a Trainer
based script needs one extra line:

```python
import ecocode
from ecocode.integrations.huggingface import EcoCodeCallback

with ecocode.run("sft-llama3") as eco_run:
    trainer = Trainer(model=model, args=training_args, callbacks=[EcoCodeCallback(eco_run)])
    trainer.train()
```

The same pattern covers TRL (`EcoCodeSFTCallback`, `EcoCodeDPOCallback`, `EcoCodeGRPOCallback`),
PyTorch Lightning and Keras (`EcoCodeCallback` from their modules), DeepSpeed
(`EcoCodeDeepSpeedCallback`, whose hooks you call from your loop), vLLM
(`EcoCodeVLLMMiddleware`) and ONNX Runtime (`EcoCodeORTWrapper`). Install the matching extra
(`pip install "ecocodes[hf]"`, `[trl]`, `[lightning]`, `[keras]`, `[onnx]` or `[all]`); snippets
for each are in [python/README.md](../python/README.md#integrations).

## Troubleshooting

### `ecocode doctor` shows `[!!] powercap ... readable by root only`

Linux keeps the RAPL counters root-only. Install the udev rule once, then log in again or run
`newgrp powercap`:

```bash
sudo packaging/linux/install-permissions.sh
ecocode doctor      # should now show [ok] powercap
```

For a single session, `sudo chmod -R a+r /sys/class/powercap/intel-rapl` works until the next
reboot. EcoCode never runs `sudo` itself.

### CPU energy is always 0

Either the counters are unreadable (above), or the machine has no CPU energy sensor: most virtual
machines and cloud instances, and Windows PCs without an Energy Meter Interface. `ecocode doctor`
tells you which. CPU and memory usage are still recorded.

### GPU energy is always 0

Check that `nvidia-smi` works; NVML is loaded from the driver. GPU energy is attributed by your
processes' share of GPU activity, so a process that has not touched the GPU yet gets none. On
macOS, GPU energy is only available on Apple Silicon.

### `output_file is required when using exporter='csv'`

The `csv`, `json` and `logfile` exporters write to a file; pass `output_file="..."` to
`ecocode.run()` (or `-f` to the CLI).

### `api_key is required when using exporter='online'`

Pass `api_key=` or set `ECOCODE_API_KEY`. Keys start with `eco_` or `ecocode_` and come from the
dashboard under Settings, Developer. `base_url=` and `ECOCODE_API_URL` are ignored: the backend
address is fixed.

### `monitor-local subprocess exited immediately`

The SDK could not start the `ecocode` binary, or the output path is not writable. Run
`ecocode --version` in the same environment; if it is missing, reinstall the package or put your
own build on `PATH`.

### The CSV is empty after the run

The file is flushed after every sample, so an empty file means no sample was written. Make sure
the run was long enough for at least one interval, and stop runs with Ctrl+C rather than
`kill -9` so the final record and summary are written.
