# EcoCode on Windows

Supported on Windows 10/11, x86_64. Run `ecocode doctor` to see what your machine can measure.

## What is measured

| | Source | Notes |
|---|---|---|
| CPU energy | Energy Meter Interface (EMI), Windows' built-in energy counters | Measured, **only on machines that have an EMI device**: many recent Intel laptops and Surface devices. Most desktops, servers and AMD systems have none, and CPU energy is then reported as unavailable |
| iGPU / DRAM energy | EMI channels (`…PP1`, `…DRAM`) | Where the EMI device exposes them |
| NVIDIA GPU energy | NVML (`nvml.dll`, installed with the NVIDIA driver) | Measured, per GPU, same as Linux |
| CPU usage | `GetProcessTimes` | |
| Memory usage | Private working set (Task Manager's "Memory" column) | Working set on Windows older than 10 1809 |

EMI channels map as follows: CPU = cores (`PP0`), or the package (`PKG`) when there is no core
channel; `PP1` → integrated GPU; `DRAM` → memory. Platform totals (`PSYS`) are not used.

There is no kernel driver: without EMI, Windows gives normal programs no access to the CPU's energy
registers. See [drivers/windows/README.md](../../drivers/windows/README.md) for the options and why
none ships yet.

## How energy is attributed to your program

Same as Linux: CPU energy by your share of CPU time, DRAM by your share of memory, NVIDIA GPU by
your processes' share of GPU activity (NVML per-process samples).

## Your program and its child processes

- **Markers:** `ecocode run` passes the marker pipe to your program as an inheritable handle in
  `ECOCODE_MARKER_HANDLE` (on Linux/macOS it is a file descriptor in `ECOCODE_MARKER_FD`). The
  Python SDK picks either up automatically.
- **Stopping:** your program and everything it starts are placed in a Windows Job Object. On
  Ctrl+C, Ctrl+Break or closing the console window, your program receives the console event too and
  gets 3 seconds to exit; then every remaining process in the job is terminated. If ecocode itself
  is killed or crashes, the job closes and the whole tree ends with it.

## Testing on your machine

```powershell
cargo test --workspace                 # needs no special hardware
cargo test --workspace -- --ignored    # real sensors: a measured CPU burn (needs EMI)
ecocode doctor
```

Please report what `ecocode doctor` prints under "Energy sensors", in particular the EMI channel
names, which vary between vendors and are the least-tested part of Windows support.
