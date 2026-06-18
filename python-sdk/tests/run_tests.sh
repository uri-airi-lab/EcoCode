#!/usr/bin/env bash
#
# Run every test in tests/ one at a time, each in its own process.
#
# Each test file is a standalone script (has its own __main__), so we invoke
# `python <file>` per file rather than collecting them into a single pytest run.
#
# Usage:
#   ./run_tests.sh                 # run all test_*.py files
#   ./run_tests.sh --all           # also include demo_*.py files
#   ./run_tests.sh -k heavy        # only files whose name contains "heavy"
#   ./run_tests.sh --from heavy_hf # start at the first file matching "heavy_hf"
#   ./run_tests.sh tests/test_standalone.py tests/test_decorator.py
#
set -u

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TESTS_DIR="$SCRIPT_DIR"

# Prefer the project venv if present, else fall back to python3.
if [[ -x "$SCRIPT_DIR/env/bin/python" ]]; then
    PYTHON="$SCRIPT_DIR/env/bin/python"
else
    PYTHON="$(command -v python3 || command -v python)"
fi

INCLUDE_DEMOS=0
FILTER=""
FROM=""
EXPLICIT_FILES=()

while [[ $# -gt 0 ]]; do
    case "$1" in
        --all)   INCLUDE_DEMOS=1; shift ;;
        -k)      FILTER="${2:-}"; shift 2 ;;
        --from)  FROM="${2:-}"; shift 2 ;;
        -h|--help)
            sed -n '3,14p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
            exit 0 ;;
        *)       EXPLICIT_FILES+=("$1"); shift ;;
    esac
done

# Build the list of files to run.
declare -a FILES
if [[ ${#EXPLICIT_FILES[@]} -gt 0 ]]; then
    FILES=("${EXPLICIT_FILES[@]}")
else
    while IFS= read -r f; do FILES+=("$f"); done < <(
        ls "$TESTS_DIR"/test_*.py 2>/dev/null | xargs -n1 basename 2>/dev/null | sort
        [[ $INCLUDE_DEMOS -eq 1 ]] && ls "$TESTS_DIR"/demo_*.py 2>/dev/null | xargs -n1 basename 2>/dev/null | sort
    )
fi

if [[ -n "$FILTER" ]]; then
    declare -a KEPT
    for f in "${FILES[@]}"; do
        [[ "$(basename "$f")" == *"$FILTER"* ]] && KEPT+=("$f")
    done
    FILES=("${KEPT[@]:-}")
fi

# --from: drop everything before the first file whose basename matches $FROM,
# keeping the rest in order. Applies after -k/--all and to explicit lists too.
if [[ -n "$FROM" ]]; then
    declare -a TAIL
    found=0
    for f in "${FILES[@]:-}"; do
        [[ -z "$f" ]] && continue
        if [[ $found -eq 0 && "$(basename "$f")" == *"$FROM"* ]]; then
            found=1
        fi
        [[ $found -eq 1 ]] && TAIL+=("$f")
    done
    if [[ $found -eq 0 ]]; then
        echo "--from: no test file matched '$FROM'." >&2
        exit 1
    fi
    FILES=("${TAIL[@]}")
fi

if [[ ${#FILES[@]} -eq 0 || -z "${FILES[0]:-}" ]]; then
    echo "No test files matched." >&2
    exit 1
fi

echo "Python: $PYTHON"
echo "Running ${#FILES[@]} test(s), one process each."
echo "============================================================"

declare -a PASSED FAILED
START_ALL=$SECONDS

for f in "${FILES[@]}"; do
    name="$(basename "$f")"
    echo ""
    echo ">>> $name"
    echo "------------------------------------------------------------"
    start=$SECONDS
    # Run from the tests/ dir so relative paths inside scripts resolve.
    ( cd "$TESTS_DIR" && "$PYTHON" "$name" )
    rc=$?
    dur=$((SECONDS - start))
    if [[ $rc -eq 0 ]]; then
        echo "--- PASS ($name, ${dur}s)"
        PASSED+=("$name")
    else
        echo "--- FAIL ($name, exit $rc, ${dur}s)"
        FAILED+=("$name")
    fi
done

total_dur=$((SECONDS - START_ALL))
echo ""
echo "============================================================"
echo "Summary: ${#PASSED[@]} passed, ${#FAILED[@]} failed, ${total_dur}s total"
if [[ ${#FAILED[@]} -gt 0 ]]; then
    printf '  FAIL: %s\n' "${FAILED[@]}"
    exit 1
fi
echo "All tests passed."
