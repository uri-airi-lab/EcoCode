# Windows CPU energy driver (not implemented)

This directory is a placeholder. EcoCode does not ship a Windows kernel driver; this page records
why, and what it would take.

## Why a driver would be needed

CPU energy counters (Intel and AMD RAPL registers) can only be read from kernel mode. Linux and
macOS expose them to normal programs; Windows only does through the **Energy Meter Interface
(EMI)**, and only on hardware whose vendor provides an EMI device — mostly recent laptops. On
Windows desktops, servers and most AMD systems, EcoCode therefore reports CPU energy as unavailable
(NVIDIA GPU energy is still measured via NVML). A driver would fill that gap.

## Existing option reviewed: scaphandre's driver

[hubblo-org/windows-rapl-driver](https://github.com/hubblo-org/windows-rapl-driver)
(Apache-2.0, ~300 lines of C), used by scaphandre. Reviewed October 2026:

- **Good:** read-only (`__readmsr` only) with an allowlist of RAPL registers — Intel package, cores,
  graphics, DRAM, platform and power-limit registers; AMD unit, core and package registers. The
  licence is compatible with EcoCode's AGPL-3.0 and commercial licence (keep the notice).
- **Unsigned:** its README requires `bcdedit -set TESTSIGNING ON`, which needs Secure Boot turned
  off. Acceptable on a test machine, not for users.
- **Bugs:**
  1. `affinity.Mask = data.cpuIndex` — `Mask` is a bitmask and should be `1 << cpu`; reads land on
     the wrong CPU, so multi-socket machines read the wrong socket.
  2. `DriverUnload` deletes the symbolic link by the device name instead of the link name, so the
     link leaks and reloading the driver fails silently.
  3. The device is created without a security descriptor; with Windows' default permissions any
     local user may be able to read energy counters (the CVE-2020-8694 "PLATYPUS" side channel).
     It should be created with an SDDL limited to Administrators and an "EcoCode Users" group.
  4. The IOCTL code and output buffer size are not checked; CPU vendor detection runs on every open.
- **Maintenance:** driver code from 2023; later commits only touch the README.

## If this is picked up later

1. Fork the driver into this directory, keeping its Apache-2.0 notice, and fix the bugs above.
   Rename the device (e.g. `\\.\EcoCodeRapl`).
2. Add a `rapl_driver.rs` source to `crates/ecocode-sensors/src/windows/`, tried after EMI and
   before giving up. It could also talk to an already-installed scaphandre driver
   (`\\.\ScaphandreDriver`, same request format) as an interim option.
3. Production signing: an EV code-signing certificate, a Microsoft Partner Center account, and
   attestation signing for every release; check compatibility with Memory Integrity (HVCI).

Decide based on how many target machines lack EMI — `ecocode doctor` on Windows shows it.
