"""Heavy integration tests: real GPT-2 / ResNet workloads through the real
callbacks. CUDA if available, else CPU with the workload scaled down by
ECOCODE_HEAVY_SCALE (default 0.05 on CPU, 1.0 on CUDA).

Exporter: ``online`` when ECOCODE_API_KEY is set (results land on the
dashboard; records are not asserted), otherwise the JSON exporter with fake
sensors so the emitted markers and persisted records are asserted locally.

Run explicitly: ``pytest -m heavy tests/integration -v -s``.
"""
from __future__ import annotations

import os

import pytest

os.environ.setdefault("KERAS_BACKEND", "torch")
os.environ.setdefault("TOKENIZERS_PARALLELISM", "false")
os.environ.setdefault("WANDB_DISABLED", "true")


def pytest_collection_modifyitems(items):
    for item in items:
        if "/integration/" in str(item.fspath).replace(os.sep, "/"):
            item.add_marker(pytest.mark.heavy)


@pytest.fixture(scope="session")
def device():
    torch = pytest.importorskip("torch")
    return "cuda" if torch.cuda.is_available() else "cpu"


@pytest.fixture(scope="session")
def scale(device):
    """Multiplier applied to sample counts / steps so CPU runs finish in minutes."""
    default = 1.0 if device == "cuda" else 0.05
    return float(os.environ.get("ECOCODE_HEAVY_SCALE", default))


@pytest.fixture(scope="session")
def scaled(scale):
    """``scaled(n, minimum=1)`` → n multiplied by the scale factor, at least ``minimum``."""

    def _scaled(n: int, minimum: int = 1) -> int:
        return max(minimum, int(round(n * scale)))

    return _scaled


class HeavyRun:
    """``with heavy_run("name") as run``, online when an API key is present,
    otherwise a local JSON run whose ``records`` can be asserted."""

    def __init__(self, json_run, online: bool):
        self._json_run = json_run
        self.online = online
        self.run = None

    def __call__(self, name: str):
        if self.online:
            import ecocode

            self.run = ecocode.run(name, exporter="online", interval=1.0)
        else:
            self.run = self._json_run(name, interval=0.5)
        return self

    def __enter__(self):
        return self.run.__enter__()

    def __exit__(self, *exc):
        return self.run.__exit__(*exc)

    # Only meaningful for local JSON runs.
    @property
    def records(self):
        return self._json_run.records

    def last(self):
        return self._json_run.last()

    def max_of(self, key):
        return self._json_run.max_of(key)

    def custom_keys(self):
        return self._json_run.custom_keys()


@pytest.fixture
def heavy_run(request, tmp_path, monkeypatch):
    online = bool(os.environ.get("ECOCODE_API_KEY"))
    if online:
        # Real sensors, real backend; the binary still has to be resolvable.
        request.getfixturevalue("_warm_binary")
        binary = request.getfixturevalue("_warm_binary")
        if binary is not None:
            monkeypatch.setenv("PATH", str(binary.parent) + os.pathsep + os.environ.get("PATH", ""))
        monkeypatch.chdir(tmp_path)
        return HeavyRun(None, online=True)
    return HeavyRun(request.getfixturevalue("json_run"), online=False)
