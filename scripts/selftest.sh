#!/usr/bin/env bash
#
# EcoCode self-test for Linux and macOS: builds, runs every test, measures a
# real workload with this machine's sensors, and writes selftest-report.txt.
#
#   scripts/selftest.sh
#
# Needs Rust (rustup) and Python 3; pytest is optional (pip install pytest).
# Send selftest-report.txt back when something fails.
set -u

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT" || exit 1
REPORT="$ROOT/selftest-report.txt"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
PY="$(command -v python3 || command -v python || true)"
BIN="$ROOT/target/release/ecocode"
CHECKS="$ROOT/scripts/selftest_checks.py"

: > "$REPORT"
declare -a RESULTS
log() { echo "$@" | tee -a "$REPORT"; }

# step NAME COMMAND... — runs a step, records PASS/FAIL, keeps the output in the report.
step() {
    local name=$1; shift
    log ""
    log "=== $name"
    if "$@" >"$WORK/out" 2>&1; then
        RESULTS+=("PASS  $name")
        grep -v "^test result: ok. 0 passed" "$WORK/out" | tail -n 15 | tee -a "$REPORT"
    else
        RESULTS+=("FAIL  $name")
        tail -n 60 "$WORK/out" | tee -a "$REPORT"
    fi
    # cargo test: one total line instead of one line per test binary.
    awk '/^test result:/ {p+=$4; f+=$6; i+=$8} END {if (NR && (p+f+i)) printf "  total: %d passed, %d failed, %d ignored\n", p, f, i}' \
        "$WORK/out" | tee -a "$REPORT"
}
skip() { log ""; log "=== $1"; log "  skipped: $2"; RESULTS+=("SKIP  $1 ($2)"); }

log "EcoCode self-test — $(date -u +%Y-%m-%dT%H:%M:%SZ)"
log "$(uname -a)"
log "$(cargo --version 2>&1) / $(rustc --version 2>&1)"
log "Python: ${PY:+$($PY --version 2>&1)}${PY:-not found}"

step "build (cargo build --release)" cargo build --release --locked
if [[ ! -x "$BIN" ]]; then
    log "Build failed; stopping."
    printf '%s\n' "${RESULTS[@]}" | tee -a "$REPORT"
    exit 1
fi

step "ecocode doctor" "$BIN" doctor
"$BIN" doctor --json > "$WORK/doctor.json"

step "Rust tests (cargo test --workspace)" cargo test --workspace --locked
if grep -q '"domain": "cpu"' "$WORK/doctor.json"; then
    step "hardware tests (cargo test -- --ignored)" cargo test --workspace --locked -- --ignored
else
    skip "hardware tests" "no CPU energy sensor (see ecocode doctor)"
fi

if [[ -z "$PY" ]]; then
    skip "Python SDK tests" "python3 not found"
    skip "measured run" "python3 not found"
else
    if "$PY" -c "import pytest" 2>/dev/null; then
        step "Python SDK tests (pytest)" env ECOCODE_BIN="$BIN" "$PY" -m pytest -q -p no:cacheprovider "$ROOT/python/tests/unit" "$ROOT/python/tests/e2e"
    else
        skip "Python SDK tests" "pytest not installed (pip install pytest)"
    fi
    step "measured run: 10 s CPU burn under ecocode run" bash -c \
        "'$BIN' run -o json -f '$WORK/run.json' -i 1 -- '$PY' '$CHECKS' burn 10 && '$PY' '$CHECKS' check-run '$WORK/doctor.json' '$WORK/run.json'"
    step "SDK standalone run (real sensors)" env PATH="$ROOT/target/release:$PATH" \
        "$PY" "$CHECKS" sdk-standalone "$WORK/doctor.json" "$WORK/sdk.json"
    step "SDK attached run (ecocode run -- python)" bash -c \
        "'$BIN' run -o json -f '$WORK/attached.json' -i 0.5 -- '$PY' '$CHECKS' sdk-attached-child && '$PY' '$CHECKS' check-attached '$WORK/attached.json'"
fi

log ""
log "=== summary"
printf '%s\n' "${RESULTS[@]}" | tee -a "$REPORT"
log ""
log "Report written to $REPORT"
! printf '%s\n' "${RESULTS[@]}" | grep -q '^FAIL'
