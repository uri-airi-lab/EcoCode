"""Hardware summary sent once per run (online standalone mode), using only the
standard library on Linux, macOS and Windows."""

from __future__ import annotations

import ctypes
import os
import platform
import shutil
import subprocess
import sys
from typing import Optional


def _linux_cpu_name() -> Optional[str]:
    try:
        with open("/proc/cpuinfo") as f:
            for i, line in enumerate(f):
                if i > 200:  # CS-DOS-001: never read an unbounded file
                    break
                if line.lower().startswith("model name"):
                    return line.split(":", 1)[1].strip()
    except OSError:
        pass
    return None


def _linux_ram_bytes() -> Optional[int]:
    try:
        with open("/proc/meminfo") as f:
            # CS-DOS-001: the real file has ~50 lines; stop past that.
            for i, line in enumerate(f):
                if i >= 60:
                    break
                if line.startswith("MemTotal:"):
                    parts = line.split()
                    if len(parts) >= 2 and parts[1].isdigit():
                        return int(parts[1]) * 1024
    except OSError:
        pass
    return None


def _sysctl(name: str, kind: str) -> object:
    """Reads a macOS sysctl as "u64" or "str" without spawning a process."""
    libc = ctypes.CDLL(None)
    size = ctypes.c_size_t(0)
    if libc.sysctlbyname(name.encode(), None, ctypes.byref(size), None, 0) != 0 or size.value == 0:
        return None
    buf = ctypes.create_string_buffer(size.value)
    if libc.sysctlbyname(name.encode(), buf, ctypes.byref(size), None, 0) != 0:
        return None
    if kind == "u64":
        return int.from_bytes(buf.raw[: size.value], sys.byteorder)
    return buf.value.decode(errors="replace").strip() or None


def _windows_cpu_name() -> Optional[str]:
    try:
        import winreg

        key = winreg.OpenKey(winreg.HKEY_LOCAL_MACHINE, r"HARDWARE\DESCRIPTION\System\CentralProcessor\0")
        value, _ = winreg.QueryValueEx(key, "ProcessorNameString")
        return str(value).strip()
    except OSError:
        return None


def _windows_ram_bytes() -> Optional[int]:
    class MEMORYSTATUSEX(ctypes.Structure):
        _fields_ = [
            ("dwLength", ctypes.c_ulong),
            ("dwMemoryLoad", ctypes.c_ulong),
            ("ullTotalPhys", ctypes.c_ulonglong),
            ("ullAvailPhys", ctypes.c_ulonglong),
            ("ullTotalPageFile", ctypes.c_ulonglong),
            ("ullAvailPageFile", ctypes.c_ulonglong),
            ("ullTotalVirtual", ctypes.c_ulonglong),
            ("ullAvailVirtual", ctypes.c_ulonglong),
            ("ullAvailExtendedVirtual", ctypes.c_ulonglong),
        ]

    status = MEMORYSTATUSEX()
    status.dwLength = ctypes.sizeof(MEMORYSTATUSEX)
    if ctypes.windll.kernel32.GlobalMemoryStatusEx(ctypes.byref(status)):  # type: ignore[attr-defined]
        return int(status.ullTotalPhys)
    return None


def _nvidia_gpus() -> list[str]:
    nvidia_smi = shutil.which("nvidia-smi")
    if not nvidia_smi:
        return []
    try:
        res = subprocess.run(
            [nvidia_smi, "--query-gpu=name", "--format=csv,noheader"],
            capture_output=True,
            text=True,
            check=False,
            timeout=5,
        )
    except (OSError, subprocess.SubprocessError):
        return []
    if res.returncode != 0:
        return []
    return [line.strip() for line in res.stdout.splitlines() if line.strip()]


def collect_system_info() -> dict:
    """CPU, core count, RAM and GPUs, in the backend's system-info format."""
    cpu_name: Optional[str] = None
    ram: Optional[int] = None
    gpus = _nvidia_gpus()
    try:
        if sys.platform.startswith("linux"):
            cpu_name, ram = _linux_cpu_name(), _linux_ram_bytes()
        elif sys.platform == "darwin":
            cpu_name = _sysctl("machdep.cpu.brand_string", "str")  # type: ignore[assignment]
            ram = _sysctl("hw.memsize", "u64")  # type: ignore[assignment]
            # Apple Silicon's GPU is part of the chip; the CLI reports it as GPU 0.
            if not gpus and _sysctl("hw.optional.arm64", "u64") == 1 and cpu_name:
                gpus = [f"{cpu_name} GPU"]
        elif os.name == "nt":
            cpu_name, ram = _windows_cpu_name(), _windows_ram_bytes()
    except Exception:
        pass
    return {
        "cpu_name": cpu_name or platform.processor() or platform.machine() or "Unknown",
        "cpu_cores": os.cpu_count() or 0,
        "ram_gb": round(ram / 1_073_741_824, 1) if ram else 0.0,
        "gpu_count": len(gpus),
        "gpu_names": ", ".join(gpus),
    }
