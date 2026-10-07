import json
import os
import subprocess
import sys
import textwrap
import time
from pathlib import Path

import ecocode

SRC = str(Path(__file__).resolve().parents[2] / "src")


def burn(seconds):
    end = time.time() + seconds
    while time.time() < end:
        pass


def test_standalone_json_run_records_markers(ecocode_bin):
    with ecocode.run("e2e", exporter="json", output_file="out.json", interval=0.2) as run:
        run.mark("phase", "forward")
        run.log_step(1)
        burn(0.7)
        run.mark("phase", "eval")
        run.log_accuracy(0.9)
        run.log_custom("grad_norm", 0.25)
        burn(0.7)
    assert run._mode == "standalone_local"
    assert run._monitor_proc.poll() is not None, "monitor exits when the run ends"

    records = json.loads(Path("out.json").read_text())
    assert len(records) >= 4
    assert {r["phase"] for r in records} >= {"forward", "eval"}
    last = records[-1]
    assert last["pid"] == os.getpid()
    assert last["accuracy"] == 0.9 and last["custom_metrics"] == {"grad_norm": 0.25}
    assert last["gpu_metrics"][0]["gpu_energy"] > 0  # fake GPU energy is attributed
    assert sum(r["cpu_usage"] for r in records) > 0


def test_decorator_and_start_stop(ecocode_bin):
    @ecocode.track("decorated", exporter="csv", output_file="dec.csv", interval=0.2)
    def train():
        burn(0.6)
        return 42

    assert train() == 42
    header, *rows = Path("dec.csv").read_text().splitlines()
    assert header.startswith("ID,Run ID,PID") and len(rows) >= 2

    run = ecocode.run("manual", exporter="json", output_file="manual.json", interval=0.2)
    run.start()
    run.log_tokens(128)
    burn(0.6)
    run.stop()
    assert json.loads(Path("manual.json").read_text())[-1]["tokens_processed"] == 128


def test_attached_mode_under_ecocode_run(ecocode_bin):
    """`ecocode run -- python train.py`: the SDK finds the run ID and marker pipe
    (a file descriptor on Linux/macOS, a handle on Windows) in its environment."""
    script = Path("train.py")
    script.write_text(textwrap.dedent(f"""
        import sys, time
        sys.path.insert(0, {SRC!r})
        import ecocode
        with ecocode.run("attached") as run:
            assert run._mode == "attached", run._mode
            run.mark("phase", "forward")
            run.log_step(5)
            end = time.time() + 1.0
            while time.time() < end:
                pass
    """))
    result = subprocess.run(
        [str(ecocode_bin), "run", "-o", "json", "-f", "attached.json", "-i", "0.2", "--", sys.executable, str(script)],
        capture_output=True,
        encoding="utf-8",  # ecocode writes UTF-8 when piped (Windows defaults to the ANSI code page)
        timeout=60,
    )
    assert result.returncode == 0, result.stderr
    assert "Marker pipe latency" in result.stdout
    last = json.loads(Path("attached.json").read_text())[-1]
    assert last["phase"] == "forward" and last["training_step"] == 5
