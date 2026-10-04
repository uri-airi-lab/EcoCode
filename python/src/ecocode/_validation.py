"""
Input validation helpers for the EcoCode SDK.

All validators raise ``ValueError`` with a clear message on bad input so that
misconfigured environments fail loudly rather than silently forwarding credentials
or corrupted data to the backend.

Security findings addressed:
  CS-CRED-002  validate_api_key       — reject malformed / injected API key strings
  CS-INT-001   validate_run_id        — reject out-of-range / non-positive run IDs
  CS-FD-001    validate_marker_fd     — reject stdin/stdout/stderr FD aliases (Linux, macOS)
  CS-FD-001    validate_marker_handle — reject malformed Windows handle values
  CS-TYPE-001  validate_metric_key    — reject keys with unsafe characters / lengths

The backend URL is fixed (see ``_http.BASE_URL``), so it needs no validation.
"""

from __future__ import annotations

import os
import re

# ---------------------------------------------------------------------------
# API key (CS-CRED-002)
# ---------------------------------------------------------------------------

_API_KEY_RE = re.compile(r"^(?:eco|ecocode)_[A-Za-z0-9_\-]{8,240}$")
_CTRL_CHARS_RE = re.compile(r"[\x00-\x1f\x7f]")


def validate_api_key(key: str) -> str:
    """Validate and return *key*.

    Must be non-empty, start with ``eco_`` or ``ecocode_``, contain only
    alphanumerics/underscore/hyphen after the prefix, have no control chars
    (blocks header injection), and be 12-248 characters total. Raises
    ValueError otherwise.
    """
    if not isinstance(key, str) or not key:
        raise ValueError(
            "ECOCODE_API_KEY must be a non-empty string. "
            "Generate a key from the EcoCode dashboard."
        )
    if _CTRL_CHARS_RE.search(key):
        raise ValueError(
            "ECOCODE_API_KEY contains control characters and will be rejected. "
            "Check that the key was not corrupted when reading from the environment."
        )
    if not _API_KEY_RE.match(key):
        raise ValueError(
            f"ECOCODE_API_KEY does not match the expected format (ecocode_<token> or eco_<token>). "
            f"Received a key starting with: {key[:12]!r}... — "
            "generate a valid key from the EcoCode dashboard."
        )
    return key


# ---------------------------------------------------------------------------
# Run ID (CS-INT-001)
# ---------------------------------------------------------------------------

_RUN_ID_MAX = 2 ** 53  # JavaScript-safe integer boundary (matches most backends)


def validate_run_id(raw: str) -> int:
    """Parse and validate the ``ECOCODE_RUN_ID`` environment variable.

    Must be a base-10 integer, >= 1, and <= 2**53 (JavaScript-safe integer
    boundary). Raises ValueError otherwise.
    """
    try:
        run_id = int(raw, 10)
    except (ValueError, TypeError):
        raise ValueError(
            f"ECOCODE_RUN_ID must be a positive integer, got: {raw!r}"
        )

    if run_id < 1:
        raise ValueError(
            f"ECOCODE_RUN_ID must be >= 1, got: {run_id}. "
            "A run ID of 0 or negative is not valid."
        )

    if run_id > _RUN_ID_MAX:
        raise ValueError(
            f"ECOCODE_RUN_ID {run_id} exceeds the maximum allowed value "
            f"({_RUN_ID_MAX}). The value may have been corrupted."
        )

    return run_id


# ---------------------------------------------------------------------------
# Marker file descriptor (CS-FD-001)
# ---------------------------------------------------------------------------


def validate_marker_fd(raw: str) -> int:
    """Parse and validate the ``ECOCODE_MARKER_FD`` environment variable.

    Must be a base-10 integer, > 2 (not stdin=0/stdout=1/stderr=2), and an FD
    that's actually open in this process (checked via ``os.fstat``). Raises
    ValueError otherwise.
    """
    try:
        fd = int(raw, 10)
    except (ValueError, TypeError):
        raise ValueError(
            f"ECOCODE_MARKER_FD must be an integer file descriptor, got: {raw!r}"
        )

    if fd <= 2:
        raise ValueError(
            f"ECOCODE_MARKER_FD={fd} is not allowed. "
            "FD 0 (stdin), 1 (stdout), and 2 (stderr) are reserved and must not "
            "be used as a marker pipe. The CLI should pass a dedicated write-only FD."
        )

    try:
        os.fstat(fd)
    except OSError:
        raise ValueError(
            f"ECOCODE_MARKER_FD={fd} is not a valid open file descriptor in this "
            "process. The FD may have been closed or the environment variable may "
            "have been tampered with."
        )

    return fd


# ---------------------------------------------------------------------------
# Marker handle (CS-FD-001, Windows)
# ---------------------------------------------------------------------------

_HANDLE_MAX = 2 ** 63 - 1


def validate_marker_handle(raw: str) -> int:
    """Parse and validate the ``ECOCODE_MARKER_HANDLE`` environment variable.

    On Windows the CLI passes the marker pipe as an inherited handle value.
    Must be a positive base-10 integer. Whether it is an open handle is checked
    when it is opened (``msvcrt.open_osfhandle``). Raises ValueError otherwise.
    """
    try:
        handle = int(raw, 10)
    except (ValueError, TypeError):
        raise ValueError(
            f"ECOCODE_MARKER_HANDLE must be an integer handle value, got: {raw!r}"
        )
    if not 0 < handle <= _HANDLE_MAX:
        raise ValueError(f"ECOCODE_MARKER_HANDLE={handle} is out of range.")
    return handle


# ---------------------------------------------------------------------------
# Custom metric key (CS-TYPE-001)
# ---------------------------------------------------------------------------

_METRIC_KEY_RE = re.compile(r"^[A-Za-z0-9_\-]{1,128}$")


def validate_metric_key(key: str) -> str:
    """Validate a custom metric key before forwarding it to the backend.

    Must be non-empty, alphanumeric/underscore/hyphen only, and at most 128
    characters. Raises ValueError otherwise.
    """
    if not isinstance(key, str) or not key:
        raise ValueError("Metric key must be a non-empty string.")

    if not _METRIC_KEY_RE.match(key):
        raise ValueError(
            f"Metric key {key!r} contains invalid characters or exceeds 128 chars. "
            "Only alphanumeric characters, underscores, and hyphens are allowed."
        )

    return key
