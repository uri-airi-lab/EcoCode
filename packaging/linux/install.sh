#!/usr/bin/env bash
#
# Build ecocode from this checkout and install it to /usr/local/bin.
#
#   packaging/linux/install.sh
#
# Needs Rust (rustup selects the version pinned in rust-toolchain.toml).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

cargo build --release --locked
sudo install -m 0755 target/release/ecocode /usr/local/bin/ecocode
echo "Installed /usr/local/bin/ecocode ($(/usr/local/bin/ecocode --version))."
echo
/usr/local/bin/ecocode doctor || true
echo
echo "If RAPL shows as root-only above, allow your user to read it permanently with:"
echo "  sudo packaging/linux/install-permissions.sh"
