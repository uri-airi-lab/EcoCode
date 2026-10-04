# Packaging

How EcoCode gets onto a machine, per OS. The recommended route everywhere is the Python package,
whose wheels bundle the `ecocode` binary.

| | Python (all OSes) | Native |
|---|---|---|
| Linux | `pip install ecocode` | [`linux/install.sh`](linux/) + optional RAPL permissions rule |
| macOS | `pip install ecocode` | [Homebrew formula](macos/) |
| Windows | `pip install ecocode` | [winget manifests](windows/) |

Wheels are built per platform by CI (`.github/workflows/ci.yml`, job `wheels`) with
[maturin](https://www.maturin.rs/) from [`python/pyproject.toml`](../python/pyproject.toml).
Before publishing Linux wheels to PyPI, build them in a manylinux container (for example with
`PyO3/maturin-action`) so they run on older distributions.
