"""Checks shared by selftest.sh (Linux/macOS) and selftest.ps1 (Windows).

    python selftest_checks.py burn SECONDS
    python selftest_checks.py check-run DOCTOR_JSON RECORDS_JSON
    python selftest_checks.py sdk-standalone DOCTOR_JSON OUT_JSON
    python selftest_checks.py sdk-attached-child
    python selftest_checks.py check-attached RECORDS_JSON

Exit code 0 = pass. Only the standard library and the SDK source are used.
"""

import json
import os
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "python" / "src"))


def burn(seconds: float) -> None:
    """Keeps one CPU core busy."""
    end = time.time() + seconds
    x = 0
    while time.time() < end:
        x += 1


def load(path):
    return json.loads(Path(path).read_text(encoding="utf-8"))


def channel_domains(doctor) -> set:
    return {c["domain"] for c in doctor["channels"]}


def watts(records, key) -> float:
    secs = sum(r["elapsed_secs"] for r in records) or 1.0
    return sum((r.get(key) or 0.0) for r in records) / secs


def check_energy(doctor, records, label) -> bool:
    """CPU energy must be non-zero when the machine has a CPU energy sensor."""
    if not records:
        print(f"  {label}: no records")
        return False
    domains = channel_domains(doctor)
    print(f"  {label}: {len(records)} records, CPU {sum(r['cpu_usage'] for r in records) / len(records):.1f}% avg")
    print(f"    CPU {watts(records, 'cpu_energy'):.3f} W, GPU {watts(records, 'gpu_energy'):.3f} W, "
          f"DRAM {watts(records, 'mem_energy'):.3f} W (attributed to the run)")
    if "cpu" not in domains:
        print("    no CPU energy sensor on this machine (see ecocode doctor): energy not checked")
        return True
    ok = watts(records, "cpu_energy") > 0
    if not ok:
        print("    FAIL: the machine has a CPU energy sensor but the run measured 0 J")
    return ok


def main() -> int:
    cmd, args = sys.argv[1], sys.argv[2:]
    if cmd == "burn":
        burn(float(args[0]))
        return 0
    if cmd == "check-run":
        return 0 if check_energy(load(args[0]), load(args[1]), "ecocode run") else 1
    if cmd == "sdk-standalone":
        import warnings

        import ecocode

        warnings.filterwarnings("ignore", message=".*unexpected path.*")
        with ecocode.run("selftest", exporter="json", output_file=args[1], interval=0.5) as run:
            run.mark("phase", "forward")
            burn(2)
            run.mark("phase", "eval")
            run.log_step(1)
            burn(2)
        records = load(args[1])
        phases = {r["phase"] for r in records}
        ok = check_energy(load(args[0]), records, "SDK standalone")
        if not {"forward", "eval"} <= phases:
            print(f"    FAIL: markers missing, phases seen: {sorted(p for p in phases if p)}")
            ok = False
        return 0 if ok else 1
    if cmd == "sdk-attached-child":
        import ecocode

        with ecocode.run("selftest-attached") as run:
            if run._mode != "attached":
                print(f"    FAIL: SDK mode is {run._mode!r}, expected 'attached'")
                return 1
            run.mark("phase", "forward")
            run.log_step(7)
            burn(2)
        return 0
    if cmd == "check-attached":
        last = load(args[0])[-1]
        ok = last.get("phase") == "forward" and last.get("training_step") == 7
        print(f"  SDK attached: last record phase={last.get('phase')!r} step={last.get('training_step')!r}")
        return 0 if ok else 1
    print(f"unknown command {cmd}", file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main())
