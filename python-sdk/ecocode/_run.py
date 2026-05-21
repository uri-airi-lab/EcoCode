"""Core EcoCodeRun class — pure Python replacement for the PyO3 _core module.

Supports three usage patterns:
    # Context manager
    with ecocode.run("name", api_key="eco_...") as run: ...

    # Decorator
    @ecocode.track("name", api_key="eco_...")
    def train(): ...

    # Explicit start/stop
    run = ecocode.run("name", api_key="eco_...")
    run.start()
    ...
    run.stop()
"""

from __future__ import annotations

import os
import platform
import shutil
import subprocess
import sys
from datetime import datetime, timezone
from typing import Any, Optional

from ._http import EcoCodeHTTPClient
from ._pipe import InheritedFdWriter, NullWriter, PipeWriter

_DEFAULT_BASE_URL = "https://ecocode.app"


class EcoCodeRun:
    """Context manager for tracking an ML run's energy and semantic markers."""

    def __init__(
        self,
        name: str,
        *,
        api_key: Optional[str] = None,
        project_id: Optional[int] = None,
        base_url: Optional[str] = None,
        interval: float = 1.0,
    ) -> None:
        self._name = name
        self._api_key = api_key or os.environ.get("ECOCODE_API_KEY")
        self._project_id = project_id
        self._base_url = base_url or os.environ.get(
            "ECOCODE_API_URL", _DEFAULT_BASE_URL
        )
        self._interval = interval
        self._run_id: Optional[int] = None
        self._monitor_proc: Optional[subprocess.Popen] = None
        self._pipe: PipeWriter = NullWriter()
        self._client: Optional[EcoCodeHTTPClient] = None
        self._mode: str = "unknown"

    # --- Context manager protocol ---

    def __enter__(self) -> EcoCodeRun:
        # DDP rank detection: only track on rank 0
        rank = os.environ.get("RANK") or os.environ.get("LOCAL_RANK")
        if rank is not None and int(rank) != 0:
            self._mode = "noop"
            return self

        # Attached mode: CLI already created the run
        env_run_id = os.environ.get("ECOCODE_RUN_ID")
        env_marker_fd = os.environ.get("ECOCODE_MARKER_FD")

        if env_run_id and env_marker_fd:
            self._mode = "attached"
            self._run_id = int(env_run_id)
            self._pipe = InheritedFdWriter(int(env_marker_fd))
            return self

        # Standalone mode: SDK creates run + spawns monitor
        if not self._api_key:
            raise ValueError(
                "api_key is required in standalone mode. "
                "Pass api_key= or set ECOCODE_API_KEY."
            )

        self._client = EcoCodeHTTPClient(self._base_url, self._api_key)
        started_at = datetime.now(timezone.utc).isoformat()
        command = _sanitize_command(sys.argv) if hasattr(sys, "argv") and sys.argv else None

        self._run_id = self._client.create_run(
            project_id=self._project_id or 0,
            name=self._name,
            started_at=started_at,
            command=command,
        )

        self._client.save_system_info(self._run_id, _collect_system_info())

        # Spawn the energy monitor subprocess
        ecocode_bin = shutil.which("ecocode")
        if not ecocode_bin:
            raise RuntimeError(
                "[ecocode] CLI binary not found on PATH.\n"
                "Install it with: pip install ecocode-monitor\n"
                "Or reinstall:    pip install ecocode"
            )

        self._mode = "standalone"
        self._monitor_proc = subprocess.Popen(
            [
                ecocode_bin,
                "monitor",
                "--run-id",
                str(self._run_id),
                "--pid",
                str(os.getpid()),
                "--interval",
                str(self._interval),
            ],
            stdin=subprocess.PIPE,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            env={**os.environ, "ECOCODE_API_KEY": self._api_key},
        )

        # Give the monitor a moment to start and detect early crashes.
        import time as _time
        _time.sleep(0.2)
        rc = self._monitor_proc.poll()
        if rc is not None:
            raise RuntimeError(
                f"[ecocode] monitor subprocess exited immediately (code {rc}).\n"
                "Check that ECOCODE_API_KEY is valid and the backend is reachable."
            )

        self._pipe = PipeWriter(self._monitor_proc.stdin)
        return self

    def __exit__(self, exc_type, exc_val, exc_tb) -> bool:
        if self._mode == "noop":
            return False

        self._pipe.close()

        if self._monitor_proc is not None:
            try:
                self._monitor_proc.wait(timeout=10)
            except subprocess.TimeoutExpired:
                self._monitor_proc.kill()
                self._monitor_proc.wait()

        if self._mode != "attached" and self._client and self._run_id:
            status = "failed" if exc_type else "finished"
            ended_at = datetime.now(timezone.utc).isoformat()
            try:
                self._client.complete_run(self._run_id, status, ended_at)
            except Exception:
                pass  # Don't mask the original exception

        return False  # Never suppress exceptions

    # --- Explicit start/stop (alternative to context manager) ---

    def start(self) -> EcoCodeRun:
        """Begin tracking. Alternative to using as a context manager."""
        return self.__enter__()

    def stop(self) -> None:
        """Stop tracking and finalize the run. Alternative to context manager exit."""
        self.__exit__(None, None, None)

    # --- Marker API (identical signatures to the previous PyO3 class) ---

    def mark(self, key: str, value: Any) -> None:
        self._pipe.send({"event": str(key), "value": str(value)})

    def log_epoch(self, epoch: int) -> None:
        self._pipe.send({"event": "epoch", "value": epoch})

    def log_step(self, step: int) -> None:
        self._pipe.send({"event": "step", "value": step})

    def log_tokens(self, count: int) -> None:
        self._pipe.send({"event": "tokens", "count": count})

    def log_loss(self, loss: float) -> None:
        self._pipe.send({"event": "loss", "value": loss})

    def log_lr(self, lr: float) -> None:
        self._pipe.send({"event": "learning_rate", "value": lr})

    def log_sequence_length(self, seq_len: int) -> None:
        self._pipe.send({"event": "sequence_length", "value": seq_len})

    def log_accuracy(self, accuracy: float) -> None:
        self._pipe.send({"event": "accuracy", "value": accuracy})

    def log_custom(self, key: str, value: float) -> None:
        self._pipe.send({"event": "custom", "key": key, "value": value})


def _collect_system_info() -> dict:
    """Collect system info using pure Python (no external dependencies)."""
    info = {
        "cpu_name": platform.processor() or "Unknown",
        "cpu_cores": os.cpu_count() or 0,
        "ram_gb": 0.0,
        "gpu_count": 0,
        "gpu_names": "",
    }
    try:
        with open("/proc/meminfo") as f:
            for line in f:
                if line.startswith("MemTotal:"):
                    kb = int(line.split()[1])
                    info["ram_gb"] = round(kb / 1_048_576, 1)
                    break
    except OSError:
        pass
    return info


def _sanitize_command(args: list[str]) -> str:
    """Sanitize sensitive keys (like API keys/tokens) from sys.argv."""
    sensitive_keys = {"--api-key", "--token", "-k", "-t"}
    sanitized = []
    skip_next = False
    for i, arg in enumerate(args):
        if skip_next:
            skip_next = False
            continue
        
        if "=" in arg:
            key, val = arg.split("=", 1)
            if key.lower() in sensitive_keys:
                sanitized.append(f"{key}=********")
            else:
                sanitized.append(arg)
        elif arg.lower() in sensitive_keys:
            sanitized.append(arg)
            if i + 1 < len(args):
                sanitized.append("********")
                skip_next = True
        else:
            sanitized.append(arg)
            
    return " ".join(sanitized)
