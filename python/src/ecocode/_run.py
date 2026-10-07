"""The ``EcoCodeRun`` class behind ``ecocode.run()`` and ``ecocode.track()``.

A run can be used as a context manager, as a decorator, or with explicit
``start()`` / ``stop()`` calls:

    with ecocode.run("name", exporter="terminal") as run: ...

    @ecocode.track("name", exporter="terminal")
    def train(): ...

    run = ecocode.run("name", exporter="terminal")
    run.start()
    ...
    run.stop()

Exporters: ``terminal`` (default) prints metrics to stdout; ``csv``, ``json``
and ``logfile`` write records to ``output_file``. None of these need an API
key or the network. ``online`` sends records to https://ecocode.harmonyhpc.io
and requires ``api_key`` (or ``ECOCODE_API_KEY``).
"""

from __future__ import annotations

import os
import subprocess
import sys
import warnings
from datetime import datetime, timezone
from typing import Any, Optional

from ._binary import resolve_ecocode_binary
from ._http import BASE_URL, EcoCodeHTTPClient
from ._pipe import NullWriter, PipeWriter, marker_pipe_in, open_inherited_writer
from ._sysinfo import collect_system_info
from ._validation import validate_api_key, validate_run_id

# Only the environment variables the monitor subprocess needs are forwarded.
# Passing the whole parent environment would expose every secret in it to the
# child (visible via /proc/<pid>/environ on Linux).
_SUBPROCESS_ENV_PASSTHROUGH = frozenset({
    # Shell / runtime essentials
    "PATH", "HOME", "USER", "LOGNAME", "SHELL",
    "LANG", "LC_ALL", "LC_CTYPE", "LANGUAGE",
    "TMPDIR", "TMP", "TEMP",
    "XDG_RUNTIME_DIR", "XDG_DATA_DIRS",
    # Windows essentials (process creation, home directory, DLL lookup)
    "SYSTEMDRIVE", "WINDIR", "COMSPEC", "PATHEXT",
    "USERPROFILE", "APPDATA", "LOCALAPPDATA", "PROGRAMDATA",
    "PROGRAMFILES", "PROGRAMFILES(X86)", "NUMBER_OF_PROCESSORS",
    # EcoCode test hook: fake sensors for runs on machines without hardware
    "ECOCODE_SENSORS",
    # Dynamic linker, needed for native extensions
    "LD_LIBRARY_PATH", "LD_PRELOAD",
    "DYLD_LIBRARY_PATH", "DYLD_FALLBACK_LIBRARY_PATH",  # macOS
    # NVIDIA / CUDA
    "CUDA_VISIBLE_DEVICES", "CUDA_HOME", "CUDA_PATH",
    "NVML_LIBRARY_PATH", "NVIDIA_VISIBLE_DEVICES",
    # AMD ROCm
    "ROCR_VISIBLE_DEVICES", "HIP_VISIBLE_DEVICES", "HSA_OVERRIDE_GFX_VERSION",
    # Intel oneAPI
    "ONEAPI_DEVICE_SELECTOR", "ZE_AFFINITY_MASK",
    # System hardware detection
    "SYSTEM_ROOT", "SYSTEMROOT",
})

# Exporters that are handled entirely locally (no API key, no network).
_LOCAL_EXPORTERS = frozenset({"terminal", "csv", "json", "logfile"})


