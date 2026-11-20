import os
import socket
import subprocess
import sys
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
        seen.append((req.full_url, timeout))
        raise OSError("offline")

    monkeypatch.setattr(urllib.request, "urlopen", fake_urlopen)
    with pytest.raises(OSError):
        _http.EcoCodeHTTPClient("ecocode_testkey123").create_run(1, "r", "2026-01-01T00:00:00Z")
    # A slow backend gets 30 s before the SDK gives up on a call.
    assert seen == [("https://ecocode.harmonyhpc.io/api/ingest/run", 30)]


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
    assert not r.is_recording


class FakeBackend:
    """Stands in for EcoCodeHTTPClient; `fail` names the calls that time out."""

    def __init__(self, fail=()):
        self.fail = set(fail)
        self.calls = []

    def _call(self, name):
        self.calls.append(name)
        if name in self.fail:
            raise TimeoutError("The read operation timed out")

    def create_run(self, **kwargs):
        self._call("create_run")
        return 42

    def save_system_info(self, run_id, info):
        self._call("save_system_info")

    def complete_run(self, run_id, status, ended_at):
        self._call(f"complete_run:{status}")


@pytest.fixture
def online(monkeypatch):
    """An online run against FakeBackend, with a stand-in monitor process that
    reads its stdin until the run closes it (or exits at once with `exit_now`)."""
    from ecocode import _run

    monkeypatch.setenv("ECOCODE_API_KEY", "eco_12345678901234567890123456789012")
    monkeypatch.setattr(_run, "resolve_ecocode_binary", lambda: sys.executable)
    monkeypatch.setattr(_run, "_CLOSE_RETRY_WAITS", (0, 0))
    real_popen = subprocess.Popen
    state = {"monitor_args": None, "exit_now": False}

    def fake_popen(args, **kwargs):
        state["monitor_args"] = args
        code = "pass" if state["exit_now"] else "import sys; sys.stdin.read()"
        return real_popen([sys.executable, "-c", code], **kwargs)

    monkeypatch.setattr(_run.subprocess, "Popen", fake_popen)

    def start(backend):
        monkeypatch.setattr(_run, "EcoCodeHTTPClient", lambda key: backend)
        return ecocode.run("online-test", exporter="online")

    state["start"] = start
    return state


def test_run_is_measured_when_only_system_info_fails(online):
    backend = FakeBackend(fail={"save_system_info"})
    with pytest.warns(UserWarning, match="Could not save system info for run 42"):
        with online["start"](backend) as r:
            assert r.is_recording
            r.log_step(1)
    assert online["monitor_args"][1:3] == ["monitor", "--run-id"] and online["monitor_args"][3] == "42"
    assert backend.calls == ["create_run", "save_system_info", "complete_run:finished"]


def test_run_that_cannot_be_measured_is_closed_as_failed(online):
    online["exit_now"] = True
    backend = FakeBackend()
    with pytest.raises(RuntimeError, match="exited immediately"):
        online["start"](backend).start()
    assert backend.calls == ["create_run", "save_system_info", "complete_run:failed"]


def test_error_in_user_code_closes_the_run_as_failed(online):
    backend = FakeBackend()
    with pytest.raises(ZeroDivisionError):
        with online["start"](backend):
            1 / 0
    assert backend.calls[-1] == "complete_run:failed"


def test_closing_is_retried_while_offline_then_reported_not_raised(online, capsys):
    backend = FakeBackend(fail={"complete_run:finished"})
    with pytest.warns(UserWarning, match="may stay 'running'"):
        with online["start"](backend):
            pass
    assert backend.calls.count("complete_run:finished") == 3
    assert "trying again" in capsys.readouterr().err


def test_closing_succeeds_once_the_network_is_back(online):
    backend = FakeBackend()
    attempts = []

    def flaky_complete(run_id, status, ended_at):
        attempts.append(status)
        if len(attempts) == 1:
            raise TimeoutError("The read operation timed out")

    backend.complete_run = flaky_complete
    with online["start"](backend):
        pass
    assert attempts == ["finished", "finished"]


def test_rejected_close_is_not_retried(online):
    import urllib.error

    backend = FakeBackend()
    attempts = []

    def rejected(run_id, status, ended_at):
        attempts.append(status)
        raise urllib.error.HTTPError("u", 404, "Not Found", None, None)

    backend.complete_run = rejected
    with pytest.warns(UserWarning, match="may stay 'running'"):
        with online["start"](backend):
            pass
    assert attempts == ["finished"]


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
