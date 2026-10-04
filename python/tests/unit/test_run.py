import os
import socket
import urllib.request

import pytest

import ecocode
from ecocode import _http
from ecocode._run import _sanitize_command


@pytest.fixture(autouse=True)
def clean_env(monkeypatch):
    for var in ["ECOCODE_RUN_ID", "ECOCODE_MARKER_FD", "ECOCODE_MARKER_HANDLE", "ECOCODE_API_URL",
                "ECOCODE_API_KEY", "RANK", "LOCAL_RANK"]:
        monkeypatch.delenv(var, raising=False)


def test_backend_url_is_fixed(monkeypatch):
    seen = []

    def fake_urlopen(req, timeout, context):
        seen.append(req.full_url)
        raise OSError("offline")

    monkeypatch.setattr(urllib.request, "urlopen", fake_urlopen)
    with pytest.raises(OSError):
        _http.EcoCodeHTTPClient("ecocode_testkey123").create_run(1, "r", "2026-01-01T00:00:00Z")
    assert seen == ["https://ecocode.harmonyhpc.io/api/ingest/run"]


def test_base_url_and_env_override_are_ignored_with_a_warning(monkeypatch):
    with pytest.warns(UserWarning, match="ignored"):
        ecocode.run("r", exporter="terminal", base_url="http://localhost:3001")
    monkeypatch.setenv("ECOCODE_API_URL", "http://evil.example")
    with pytest.warns(UserWarning, match="ignored"):
        ecocode.run("r", exporter="terminal")


def test_exporter_argument_checks():
    with pytest.raises(ValueError, match="output_file"):
        ecocode.run("r", exporter="csv")
    with pytest.raises(ValueError, match="api_key"):
        ecocode.run("r", exporter="online").start()


def test_non_zero_ddp_rank_does_nothing(monkeypatch):
    monkeypatch.setenv("RANK", "1")
    with ecocode.run("r", exporter="json", output_file="unused.json") as r:
        r.mark("phase", "forward")
    assert r._mode == "noop"
    assert not os.path.exists("unused.json")


def _closed_port():
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def test_unreachable_backend_falls_back_to_noop(monkeypatch):
    monkeypatch.setattr(_http, "BASE_URL", f"http://127.0.0.1:{_closed_port()}")
    monkeypatch.setenv("ECOCODE_API_KEY", "eco_12345678901234567890123456789012")
    with pytest.warns(UserWarning, match="EcoCode backend unreachable"):
        with ecocode.run("offline", exporter="online") as r:
            r.log_step(1)
    assert r._mode == "noop"


@pytest.mark.skipif(os.name == "nt", reason="file descriptors are POSIX")
def test_attached_mode_writes_to_the_inherited_pipe(monkeypatch):
    r, w = os.pipe()
    monkeypatch.setenv("ECOCODE_RUN_ID", "7")
    monkeypatch.setenv("ECOCODE_MARKER_FD", str(w))
    with ecocode.run("attached") as run:
        run.log_loss(1.5)
    assert run._mode == "attached" and run._run_id == 7
    with os.fdopen(r, "rb") as f:
        assert b'"loss"' in f.read()


def test_invalid_marker_pipe_degrades_to_no_markers(monkeypatch):
    monkeypatch.setenv("ECOCODE_RUN_ID", "7")
    monkeypatch.setenv("ECOCODE_MARKER_HANDLE" if os.name == "nt" else "ECOCODE_MARKER_FD", "1")
    with pytest.warns(UserWarning, match="Ignoring invalid marker pipe"):
        with ecocode.run("attached") as run:
            run.log_step(1)
    assert run._mode == "attached"


def test_secret_flags_are_redacted():
    assert _sanitize_command(["train.py", "--api-key", "SECRET", "--lr=3", "--token=abc"]) == (
        "train.py --api-key ******** --lr=3 --token=********"
    )
