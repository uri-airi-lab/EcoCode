"""Core EcoCodeRun class — pure Python replacement for the PyO3 _core module.

Supports three usage patterns:
    # Context manager
    with ecocode.run("name", exporter="local") as run: ...

    # Decorator
    @ecocode.track("name", exporter="local")
    def train(): ...

    # Explicit start/stop
    run = ecocode.run("name", exporter="local")
    run.start()
    ...
    run.stop()

Exporter modes
--------------
exporter="local" (default)
    Records are written to a local SQLite database.
    No API key required. No network traffic.
    db_path controls where the database is stored
    (default: ~/.ecocode/ecocode.db).

exporter="terminal"
    Energy metrics are printed to stdout in real time.
    No API key, no network traffic.

exporter="csv" / exporter="json"
    Records written to the file specified by output_file.
    No API key, no network traffic.

exporter="online"
    Records sent to the EcoCode SaaS platform.
    Requires api_key (or ECOCODE_API_KEY env var).
    Use this to access the web dashboard and workspaces.
"""

from __future__ import annotations

import os
import platform
import shutil
import stat
import subprocess
import sys
from datetime import datetime, timezone
from typing import Any, Optional

from ._http import EcoCodeHTTPClient
from ._pipe import InheritedFdWriter, NullWriter, PipeWriter
from ._validation import (
    validate_api_key,
    validate_base_url,
    validate_marker_fd,
    validate_run_id,
)

_DEFAULT_BASE_URL = "https://ecocode.app"

