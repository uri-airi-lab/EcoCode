"""Marker writers: how the SDK sends markers to the ecocode monitor.

- Standalone mode: the SDK starts ``ecocode monitor`` and writes to its stdin
  (:class:`PipeWriter`).
- Attached mode (``ecocode run -- python train.py``): the CLI passes a pipe to
  this process (a file descriptor in ``ECOCODE_MARKER_FD`` on Linux/macOS, a
  handle in ``ECOCODE_MARKER_HANDLE`` on Windows), opened by
  :func:`open_inherited_writer`.
- Non-zero DDP ranks and inactive runs use :class:`NullWriter`.

Every marker is one line of JSON with a ``sent_at_ns`` timestamp.
"""

from __future__ import annotations

import json
import os
import time
from typing import IO, Any, Mapping, Optional

# Environment variables set by `ecocode run` for the child process.
MARKER_FD_ENV = "ECOCODE_MARKER_FD"
MARKER_HANDLE_ENV = "ECOCODE_MARKER_HANDLE"


class PipeWriter:
    """Writes newline-delimited JSON to a binary stream (the monitor's stdin,
    or an inherited pipe). A closed pipe means the monitor exited: markers are
    then dropped silently."""

    def __init__(self, stream: IO[bytes]) -> None:
        self._stream = stream

    def send(self, event: dict[str, Any]) -> None:
        try:
            event = {**event, "sent_at_ns": time.time_ns()}
            self._stream.write((json.dumps(event) + "\n").encode())
            self._stream.flush()
        except (BrokenPipeError, OSError, ValueError):
            pass  # Monitor exited or stream closed; drop the marker

    def close(self) -> None:
        try:
            self._stream.close()
        except OSError:
            pass


class NullWriter:
    """No-op writer for DDP non-zero ranks or pre-enter state."""

    def send(self, event: dict[str, Any]) -> None:
        pass

    def close(self) -> None:
        pass


def marker_pipe_in(env: Mapping[str, str]) -> bool:
    """Whether ``env`` carries a marker pipe from ``ecocode run``."""
    key = MARKER_HANDLE_ENV if os.name == "nt" else MARKER_FD_ENV
    return bool(env.get(key))


def open_inherited_writer(env: Mapping[str, str]) -> Optional[PipeWriter]:
    """Opens the marker pipe passed by ``ecocode run``, or returns None if
    there is none. Raises ValueError if the variable is set but invalid."""
    if os.name == "nt":
        raw = env.get(MARKER_HANDLE_ENV)
        if not raw:
            return None
        from .windows import open_marker_handle

        return PipeWriter(open_marker_handle(raw))
    raw = env.get(MARKER_FD_ENV)
    if not raw:
        return None
    from .posix import open_marker_fd

    return PipeWriter(open_marker_fd(raw))


__all__ = [
    "MARKER_FD_ENV",
    "MARKER_HANDLE_ENV",
    "NullWriter",
    "PipeWriter",
    "marker_pipe_in",
    "open_inherited_writer",
]
