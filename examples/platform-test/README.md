# Platform test: a real training run, end to end

[`train.py`](train.py) trains a small CNN on MNIST with PyTorch — on the GPU when there is one
(NVIDIA CUDA or Apple MPS), otherwise the CPU — and reports it to the EcoCode web platform
(https://ecocode.harmonyhpc.io): energy, plus epochs, phases, steps, loss, accuracy, learning rate,
samples processed and throughput. Two DataLoader worker processes check that child processes are
measured too. One full run (3 epochs) takes 1–5 minutes depending on the machine.

Run the tests below on every machine and write anything unexpected in your issues doc.

## 1. Setup (once per machine)

Prerequisites: Rust (https://rustup.rs), Python 3.9+, and on Windows the Visual Studio Build Tools
("Desktop development with C++"). From the repository root:

**Linux / macOS**
```bash
python3 -m venv .venv
source .venv/bin/activate
```
**Windows (PowerShell)**
```powershell
python -m venv .venv
.venv\Scripts\Activate.ps1
```

Install PyTorch:
- **NVIDIA GPU** — the CUDA build matching your driver (see https://pytorch.org/get-started/locally/), e.g.
  `pip install torch torchvision --index-url https://download.pytorch.org/whl/cu124`
- **Everything else** (Mac, CPU-only) — `pip install torch torchvision`

Install the EcoCode SDK with its `ecocode` binary (builds it with Rust; a few minutes the first time):
```bash
pip install ./python
```

Check it, and note what this machine can measure:
```bash
python -c "import torch; print('cuda', torch.cuda.is_available(), '| mps', torch.backends.mps.is_available())"
ecocode doctor
```

On the web platform: create a project named **`ecocode-test`** (once; all machines share it), note its
**project ID**, and generate an **API key** under Settings → Developer. Then:
```bash
ecocode config set-api-key eco_YOUR_KEY
```
and for the SDK test set the key in your shell too — Linux/macOS: `export ECOCODE_API_KEY=eco_YOUR_KEY`,
Windows: `$env:ECOCODE_API_KEY="eco_YOUR_KEY"`.

Use a run name that says which machine and test it is, e.g. `linux-gpu-T1`.

## 2. Tests

### T1 — CLI + SDK (attached mode)
The CLI measures; the training script sends markers.
```bash
ecocode run -o online --project "ecocode-test" --name MACHINE-T1 --price 0.2 -- python examples/platform-test/train.py --mode attached
```
Terminal: three `epoch N: … accuracy …` lines (accuracy ≈ 0.98), then `Marker pipe latency: …`.

### T2 — SDK only (standalone mode)
The SDK starts the monitor itself; no `ecocode run`.
```bash
python examples/platform-test/train.py --mode sdk --name MACHINE-T2 --project-id YOUR_PROJECT_ID
```

### T3 — Cancel
Start T1 again with `--name MACHINE-T3`, press **Ctrl+C** during epoch 1.
Terminal: `Ctrl+C received — saving run data...`, exits within a few seconds.

### T4 — Script error
```bash
python examples/platform-test/train.py --mode sdk --name MACHINE-T4 --project-id YOUR_PROJECT_ID --crash
```
The script raises `RuntimeError: --crash: simulated training failure` after epoch 0.

### T5 — Same run offline (reference)
```bash
ecocode run -o json -f MACHINE-T5.json -- python examples/platform-test/train.py --mode attached
```
Nothing should appear on the web platform. Keep `MACHINE-T5.json`: if T1 looks wrong on the
dashboard, this file shows what EcoCode measured locally for the same workload.

## 3. What to check on the dashboard

| Check | T1 | T2 | T3 | T4 |
|---|---|---|---|---|
| Run appears in `ecocode-test` with the right name | ✓ | ✓ | ✓ | ✓ |
| Status | finished | finished | **canceled** | **failed** |
| Duration matches the terminal's "Finished in … s" | ✓ | ✓ | | |
| System info: CPU, cores, RAM, GPU names correct for the machine | ✓ | ✓ | | |
| Phases over time: `epoch:0`, `eval`, `epoch:1`, …, `done` (plus `data_loading` on the first run, while MNIST downloads) | ✓ | ✓ | up to cancel | up to epoch 0 |
| Steps rise to ≈ 1400 (3 × 469); loss falls; accuracy ≈ 0.98 | ✓ | ✓ | partial | |
| Learning rate 0.001 → 0.0007 → 0.00049 | ✓ | ✓ | | |
| Custom metrics: `epoch`, `grad_norm`, `samples_per_sec`, `val_loss` | ✓ | ✓ | | |
| Energy (see below) | ✓ | ✓ | ✓ | |
| Carbon and cost calculated (cost uses 0.2 per kWh) | ✓ | | | |

**Expected energy, per machine** — compare with what `ecocode doctor` reported:

| Machine | CPU energy | GPU energy |
|---|---|---|
| Linux with RAPL (`[ok] powercap`) | > 0 | NVIDIA: > 0, per-GPU chart present |
| Linux without RAPL (VM, `[!!]`/`[--] powercap`) | 0 (or > 0 via hwmon on ARM servers) | NVIDIA: > 0 |
| Apple Silicon Mac | > 0 | > 0 (the Apple GPU, shown as GPU 0) |
| Intel Mac | > 0 if `[ok] smc`, else 0 | 0 |
| Windows with EMI (`[ok] emi`) | > 0 | NVIDIA: > 0 |
| Windows without EMI | 0 | NVIDIA: > 0 |

Also note, without counting it as a failure: whether the dashboard shows the Neural Engine (`ane`)
entry on Macs, and how it presents CPU energy 0 on machines without a CPU sensor.

## 4. What to write in the issues doc
Machine, test (T1–T5), run name, run ID (from the dashboard URL), what the dashboard shows vs. the
table above (a screenshot helps), and the terminal output — especially any `Warning:`, `Error:` or
`[ecocode]` lines.
