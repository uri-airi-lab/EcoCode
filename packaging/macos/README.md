# macOS packaging

- **Python users:** `pip install ecocode` — the wheel includes the `ecocode` binary (arm64 and
  x86_64 wheels are built by CI, see `.github/workflows/ci.yml`).
- **Homebrew:** [`ecocode.rb`](ecocode.rb) builds the CLI from source. See the comment at the top of
  the file for how to publish it in a tap; to try it from a checkout:
  `brew install --HEAD --formula packaging/macos/ecocode.rb`.
- **From source:** `cargo build --release` at the repository root, then copy
  `target/release/ecocode` onto your `PATH`.

No permissions setup is needed: IOReport, SMC and `proc_pid_rusage` work without `sudo`.
See [docs/platforms/macos.md](../../docs/platforms/macos.md).

Release binaries should be signed and notarized (Developer ID) so Gatekeeper does not block
them when downloaded outside `pip`/Homebrew.
