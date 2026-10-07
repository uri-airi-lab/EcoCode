# Linux packaging

- **Python users:** `pip install ecocode`; the wheel includes the `ecocode` binary.
- **From this checkout:** [`install.sh`](install.sh) builds the release binary, installs it to
  `/usr/local/bin` and runs `ecocode doctor`.
- **Reading RAPL without sudo:** [`install-permissions.sh`](install-permissions.sh) installs the
  udev rule [`60-ecocode-powercap.rules`](60-ecocode-powercap.rules), which gives the `powercap`
  group read access to the energy counters at every boot, and adds you to the group.
  `--remove` undoes it.

See [docs/platforms/linux.md](../../docs/platforms/linux.md) for what is measured and why the
counters are root-only by default.
