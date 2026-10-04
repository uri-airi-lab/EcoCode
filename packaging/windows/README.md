# Windows packaging

- **Python users:** `pip install ecocode` — the wheel includes `ecocode.exe` (built by CI, see
  `.github/workflows/ci.yml`).
- **winget:** [`winget/`](winget/) holds manifest templates for a portable `ecocode.exe` in a zip.
  To publish: build with the MSVC toolchain (`cargo build --release`), sign `ecocode.exe`, zip it,
  attach it to a GitHub release, fill in the URL and SHA256, validate with
  `winget validate --manifest packaging\windows\winget`, and submit to
  [microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs).
- **MSI (optional):** [`cargo-wix`](https://github.com/volks73/cargo-wix) can produce an MSI that
  installs `ecocode.exe` and adds it to `PATH` (`cargo install cargo-wix`, `cargo wix init -p ecocode-cli`).
  Not needed while there is no driver to install.

No kernel driver is installed: CPU energy comes from the Energy Meter Interface where the hardware
has one. See [docs/platforms/windows.md](../../docs/platforms/windows.md) and
[drivers/windows/README.md](../../drivers/windows/README.md).