# CS-CRED-001: Allowlist of environment variables forwarded to the monitor
# subprocess. Passing the full parent environment ({**os.environ, ...}) would
# spread every secret in the process environment into the child — visible via
# /proc/<pid>/environ on Linux. Only forward what the monitor actually needs.
_SUBPROCESS_ENV_PASSTHROUGH = frozenset({
    # Shell / runtime essentials
    "PATH", "HOME", "USER", "LOGNAME", "SHELL",
    "LANG", "LC_ALL", "LC_CTYPE", "LANGUAGE",
    "TMPDIR", "TMP", "TEMP",
    "XDG_RUNTIME_DIR", "XDG_DATA_DIRS",
    # Dynamic linker — required for native extensions
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
            import warnings
            warnings.warn(
                "[ecocode] The 'local' SQLite exporter is no longer supported and has been removed. "
                "Falling back to the 'terminal' exporter.",
                UserWarning,
                stacklevel=2,
            )
            self._exporter = "terminal"

        if db_path:
            import warnings
            warnings.warn(
                "[ecocode] The 'db_path' argument is obsolete and ignored because the local SQLite exporter was removed.",
                UserWarning,
                stacklevel=2,
            )

        self._output_file = output_file
        self._db_path = None  # Obsolete
        self._project_id = project_id
        self._interval = interval
        self._run_id: Optional[int] = None
        self._monitor_proc: Optional[subprocess.Popen] = None
        self._pipe: PipeWriter = NullWriter()
        self._client: Optional[EcoCodeHTTPClient] = None
        self._mode: str = "unknown"

        if self._exporter == "online":
            # Resolve API key and base URL from arguments then environment, and
            # validate both eagerly so misconfiguration fails at construction time
            # rather than at the first network call. (CS-CRED-002, CS-SSRF-001)
            raw_key = api_key or os.environ.get("ECOCODE_API_KEY")
            self._api_key: Optional[str] = validate_api_key(raw_key) if raw_key else None

            raw_url = base_url or os.environ.get("ECOCODE_API_URL", _DEFAULT_BASE_URL)
            self._base_url: str = validate_base_url(raw_url)
        else:
            # Local exporters never touch the network.
            self._api_key = None
            self._base_url = _DEFAULT_BASE_URL

        # Validate exporter-specific arguments early.
        if self._exporter in ("csv", "json", "logfile") and not self._output_file:
            raise ValueError(
                f"[ecocode] output_file is required when using exporter='{self._exporter}'."
            )

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

            # CS-INT-001: Validate run ID is a sane positive integer rather than
            # blindly casting whatever the env var contains.
            try:
                self._run_id = validate_run_id(env_run_id)
            except ValueError as exc:
                raise ValueError(
                    f"[ecocode] Invalid ECOCODE_RUN_ID: {exc}"
                ) from exc

            # CS-FD-001: Validate the inherited FD before opening it. A tampered
            # ECOCODE_MARKER_FD=1 would redirect all marker writes to stdout.
            try:
                fd = validate_marker_fd(env_marker_fd)
                self._pipe = InheritedFdWriter(fd)
            except ValueError as exc:
                import warnings
                warnings.warn(
                    f"[ecocode] Ignoring invalid ECOCODE_MARKER_FD: {exc} — "
                    "phase markers will not be recorded for this run.",
                    stacklevel=3,
                )
                self._pipe = NullWriter()

            return self

        # Standalone mode — branch on exporter type.
        if self._exporter in _LOCAL_EXPORTERS:
            return self._start_local_standalone()
        else:
            return self._start_online_standalone()

    def _start_local_standalone(self) -> EcoCodeRun:
        """Start a standalone run with a local exporter (no API key needed)."""
        ecocode_bin = _resolve_ecocode_binary()

        # Build safe environment for the subprocess.
        safe_env: dict[str, str] = {
            k: v
            for k, v in os.environ.items()
            if k in _SUBPROCESS_ENV_PASSTHROUGH
        }

        # For the local exporter we use `ecocode monitor-local` which writes to
        # a local exporter without any network calls. Run ID 0 is a placeholder;
        # the monitor will create/manage the run internally via the local exporter.
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

        self._client = EcoCodeHTTPClient(self._base_url, self._api_key)
        started_at = datetime.now(timezone.utc).isoformat()
        command = _sanitize_command(sys.argv) if hasattr(sys, "argv") and sys.argv else None

        try:
            self._run_id = self._client.create_run(
                project_id=self._project_id or 0,
                name=self._name,
                started_at=started_at,
                command=command,
            )
            self._client.save_system_info(self._run_id, _collect_system_info())
        except Exception as exc:
            import warnings
            warnings.warn(
                f"[ecocode] EcoCode backend unreachable or failed to initialize run: {exc}. "
                "Running in local fallback mode.",
                category=UserWarning,
                stacklevel=3,
            )
            self._mode = "noop"
            return self

        ecocode_bin = _resolve_ecocode_binary()

        # CS-CRED-001: Build a minimal environment for the monitor subprocess.
        safe_env: dict[str, str] = {
            k: v
            for k, v in os.environ.items()
            if k in _SUBPROCESS_ENV_PASSTHROUGH
        }
        safe_env["ECOCODE_API_KEY"] = self._api_key  # type: ignore[assignment]
        safe_env["ECOCODE_API_URL"] = self._base_url

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

    # --- Explicit start/stop (alternative to context manager) ---

    def start(self) -> EcoCodeRun:
        """Begin tracking. Alternative to using as a context manager."""
        return self.__enter__()

    def stop(self) -> None:
        """Stop tracking and finalize the run. Alternative to context manager exit."""
        self.__exit__(None, None, None)

    # --- Marker API ---

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


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def _resolve_ecocode_binary() -> str:
    """Locate and validate the ecocode CLI binary."""
    ecocode_bin = shutil.which("ecocode")
    if not ecocode_bin:
        raise RuntimeError(
            "[ecocode] CLI binary not found on PATH.\n"
            "Install it with: pip install ecocode-monitor\n"
            "Or reinstall:    pip install ecocode"
        )

    # CS-EXEC-001: Verify the resolved binary is actually executable and
    # warn if it was found outside a set of expected trusted locations.
    try:
        bin_stat = os.stat(ecocode_bin)
        if not (bin_stat.st_mode & stat.S_IXUSR):
            raise RuntimeError(
                f"[ecocode] Resolved binary '{ecocode_bin}' is not executable."
            )
    except OSError as exc:
        raise RuntimeError(
            f"[ecocode] Cannot stat binary '{ecocode_bin}': {exc}"
        ) from exc

    _trusted_prefixes = (
        sys.prefix,                       # active Python venv / conda env
        os.path.expanduser("~/.local"),   # user-local installs
        "/usr", "/opt", "/bin", "/sbin",  # standard system locations
    )
    if not any(ecocode_bin.startswith(p) for p in _trusted_prefixes):
        import warnings
        warnings.warn(
            f"[ecocode] Monitor binary resolved from an unexpected path: "
            f"'{ecocode_bin}'. If you did not install EcoCode here, "
            "check your PATH for unexpected entries.",
            stacklevel=4,
        )

    return ecocode_bin


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
            # CS-DOS-001: Guard against bind-mounted or crafted /proc/meminfo
            # that could be arbitrarily large. The real kernel file has ~50
            # entries; bail out if we exceed that to avoid memory exhaustion.
            for i, line in enumerate(f):
                if i >= 60:
                    break
                if line.startswith("MemTotal:"):
                    parts = line.split()
                    if len(parts) >= 2 and parts[1].isdigit():
                        kb = int(parts[1])
                        info["ram_gb"] = round(kb / 1_048_576, 1)
                    break
    except OSError:
        pass

    # Try to query nvidia-smi for GPU information
    try:
        nvidia_smi_bin = shutil.which("nvidia-smi")
        if nvidia_smi_bin:
            res = subprocess.run(
                [nvidia_smi_bin, "--query-gpu=name", "--format=csv,noheader"],
                capture_output=True,
                text=True,
                check=False,
                timeout=5,
            )
            if res.returncode == 0:
                gpus = [line.strip() for line in res.stdout.strip().splitlines() if line.strip()]
                if gpus:
                    info["gpu_count"] = len(gpus)
                    info["gpu_names"] = ", ".join(gpus)
    except Exception:
        pass

    return info


# CS-LEAK-001: Expanded set of flag names that precede a secret value.
# Keep this set lower-cased; matching is done case-insensitively.
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
    """Sanitize sensitive flag values (like API keys/tokens) from sys.argv.

    CS-LEAK-001: The original set only covered four flags. The expanded set
    covers all common ML tooling secret flags so they are never transmitted
    to the backend in the run's 'command' field.
    """
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
