"""Linux and macOS: the marker pipe is an inherited file descriptor."""

from __future__ import annotations

import os
from typing import IO

from .._validation import validate_marker_fd


def open_marker_fd(raw: str) -> IO[bytes]:
    """Opens the descriptor named by ``ECOCODE_MARKER_FD`` for writing.

    The value is validated first: it must be > 2 and open in this
    process, so a tampered value cannot redirect markers to stdout.
    """
    fd = validate_marker_fd(raw)
    return os.fdopen(fd, "wb", closefd=True)
