"""Locates the ``ecocode`` CLI binary that runs the energy monitor.

The wheel installs the binary into the environment's scripts directory
(``bin/`` on Linux/macOS, ``Scripts\\`` on Windows), next to ``python``. It is
looked for there first, then in the user scripts directory
(``pip install --user``), then on ``PATH``.
"""

from __future__ import annotations

import os
import shutil
import site
import sys
import sysconfig
import warnings
from typing import Iterator, Optional

EXE_NAME = "ecocode.exe" if os.name == "nt" else "ecocode"


def _scripts_dirs() -> Iterator[str]:
    for scheme in (None, f"{os.name}_user"):
        try:
            path = sysconfig.get_path("scripts", scheme) if scheme else sysconfig.get_path("scripts")
        except KeyError:
            continue
        if path:
            yield path


def _is_executable(path: str) -> bool:
    if not os.path.isfile(path):
        return False
    return os.name == "nt" or os.access(path, os.X_OK)


def find_ecocode_binary() -> Optional[str]:
    """Path of the ecocode binary, or None if it is not installed."""
    for directory in _scripts_dirs():
        candidate = os.path.join(directory, EXE_NAME)
        if _is_executable(candidate):
            return candidate
    found = shutil.which("ecocode")
    return found if found and _is_executable(found) else None


def _trusted_prefixes() -> list[str]:
    """Locations a legitimately installed ecocode binary lives in."""
    prefixes = [sys.prefix, sys.base_prefix, site.getuserbase(), os.path.expanduser("~/.cargo/bin")]
    if os.name == "nt":
        prefixes += [os.environ.get(v, "") for v in ("ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA")]
    else:
        prefixes += [os.path.expanduser("~/.local"), "/usr", "/opt", "/bin", "/sbin"]
    return [os.path.normcase(os.path.abspath(p)) for p in prefixes if p]


def _is_under(path: str, prefix: str) -> bool:
    try:
        return os.path.commonpath([path, prefix]) == prefix
    except ValueError:  # different drives on Windows
        return False


def resolve_ecocode_binary() -> str:
    """Locate and validate the ecocode CLI binary.

    CS-EXEC-001: warns when the binary is found outside the expected
    install locations, which can mean an unexpected ``PATH`` entry.
    """
    path = find_ecocode_binary()
    if path is None:
        raise RuntimeError(
            f"[ecocode] The '{EXE_NAME}' monitor binary was not found.\n"
            "It is installed with the SDK: pip install ecocode\n"
            "When working from a source checkout, build it with `cargo build --release` "
            "and put target/release on your PATH."
        )
    normalized = os.path.normcase(os.path.abspath(path))
    if not any(_is_under(normalized, p) for p in _trusted_prefixes()):
        warnings.warn(
            f"[ecocode] Monitor binary resolved from an unexpected path: '{path}'. "
            "If you did not install EcoCode here, check your PATH for unexpected entries.",
            stacklevel=4,
        )
    return path
