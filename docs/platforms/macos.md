# EcoCode on macOS

Run `ecocode doctor` to see what your Mac can measure. No `sudo` is needed for anything.

## What is measured

| | Apple Silicon (M1–M4) | Intel Mac |
|---|---|---|
| CPU energy | IOReport `CPU Energy`, measured | SMC CPU package power (`PCPT`/`PCPC`), sampled and integrated (estimated) |
| CPU energy of *your* process | Measured per process by macOS (`proc_pid_rusage` → `ri_energy_nj`) | Package energy × your share of CPU time |
| GPU energy | IOReport `GPU Energy`, measured | Not measured |
| Neural Engine energy | IOReport `ANE*`, measured; reported as `ane` in `socket_metrics` | Not present |
| Memory (DRAM) energy | IOReport `DRAM*`, measured | Not measured |
| CPU usage | `proc_pid_rusage` CPU time | same |
| Memory usage | Physical footprint (Activity Monitor's "Memory" column) | same |

## How energy is attributed to your program

- **CPU (Apple Silicon):** macOS measures each process's CPU energy itself, so EcoCode uses it
  directly (summed over your program and its child processes, capped at the CPU total). This is
  much more accurate than splitting by CPU time: one busy performance core draws most of the
  package's power, so a single-threaded job at "12% CPU" on an 8-core M1 Pro really causes about
  90% of the CPU energy.
- **GPU (Apple Silicon): whole GPU while your program is using it.** macOS has no per-process GPU
  energy a normal user can read. Its per-process GPU *time* (what Activity Monitor's GPU column
  shows) is handed out in turns that favour apps drawing on screen, so splitting energy by it
  under-credits compute jobs badly: in a test, a program keeping the GPU 100% busy got only 18%
  of the time while causing nearly all of the GPU's 11 W. Instead, while your program has
  completed or submitted GPU work within the last 5 seconds, it is credited with the GPU's full
  energy; otherwise with none.
  - Typical error: whatever other apps draw on the GPU at the same time, about 0.1 W for an
    idle desktop.
  - Overcounts if you run another GPU-heavy app (a game, a video export) during the run.
  - The GPU % column shows how busy the whole GPU is at each sample while your program is using
    it, and 0 otherwise.
- **Neural Engine:** no per-process data; attributed by your share of CPU time (an
  underestimate if your program is the only Neural Engine user). A better rule (whole Neural
  Engine while your program is using it, like the GPU) is pending a test with a Core ML workload.
- **DRAM:** attributed by your share of physical memory, the same rule as on Linux. This is an
  estimate: macOS reports how much memory each process *holds*, but memory energy comes mostly
  from reading and writing it, which no tool can measure per process. It undercounts programs
  that stream a lot of data through little memory (a GPU training job on unified memory) and
  overcounts large, idle ones.

## Intel binaries on Apple Silicon (Rosetta)

An `x86_64` build works under Rosetta and uses the Apple Silicon sensors above. Per-process CPU
time is corrected for Rosetta's clock translation. Prefer the native `arm64` build.

## Testing on your Mac

```bash
cargo test --workspace                 # everything that needs no special hardware
cargo test --workspace -- --ignored    # real sensors: IOReport, SMC, GPU clients, a measured run
```

On an Intel Mac, `reads_the_smc_key_count` prints the values of the SMC power keys
(`PCPT`, `PCPC`); please report them if `ecocode doctor` shows no CPU energy sensor.