class EcoCodeRun:
    """Context manager for tracking an ML run's energy and semantic markers."""

    def __init__(
        self,
        name: str,
        *,
        exporter: str = "terminal",
        output_file: Optional[str] = None,
        db_path: Optional[str] = None,
        api_key: Optional[str] = None,
        project_id: Optional[int] = None,
        base_url: Optional[str] = None,
        interval: float = 1.0,
    ) -> None:
        self._name = name
        self._exporter = exporter.lower().strip()
        if self._exporter in ("local", "sqlite"):
            warnings.warn(
                "[ecocode] The 'local' exporter was removed; using 'terminal' instead.",
                UserWarning,
                stacklevel=2,
            )
            self._exporter = "terminal"

        if db_path:
            warnings.warn(
                "[ecocode] 'db_path' is ignored: it belonged to the 'local' exporter, which was removed.",
                UserWarning,
                stacklevel=2,
            )

        self._output_file = output_file
        self._db_path = None
        self._project_id = project_id
        self._interval = interval
        self._run_id: Optional[int] = None
        self._monitor_proc: Optional[subprocess.Popen] = None
        self._pipe: PipeWriter = NullWriter()
        self._client: Optional[EcoCodeHTTPClient] = None
        self._mode: str = "unknown"

        if base_url is not None or os.environ.get("ECOCODE_API_URL"):
            warnings.warn(
                f"[ecocode] base_url / ECOCODE_API_URL is ignored: the EcoCode backend is fixed "
                f"({BASE_URL}).",
                UserWarning,
                stacklevel=3,
            )

        if self._exporter == "online":
            # Resolve the API key from arguments then environment, and validate it
            # eagerly so misconfiguration fails at construction time rather than at
            # the first network call.
            raw_key = api_key or os.environ.get("ECOCODE_API_KEY")
            self._api_key: Optional[str] = validate_api_key(raw_key) if raw_key else None
        else:
            # Local exporters never touch the network.
            self._api_key = None

        # Validate exporter-specific arguments early.
        if self._exporter in ("csv", "json", "logfile") and not self._output_file:
            raise ValueError(
                f"[ecocode] output_file is required when using exporter='{self._exporter}'."
            )

    def __enter__(self) -> EcoCodeRun:
        # DDP rank detection: only track on rank 0
        rank = os.environ.get("RANK") or os.environ.get("LOCAL_RANK")
        if rank is not None and int(rank) != 0:
            self._mode = "noop"
            return self

        # Attached mode: `ecocode run` already created the run and passed a
        # marker pipe (a file descriptor on Linux/macOS, a handle on Windows).
        env_run_id = os.environ.get("ECOCODE_RUN_ID")

        if env_run_id and marker_pipe_in(os.environ):
            self._mode = "attached"

            # Check the run ID is a sane positive integer rather than blindly
            # casting whatever the env var contains.
            try:
                self._run_id = validate_run_id(env_run_id)
            except ValueError as exc:
                raise ValueError(
                    f"[ecocode] Invalid ECOCODE_RUN_ID: {exc}"
                ) from exc

            # The inherited FD/handle is validated before it is opened. A
            # tampered ECOCODE_MARKER_FD=1 would otherwise redirect all marker
            # writes to stdout.
            try:
                self._pipe = open_inherited_writer(os.environ) or NullWriter()
            except ValueError as exc:
                warnings.warn(
                    f"[ecocode] Ignoring invalid marker pipe: {exc}. "
                    "Phase markers will not be recorded for this run.",
                    stacklevel=3,
                )
                self._pipe = NullWriter()

            return self

        # Standalone mode: branch on exporter type.
        if self._exporter in _LOCAL_EXPORTERS:
            return self._start_local_standalone()
        else:
            return self._start_online_standalone()

    def _start_local_standalone(self) -> EcoCodeRun:
        """Start a standalone run with a local exporter (no API key needed)."""
        ecocode_bin = resolve_ecocode_binary()

        # Build safe environment for the subprocess.
        safe_env: dict[str, str] = {
            k: v
            for k, v in os.environ.items()
            if k in _SUBPROCESS_ENV_PASSTHROUGH
        }

        # `ecocode monitor-local` writes locally, no network calls. Run ID 0 is
        # a placeholder; the monitor subprocess manages the run itself.
        self._run_id = 0
        self._mode = "standalone_local"

        cmd = [
            ecocode_bin,
            "monitor-local",
            "--run-id", str(self._run_id),
            "--pid", str(os.getpid()),
            "--interval", str(self._interval),
            "--exporter", self._exporter,
        ]
        if self._output_file:
            cmd += ["--file", self._output_file]

        self._monitor_proc = subprocess.Popen(
            cmd,
            stdin=subprocess.PIPE,
            stdout=None if self._exporter == "terminal" else subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            env=safe_env,
        )

        import time as _time
        _time.sleep(0.2)
        rc = self._monitor_proc.poll()
        if rc is not None:
            raise RuntimeError(
                f"[ecocode] monitor-local subprocess exited immediately (code {rc}).\n"
                "Check that the ecocode binary is installed and the exporter path is writable."
            )

        self._pipe = PipeWriter(self._monitor_proc.stdin)
        return self

    def _start_online_standalone(self) -> EcoCodeRun:
        """Start a standalone run posting records to the EcoCode online platform."""
        if not self._api_key:
            raise ValueError(
                "api_key is required when using exporter='online' in standalone mode. "
                "Pass api_key= or set ECOCODE_API_KEY."
            )

        self._client = EcoCodeHTTPClient(self._api_key)
        started_at = datetime.now(timezone.utc).isoformat()
        command = _sanitize_command(sys.argv) if hasattr(sys, "argv") and sys.argv else None

        try:
            self._run_id = self._client.create_run(
                project_id=self._project_id or 0,
                name=self._name,
                started_at=started_at,
                command=command,
            )
            self._client.save_system_info(self._run_id, collect_system_info())
        except Exception as exc:
            warnings.warn(
                f"[ecocode] EcoCode backend unreachable or failed to initialize run: {exc}. "
                "Running in local fallback mode.",
                category=UserWarning,
                stacklevel=3,
            )
            self._mode = "noop"
            return self

        ecocode_bin = resolve_ecocode_binary()

        # Build a minimal environment for the monitor subprocess.
        safe_env: dict[str, str] = {
            k: v
            for k, v in os.environ.items()
            if k in _SUBPROCESS_ENV_PASSTHROUGH
        }
        safe_env["ECOCODE_API_KEY"] = self._api_key  # type: ignore[assignment]

        self._mode = "standalone_online"
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
            env=safe_env,
        )

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

        if self._mode == "standalone_online" and self._client and self._run_id:
            status = "failed" if exc_type else "finished"
            ended_at = datetime.now(timezone.utc).isoformat()
            try:
                self._client.complete_run(self._run_id, status, ended_at)
            except Exception:
                pass  # Don't mask the original exception

        return False  # Never suppress exceptions

    def start(self) -> EcoCodeRun:
        """Begin tracking. Alternative to using as a context manager."""
        return self.__enter__()

    def stop(self) -> None:
        """Stop tracking and finalize the run. Alternative to context manager exit."""
        self.__exit__(None, None, None)

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


