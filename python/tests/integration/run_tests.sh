#!/usr/bin/env bash
#
# Run the heavy integration tests one file at a time, each in its own process,
# so one framework's crash or leaked CUDA memory can't take the others down.
# Without ECOCODE_API_KEY the tests record locally with fake sensors.
#
# Usage:
#   ./run_tests.sh                 # run all test_*.py files
#   ./run_tests.sh -k vllm         # only files whose name contains "vllm"
#   ./run_tests.sh --from heavy_hf # start at the first file matching "heavy_hf"
#   ./run_tests.sh test_heavy_hf.py test_heavy_trl.py
#
set -u

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TESTS_DIR="$SCRIPT_DIR"

PYTHON="${PYTHON:-$(command -v python3 || command -v python)}"

FILTER=""
FROM=""
EXPLICIT_FILES=()

while [[ $# -gt 0 ]]; do
    case "$1" in
        -k)      FILTER="${2:-}"; shift 2 ;;
        --from)  FROM="${2:-}"; shift 2 ;;
        -h|--help)
            sed -n '3,11p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
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
    )
fi

if [[ -n "$FILTER" ]]; then
    declare -a KEPT
    for f in "${FILES[@]}"; do
        [[ "$(basename "$f")" == *"$FILTER"* ]] && KEPT+=("$f")
    done
    FILES=("${KEPT[@]:-}")
fi

# --from: drop everything before the first file matching $FROM.
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
    # Run from the package root so the shared conftest.py applies.
    ( cd "$TESTS_DIR/../.." && "$PYTHON" -m pytest "tests/integration/$name" -x -s -q -p no:cacheprovider )
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
