"""End-to-end tests: the SDK driving the real ecocode binary with fake sensors.

The binary is taken from ECOCODE_BIN, else from the workspace build
(target/debug or target/release). The tests are skipped if it isn't built:
run `cargo build` at the repository root first.
"""

import os
import subprocess
import warnings
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[3]
EXE = "ecocode.exe" if os.name == "nt" else "ecocode"


def _find_binary():
    if os.environ.get("ECOCODE_BIN"):
        return Path(os.environ["ECOCODE_BIN"])
    for profile in ("debug", "release"):
        candidate = REPO / "target" / profile / EXE
        if candidate.exists():
            return candidate
    return None


@pytest.fixture(scope="session")
def _warm_binary():
    binary = _find_binary()
    if binary is not None:
        # The first launch of a newly built binary can take ~1 s (macOS scans
        # it), which would delay the monitor's first samples in the first test.
        subprocess.run([str(binary), "--version"], capture_output=True, timeout=60)
    return binary


@pytest.fixture
def ecocode_bin(_warm_binary, monkeypatch, tmp_path):
    binary = _warm_binary
    if binary is None:
        pytest.skip("ecocode binary not built (run `cargo build` at the repository root)")
    monkeypatch.setenv("PATH", str(binary.parent) + os.pathsep + os.environ.get("PATH", ""))
    monkeypatch.setenv("ECOCODE_SENSORS", "fake")
    for var in ["ECOCODE_RUN_ID", "ECOCODE_MARKER_FD", "ECOCODE_MARKER_HANDLE", "RANK", "LOCAL_RANK"]:
        monkeypatch.delenv(var, raising=False)
    monkeypatch.chdir(tmp_path)
    # A development build is not in a standard install location.
    warnings.filterwarnings("ignore", message=".*unexpected path.*")
    return binary