# Flag names whose next value is a secret. Keep this set lower-cased;
# matching is done case-insensitively.
_SENSITIVE_FLAG_NAMES: frozenset[str] = frozenset({
    # EcoCode / generic auth
    "--api-key", "--api_key", "--token", "-k", "-t",
    # HuggingFace
    "--hf-token", "--hf_token", "--huggingface-token", "--huggingface_token",
    # Weights & Biases
    "--wandb-key", "--wandb_key", "--wandb-api-key",
    # OpenAI / generic LLM providers
    "--openai-key", "--openai_key", "--openai-api-key", "--openai_api_key",
    # Generic secret/password flags
    "--password", "--passwd", "--secret",
    "--access-token", "--access_token",
    "--auth-token", "--auth_token",
    "--bearer", "--private-key", "--private_key",
    "--credentials", "--key",
})


def _sanitize_command(args: list[str]) -> str:
    """Redact the values of known secret flags (API keys, tokens) from sys.argv
    so they never reach the backend in the run's 'command' field."""
    sanitized = []
    skip_next = False
    for i, arg in enumerate(args):
        if skip_next:
            skip_next = False
            continue

        if "=" in arg:
            key, val = arg.split("=", 1)
            if key.lower() in _SENSITIVE_FLAG_NAMES:
                sanitized.append(f"{key}=********")
            else:
                sanitized.append(arg)
        elif arg.lower() in _SENSITIVE_FLAG_NAMES:
            sanitized.append(arg)
            if i + 1 < len(args):
                sanitized.append("********")
                skip_next = True
        else:
            sanitized.append(arg)

    return " ".join(sanitized)
