"""Pipe-based marker writers for communicating with the monitor subprocess."""

from __future__ import annotations

import json
import os
from typing import IO, Any


class PipeWriter:
    """Writes newline-delimited JSON to a subprocess stdin pipe."""

    def __init__(self, stream: IO[bytes]) -> None:
        self._stream = stream

    def send(self, event: dict[str, Any]) -> None:
        try:
            line = json.dumps(event) + "\n"
            self._stream.write(line.encode())
            self._stream.flush()
        except (BrokenPipeError, OSError):
            pass  # Monitor exited — silently degrade

    def close(self) -> None:
        try:
            self._stream.close()
        except OSError:
            pass


class InheritedFdWriter:
    """Writes markers to an inherited file descriptor (attached mode).

    The *fd* argument **must** be pre-validated by the caller (``_run.py``)
    using ``_validation.validate_marker_fd`` before this class is instantiated.
    That function ensures the FD is > 2 (not stdin/stdout/stderr) and is an
    open file descriptor in the current process. (CS-FD-001)
    """

    def __init__(self, fd: int) -> None:
        # fd is already validated by validate_marker_fd in _run.py
        self._file = os.fdopen(fd, "w", closefd=True)

    def send(self, event: dict[str, Any]) -> None:
        try:
            self._file.write(json.dumps(event) + "\n")
            self._file.flush()
        except (BrokenPipeError, OSError):
            pass

    def close(self) -> None:
        try:
            self._file.close()
        except OSError:
            pass


class NullWriter:
    """No-op writer for DDP non-zero ranks or pre-enter state."""

    def send(self, event: dict[str, Any]) -> None:
        pass

    def close(self) -> None:
        pass
