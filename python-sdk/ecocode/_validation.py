"""
Input validation helpers for the EcoCode SDK.

All validators raise ``ValueError`` with a clear message on bad input so that
misconfigured environments fail loudly rather than silently forwarding credentials
or corrupted data to the backend.

Security findings addressed:
  CS-CRED-002  validate_api_key   — reject malformed / injected API key strings
  CS-SSRF-001  validate_base_url  — reject disallowed schemes / plaintext HTTP outside localhost
  CS-INT-001   validate_run_id    — reject out-of-range / non-positive run IDs
  CS-FD-001    validate_marker_fd — reject stdin/stdout/stderr FD aliases
  CS-TYPE-001  validate_metric_key — reject keys with unsafe characters / lengths
"""

from __future__ import annotations

import os
import re
import urllib.parse

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
# Base URL (CS-SSRF-001)
# ---------------------------------------------------------------------------

_LOCALHOST_HOSTNAMES = frozenset({"localhost", "127.0.0.1", "::1", "[::1]"})
_ALLOWED_SCHEMES = frozenset({"https", "http"})


def validate_base_url(url: str) -> str:
    """Validate and return *url* as a safe API base URL.

    Scheme must be ``https``, or ``http`` only for localhost. Other schemes
    (``file://``, ``ftp://``, ``data:``, etc.) and embedded credentials
    (``user:pass@host``) are rejected. Raises ValueError otherwise.
    """
    if not isinstance(url, str) or not url.strip():
        raise ValueError(
            "ECOCODE_API_URL must be a non-empty string. "
            f"Got: {url!r}"
        )

    try:
        parsed = urllib.parse.urlparse(url)
    except Exception as exc:
        raise ValueError(f"ECOCODE_API_URL could not be parsed: {exc}") from exc

    scheme = (parsed.scheme or "").lower()

    if scheme not in _ALLOWED_SCHEMES:
        raise ValueError(
            f"ECOCODE_API_URL uses a disallowed scheme '{scheme}://'. "
            "Only 'https://' is permitted (or 'http://' for localhost development). "
            f"Received: {url!r}"
        )

    # Reject embedded credentials (user:password@host)
    if parsed.username or parsed.password:
        raise ValueError(
            "ECOCODE_API_URL must not contain embedded credentials (user:password@host). "
            "Use the api_key parameter or ECOCODE_API_KEY environment variable instead."
        )

    hostname = (parsed.hostname or "").lower()

    if scheme == "http" and hostname not in _LOCALHOST_HOSTNAMES:
        raise ValueError(
            f"Insecure HTTP endpoint '{url}' rejected. "
            "Transmitting credentials over plaintext HTTP exposes your API key. "
            "Use 'https://' to protect credentials in transit. "
            "HTTP is only permitted for localhost addresses "
            "(localhost, 127.0.0.1, ::1) during local development."
        )

    return url


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
