# EcoCode Python SDK

Measure the energy, carbon footprint and training metrics of any Python ML workload, from inside
your code. The SDK wraps a run in a context manager, decorator or explicit `start()`/`stop()`
pair and lets you attach ML markers (phase, epoch, step, loss, learning rate, tokens) to each
energy sample.

```python
import ecocode

with ecocode.run("llama-finetune", exporter="online", api_key="ecocode_...") as run:
    run.mark("phase", "forward")
    run.log_step(100)
    run.log_loss(1.45)
```

The Python package has no runtime dependencies. Energy sampling is done by the `ecocode` CLI
binary, which the wheel bundles and the SDK starts as a subprocess, so the Python process gains no
native threads and stays fork-safe for DataLoader workers and DDP. Measurement failures and
backend errors are reported on stderr and never interrupt your training loop.

## Install

```bash
pip install ecocodes              # SDK plus the ecocode binary for your platform
pip install "ecocodes[hf]"        # HuggingFace Trainer callback
pip install "ecocodes[trl]"       # TRL (SFT / DPO / GRPO)
pip install "ecocodes[lightning]" # PyTorch Lightning
pip install "ecocodes[keras]"     # Keras 3 / tf.keras
pip install "ecocodes[onnx]"      # ONNX Runtime
pip install "ecocodes[deepspeed]" # DeepSpeed (Linux with CUDA)
pip install "ecocodes[vllm]"      # vLLM (Linux with CUDA)
pip install "ecocodes[all]"       # HF, TRL, Lightning, Keras and ONNX Runtime
```

The package is called `ecocodes` on PyPI because `ecocode` was already taken. You still
`import ecocode` and run `ecocode` on the command line.

The wheel installs the `ecocode` binary (`ecocode.exe` on Windows) into the environment's `bin/`
or `Scripts\` directory next to `python`; the SDK looks there first, then on `PATH`. Python 3.9
or newer is required. Linux, macOS and Windows are supported; see the
[platform notes](https://github.com/uri-airi-lab/EcoCode/tree/main/docs/platforms) for what each OS can measure.

To build from source you need Rust (`rust-toolchain.toml` pins the version):

```bash
git clone https://github.com/uri-airi-lab/EcoCode.git
cd EcoCode/python
pip install -e ".[dev]"   # builds the binary with maturin
pytest                    # unit and end-to-end tests with fake sensors
```

## Quick start

Context manager:

```python
import ecocode

with ecocode.run("my-run") as run:          # prints a live table to the terminal
    for epoch in range(epochs):
        run.log_epoch(epoch)
        for batch in loader:
            run.mark("phase", "forward")
            loss = model(batch)
            run.log_loss(loss.item())
```

Decorator (the run name defaults to the function name):

```python
@ecocode.track("resnet-cifar10", exporter="csv", output_file="energy.csv")
def train():
    ...

