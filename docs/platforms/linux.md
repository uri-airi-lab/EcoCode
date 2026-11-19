# EcoCode on Linux

Supported on x86_64 and aarch64. Run `ecocode doctor` to see what your machine can measure.

## What is measured

| | Source | Notes |
|---|---|---|
| CPU energy | RAPL via powercap (`/sys/class/powercap/intel-rapl`), Intel and AMD | Measured. The package counter; see "CPU = package" below |
| DRAM energy | RAPL `dram` domain | Measured, where the CPU exposes it (most servers, few laptops) |
| iGPU energy | RAPL `uncore` / `graphics` domain | Measured, where exposed; taken out of the CPU figure |
| CPU / DRAM energy without RAPL | hwmon (`/sys/class/hwmon`): `energyN_input`, `powerN_input`, `powerN_average` | Fallback for ARM servers (NVIDIA Grace, Ampere) and boards with power telemetry. Energy counters are measured; power readings are integrated between samples (estimated) |
| NVIDIA GPU energy | NVML (driver ≥ 450) | Measured, per GPU |
| CPU usage | `/proc/<pid>/stat` | Thread group leaders only, so threads are not counted twice |
| Memory usage | PSS from `/proc/<pid>/smaps_rollup` | Shared pages are split between the processes that map them |

RAPL and hwmon are never combined, so nothing is counted twice.

## How energy is attributed to your program

Energy sensors measure a whole CPU package, DRAM channel or GPU. Each sample's energy is split by
your program's (and its child processes') share:

- **CPU and iGPU:** your share of total CPU capacity (CPU time ÷ (interval × cores)).
- **DRAM:** your share of physical memory (PSS). An estimate: DRAM power comes mostly from reading
  and writing memory, which can't be measured per process.
- **NVIDIA GPU:** your processes' share of GPU activity from NVML's per-process utilisation
  samples (compute, memory, encoder, decoder, weighted by how busy each is).

### CPU = package
CPU energy is each socket's **package** counter (`package-N`): the cores together with the caches,
memory controller and interconnect they rely on. The `core` sub-counter is not used: on Intel it
leaves those shared parts out, and on AMD the kernel's `core` counter follows a single core rather
than all of them. Where the CPU has an integrated GPU counter (`uncore` or `graphics`), that energy
is subtracted from the package and reported as iGPU, so it is not counted twice. DRAM is outside
the package and is reported separately. The platform counter `psys`, which covers the whole
machine, is not read.

The package also draws some power when the cores are idle, and your program is credited with its
share of that as well. On AMD processors without a separate graphics counter, an integrated GPU's
energy is part of the CPU figure.

## Permissions: reading RAPL without sudo

Since Linux 5.10, RAPL counters are readable by root only (CVE-2020-8694: fine-grained energy
readings can leak information about other processes). `ecocode doctor` reports this as
`[!!] powercap … readable by root only`. Options:

**Permanent, for chosen users (recommended):** a udev rule gives the `powercap` group read access
at every boot:
```bash
sudo packaging/linux/install-permissions.sh            # installs the rule, adds you to the group
newgrp powercap                                        # or log out and back in
ecocode doctor                                         # should now show [ok] powercap
```
Remove it with `sudo packaging/linux/install-permissions.sh --remove`.

**Until the next reboot, for everyone:**
```bash
sudo chmod -R a+r /sys/class/powercap/intel-rapl
```

EcoCode never runs `sudo` itself.

## Virtual machines and containers

Most VMs and cloud instances do not expose RAPL or hwmon power sensors, so CPU energy is 0 and
`ecocode doctor` says so; CPU and memory usage are still recorded. NVIDIA GPUs passed through to a
VM or container are measured if NVML works there (`nvidia-smi`). In Docker, RAPL is only visible
if `/sys/class/powercap` is mounted and readable inside the container.

## Testing on your machine

```bash
cargo test --workspace                 # needs no special hardware
cargo test --workspace -- --ignored    # real sensors: a measured CPU burn
ecocode doctor
```
