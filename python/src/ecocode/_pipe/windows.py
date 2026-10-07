"""Windows: the marker pipe is an inherited handle."""

from __future__ import annotations

import os
from typing import IO

from .._validation import validate_marker_handle


def open_marker_handle(raw: str) -> IO[bytes]:
    """Opens the handle named by ``ECOCODE_MARKER_HANDLE`` for writing.

    ``msvcrt.open_osfhandle`` wraps the inherited Win32 handle in a C runtime
    descriptor; closing the returned file closes the handle.
    """
    import msvcrt

    handle = validate_marker_handle(raw)
    try:
        fd = msvcrt.open_osfhandle(handle, os.O_WRONLY)
    except OSError as exc:
        raise ValueError(f"ECOCODE_MARKER_HANDLE={handle} is not an open handle: {exc}") from exc
    return os.fdopen(fd, "wb", closefd=True)