train()
```

Explicit start/stop, for notebooks or long-lived servers:

```python
run = ecocode.run("notebook-experiment", exporter="json", output_file="energy.json")
run.start()
...
run.stop()
```

### `ecocode.run()` parameters

| Parameter | Default | Meaning |
|---|---|---|
| `name` | `"run"` | Label for the run |
| `exporter` | `"terminal"` | `terminal`, `csv`, `json`, `logfile` or `online` |
| `output_file` | `None` | Output path, required for `csv`, `json` and `logfile` |
| `api_key` | `None` | Key for `online`; falls back to `ECOCODE_API_KEY` |
| `project_id` | `None` | Project for `online`; inferred from the key if omitted |
| `interval` | `1.0` | Seconds between energy samples |

`ecocode.track()` takes the same keyword arguments. The `online` exporter sends records to
https://ecocode.harmonyhpc.io; the backend address is fixed and `base_url=` and
`ECOCODE_API_URL` are ignored with a warning. If the backend cannot create the run at startup,
your code continues without recording and `run.is_recording` is `False`. Once the run exists it
is measured even if saving the machine's hardware details fails, and if the monitor cannot start
the run is marked failed rather than left running. If the connection drops during the run,
records are kept and uploaded in order once it is back (up to an hour of them at the default
interval), and the monitor prints a line when uploads stop and resume.

### Attached mode

When your script is started by `ecocode run`, the CLI does the measuring and the SDK only sends
markers through a pipe the CLI hands it. `ecocode.run()` detects this from the environment and
ignores its exporter arguments, so the same code works both ways:

```bash
ecocode run -o csv -f results.csv --project efficiency-study -- python train.py
```

Use attached mode when you want the CLI's exporters (including `prometheus`) or the `canceled`
status on Ctrl+C. See [docs/cli.md](https://github.com/uri-airi-lab/EcoCode/blob/main/docs/cli.md).

On multi-process jobs (`torchrun`, DeepSpeed) only rank 0 records; other ranks see `RANK` or
`LOCAL_RANK` and do nothing.

## Markers

Markers are attached to the next energy sample, so you can see what each phase of training costs.

| Method | Records |
|---|---|
| `run.mark("phase", label)` | Current phase (see labels below) |
| `run.log_epoch(n)` | Epoch number |
| `run.log_step(n)` | Optimizer step |
| `run.log_loss(x)` | Training loss |
| `run.log_lr(x)` | Learning rate |
| `run.log_tokens(n)` | Cumulative tokens processed |
| `run.log_sequence_length(n)` | Input sequence length |
| `run.log_accuracy(pct)` | Accuracy, 0 to 100 |
| `run.log_custom(key, value)` | Any numeric metric (gradient norm, perplexity, reward) |

Phase labels are free-form strings, but using these consistently makes runs comparable:

| Workload | Labels |
|---|---|
| Pretraining / SFT | `data_loading`, `tokenization`, `forward`, `backward`, `optimizer_step`, `eval`, `checkpoint_save`, `done` |
| RLHF | `reward_model`, `policy_rollout`, `reference_forward`, `ppo_update`, `dpo_update`, `grpo_update`, `kl_penalty` |
| Inference | `model_load`, `prefill`, `decode`, `kv_cache_ops`, `done` |

Frameworks that expose no hook between forward and backward (HuggingFace, TRL, Keras) use the
combined label `forward_backward`.

## Integrations

Each integration is a callback or wrapper that calls the marker API for you. Import the one you
need explicitly; none is imported by `import ecocode`.

### HuggingFace Transformers

Marks `data_loading`, `forward_backward`, `optimizer_step`, `eval`, `checkpoint_save` and `done`,
and logs epoch, step, loss, learning rate and (on recent `transformers`) total tokens.

```python
from ecocode.integrations.huggingface import EcoCodeCallback

with ecocode.run("sft-llama3") as eco_run:
    trainer = Trainer(model=model, args=training_args, callbacks=[EcoCodeCallback(eco_run)])
    trainer.train()
```

### TRL

One callback per trainer, because their energy profiles differ: `EcoCodeSFTCallback`
(`forward_backward`, `optimizer_step`), `EcoCodeDPOCallback` (`forward_backward`, `dpo_update`)
and `EcoCodeGRPOCallback` (`policy_rollout`, `grpo_update`).

```python
from ecocode.integrations.trl import EcoCodeGRPOCallback

with ecocode.run("grpo-qwen") as eco_run:
    trainer = GRPOTrainer(model=model, args=grpo_args, callbacks=[EcoCodeGRPOCallback(eco_run)])
    trainer.train()
```

### PyTorch Lightning

Lightning has an `on_before_backward` hook, so `forward` and `backward` are marked separately.
Assumes automatic optimization. The loss recorded at each optimizer step is the metric your
module logs as `train_loss` if there is one, otherwise the loss `training_step` returned.

```python
from ecocode.integrations.lightning import EcoCodeCallback

with ecocode.run("pretrain-vit") as eco_run:
    trainer = pl.Trainer(max_epochs=50, callbacks=[EcoCodeCallback(eco_run)])
    trainer.fit(model, datamodule=dm)
```

### DeepSpeed

DeepSpeed has no callback protocol, so you call the hooks yourself. Construction marks
`deepspeed_zero<N>_init`.

```python
from ecocode.integrations.deepspeed import EcoCodeDeepSpeedCallback

with ecocode.run("pretrain-70b") as eco_run:
    cb = EcoCodeDeepSpeedCallback(eco_run, zero_stage=3)
    for step, batch in enumerate(loader):
        cb.on_step_begin(step)                 # forward
        loss = engine(batch).loss
        cb.on_backward_begin()                 # backward
        engine.backward(loss)
        engine.step()
        cb.on_optimizer_step(step, loss.item())
    cb.on_train_end()
```

### vLLM

`EcoCodeVLLMMiddleware` wraps `AsyncLLMEngine.generate()` and marks `prefill` until the first
token, then `decode`, logging prompt length and cumulative tokens per request.

```python
from ecocode.integrations.vllm import EcoCodeVLLMMiddleware

