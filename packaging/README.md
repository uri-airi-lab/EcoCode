# Packaging

How EcoCode gets onto a machine, per OS. The recommended route everywhere is the Python package,
whose wheels bundle the `ecocode` binary.

| | Python (all OSes) | Native |
|---|---|---|
| Linux | `pip install ecocodes` | [`linux/install.sh`](linux/) + optional RAPL permissions rule |
| macOS | `pip install ecocodes` | [Homebrew formula](macos/) |
| Windows | `pip install ecocodes` | [winget manifests](windows/) |

The package is published as `ecocodes` because `ecocode` was already taken on PyPI. Users still
`import ecocode` and run `ecocode`.

Wheels are built with [maturin](https://www.maturin.rs/) from
[`python/pyproject.toml`](../python/pyproject.toml). CI (`.github/workflows/ci.yml`, job `wheels`)
checks on every push that each platform still builds one. The wheels that get published come from
`.github/workflows/release.yml`, which builds the Linux ones in a manylinux container so they install
on older distributions.

## Publishing to PyPI

`release.yml` publishes with PyPI trusted publishing, so no token is stored in the repository.

One-time setup:

1. On both [pypi.org](https://pypi.org) and [test.pypi.org](https://test.pypi.org), sign in (two-factor
   authentication is required) and add a pending trusted publisher under Account settings,
   Publishing: project `ecocodes`, owner `uri-airi-lab`, repository `EcoCode`, workflow
   `release.yml`, environment `pypi` on PyPI and `testpypi` on TestPyPI.
2. In the GitHub repository settings, create the environments `pypi` and `testpypi`. Adding
   required reviewers to `pypi` makes every release wait for a manual approval.

Each release:

1. Set the same version in `Cargo.toml`, `python/pyproject.toml` and
   `python/src/ecocode/__init__.py`, and check that CI is green on `main`.
2. Dry run: start the Release workflow by hand (Actions, Release, Run workflow). It builds
   everything, installs each wheel to check it, and uploads to TestPyPI. Try it with
   `pip install -i https://test.pypi.org/simple/ ecocodes`.
3. Publish: `git tag v1.0.0 && git push origin v1.0.0`. The workflow refuses a tag that does not
   match the version numbers. A version on PyPI cannot be replaced, only followed by a new one.
