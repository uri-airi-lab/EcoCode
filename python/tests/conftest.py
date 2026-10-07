"""Fixtures shared by the e2e, frameworks and integration suites.

Everything here runs the *real* ecocode binary (with ECOCODE_SENSORS=fake so no
hardware is needed) and the real ``ecocode.run`` pipeline. Nothing is mocked:
the ``marker_tap`` fixture only observes the marker stream on its way into the
monitor's stdin pipe, and ``json_run`` reads back the records the monitor
actually persisted.
"""
from __future__ import annotations

import json
import os
import subprocess
import warnings
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import pytest

REPO = Path(__file__).resolve().parents[2]
EXE = "ecocode.exe" if os.name == "nt" else "ecocode"


# ---------------------------------------------------------------------------
# The ecocode binary
# ---------------------------------------------------------------------------

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


# ---------------------------------------------------------------------------
# Marker tap: observe the real marker stream, in order, without altering it
# ---------------------------------------------------------------------------

@dataclass
class MarkerTap:
    """Ordered record of every marker the SDK sent to the monitor.

    Entries are tuples: ``("mark", key, value)``, ``("epoch", n)``,
    ``("step", n)``, ``("tokens", n)``, ``("loss", x)``, ``("lr", x)``,
    ``("seq_len", n)``, ``("accuracy", x)``, ``("custom", key, x)``.
    """

    events: list[tuple] = field(default_factory=list)

    def phases(self) -> list[str]:
        """Every ``mark("phase", ...)`` value, in order."""
        return [e[2] for e in self.events if e[0] == "mark" and e[1] == "phase"]

    def values(self, kind: str) -> list[Any]:
        """Values of one event kind (``"step"``, ``"loss"``, ...), in order."""
        return [e[1] for e in self.events if e[0] == kind]

    def customs(self, key: str) -> list[float]:
        return [e[2] for e in self.events if e[0] == "custom" and e[1] == key]

    def custom_keys(self) -> set[str]:
        return {e[1] for e in self.events if e[0] == "custom"}

    def kinds(self) -> list[str]:
        """Event kinds in order; phases appear as ``"phase:<value>"``."""
        out = []
        for e in self.events:
            if e[0] == "mark":
                out.append(f"{e[1]}:{e[2]}")
            else:
                out.append(e[0])
        return out


@pytest.fixture
def marker_tap(monkeypatch) -> MarkerTap:
    """Wrap the real EcoCodeRun marker methods so they record *and* call through."""
    from ecocode._run import EcoCodeRun

    tap = MarkerTap()

    def wrap(name, make_entry):
        original = getattr(EcoCodeRun, name)

        def wrapper(self, *args, **kwargs):
            tap.events.append(make_entry(*args, **kwargs))
            return original(self, *args, **kwargs)

        monkeypatch.setattr(EcoCodeRun, name, wrapper)

    wrap("mark", lambda key, value: ("mark", str(key), str(value)))
    wrap("log_epoch", lambda n: ("epoch", n))
    wrap("log_step", lambda n: ("step", n))
    wrap("log_tokens", lambda n: ("tokens", n))
    wrap("log_loss", lambda x: ("loss", x))
    wrap("log_lr", lambda x: ("lr", x))
    wrap("log_sequence_length", lambda n: ("seq_len", n))
    wrap("log_accuracy", lambda x: ("accuracy", x))
    wrap("log_custom", lambda key, value: ("custom", key, value))
    return tap


# ---------------------------------------------------------------------------
# JSON run: a real standalone run with the JSON exporter, records read back
# ---------------------------------------------------------------------------

class JsonRun:
    """``with json_run("name") as run:`` then ``json_run.records`` afterwards."""

    def __init__(self, path: Path, interval: float):
        self.path = path
        self.interval = interval
        self.run = None

    def __call__(self, name: str = "frameworks", interval: float | None = None) -> "JsonRun":
        import ecocode

        self.path = self.path.with_name(f"{name}.json")
        if interval is not None:
            self.interval = interval
        self.run = ecocode.run(name, exporter="json", output_file=str(self.path), interval=self.interval)
        return self

    def __enter__(self):
        return self.run.__enter__()

    def __exit__(self, *exc):
        return self.run.__exit__(*exc)

    @property
    def records(self) -> list[dict]:
        return json.loads(self.path.read_text())

    # Convenience assertions over what the monitor persisted.
    def phases(self) -> list[str]:
        return [r["phase"] for r in self.records if r.get("phase")]

    def last(self) -> dict:
        return self.records[-1]

    def max_of(self, key: str):
        vals = [r[key] for r in self.records if r.get(key) is not None]
        return max(vals) if vals else None

    def custom_keys(self) -> set[str]:
        keys: set[str] = set()
        for r in self.records:
            keys |= set((r.get("custom_metrics") or {}).keys())
        return keys


@pytest.fixture
def json_run(ecocode_bin, tmp_path) -> JsonRun:
    return JsonRun(tmp_path / "run.json", interval=0.1)


def _burn(seconds: float) -> None:
    """Busy-wait so the monitor takes at least a couple of samples."""
    import time

    end = time.time() + seconds
    while time.time() < end:
        pass


@pytest.fixture
def burn():
    return _burn