with ecocode.run("vllm-serving") as eco_run:
    eco_engine = EcoCodeVLLMMiddleware(engine, eco_run)
    async for output in eco_engine.generate(prompt, sampling_params, request_id="req-1"):
        ...
```

For an inference loop you control yourself, `InferencePhaseTracker` in the same module exposes
`on_model_load()`, `on_prefill_begin(prompt_tokens)`, `on_decode_begin()` and
`on_generation_end(total_tokens)`.

### Keras

Works with Keras 3 and `tf.keras`. Marks `forward_backward` per batch and `optimizer_step` at
batch end, logs loss, learning rate and accuracy, and forwards other numeric `logs` entries as
custom metrics (validation metrics are prefixed `val_`).

```python
from ecocode.integrations.keras import EcoCodeCallback

with ecocode.run("keras-resnet") as eco_run:
    model.fit(x_train, y_train, epochs=20, callbacks=[EcoCodeCallback(eco_run)])
```

### ONNX Runtime

`EcoCodeORTWrapper` proxies an `InferenceSession`. Construction marks `model_load`; each `run()`
is marked `prefill`, or `decode` when a later call's first input has a sequence axis of length 1
(a KV-cache decode step). It logs `step`, `batch_size` and `latency_ms`.

```python
from ecocode.integrations.onnxruntime import EcoCodeORTWrapper

session = ort.InferenceSession("model.onnx")
with ecocode.run("ort-inference") as eco_run:
    eco_session = EcoCodeORTWrapper(session, eco_run)
    result = eco_session.run(None, {"input": data})
```

### Axolotl and Unsloth

Both train through HuggingFace `Trainer` or TRL `SFTTrainer`, so the callbacks above work as is:
pass `EcoCodeCallback(eco_run)` in Axolotl's `extra_callbacks`, or `EcoCodeSFTCallback(eco_run)`
to Unsloth's `SFTTrainer`.

## Environment variables

| Variable | Set by | Meaning |
|---|---|---|
| `ECOCODE_API_KEY` | You | API key for the `online` exporter when `api_key=` is not passed |
| `ECOCODE_RUN_ID` | `ecocode run` | Run ID; together with the marker pipe variable it switches the SDK to attached mode |
| `ECOCODE_MARKER_FD` | `ecocode run` | Marker pipe file descriptor (Linux, macOS) |
| `ECOCODE_MARKER_HANDLE` | `ecocode run` | Marker pipe handle (Windows) |
| `ECOCODE_SENSORS` | You | `fake` runs the monitor with synthetic sensors, for tests on machines without hardware access |
| `RANK` / `LOCAL_RANK` | torchrun, DeepSpeed | Non-zero ranks record nothing |

## What is measured

Every sample (default once a second) records CPU, DRAM, integrated GPU and discrete GPU energy
attributed to your process tree, CPU and memory usage, and the current marker values. CPU name,
core count, total RAM and GPU names are sent once at run start. Which sensors exist depends on
the OS and hardware: RAPL and NVML on Linux, IOReport on Apple Silicon, the Energy Meter
Interface and NVML on Windows. Run `ecocode doctor` to see what your machine can measure, and
read [docs/architecture.md](https://github.com/uri-airi-lab/EcoCode/blob/main/docs/architecture.md) for how energy is attributed to a process.

## Example

[`examples/python-basic/demo_training.py`](https://github.com/uri-airi-lab/EcoCode/blob/main/examples/python-basic/demo_training.py) trains a ResNet-18 on CIFAR-10 with every marker
exercised. Run it with `ECOCODE_API_KEY=ecocode_... python demo_training.py` to see the run on the
dashboard; `EPOCHS`, `BATCH_SIZE` and `LR` can be set from the environment.

## Third-party dependencies

The SDK itself uses only the standard library. The optional extras (`transformers`, `trl`,
`pytorch-lightning`, `keras`, `onnxruntime`) and the build backend (`maturin`) are licensed under
Apache-2.0 or MIT.

## License

AGPL-3.0-only. See the [repository LICENSE](https://github.com/uri-airi-lab/EcoCode/blob/main/LICENSE).

If the AGPL does not fit your use, for example inside a closed-source product or service, contact
us about a commercial license: Mohamed Aziz Khezami (mohamedazizkhezami@gmail.com) or Professor
Abdeltawab Hendawi (hendawi@uri.edu).
