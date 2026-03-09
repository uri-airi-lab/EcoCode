#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEST_DIR="${TMPDIR:-/tmp}/ecocode-admin-smoke"
DB_PATH="$TEST_DIR/ecocode-test.db"
BACKEND_LOG="$TEST_DIR/backend.log"
RUN1_LOG="$TEST_DIR/run1.log"
RUN2_LOG="$TEST_DIR/run2.log"
RUN3_LOG="$TEST_DIR/run3.log"
RUN4_LOG="$TEST_DIR/run4.log"
RUN5_LOG="$TEST_DIR/run5.log"

mkdir -p "$TEST_DIR"
rm -f "$DB_PATH" "$DB_PATH-shm" "$DB_PATH-wal" "$BACKEND_LOG" "$RUN1_LOG" "$RUN2_LOG" "$RUN3_LOG" "$RUN4_LOG" "$RUN5_LOG"

export ECOCODE_DB_URL="sqlite://$DB_PATH"

cleanup() {
  if [[ -n "${RUN4_PID:-}" ]]; then
    kill "$RUN4_PID" 2>/dev/null || true
    wait "$RUN4_PID" 2>/dev/null || true
  fi
  if [[ -n "${RUN5_PID:-}" ]]; then
    kill "$RUN5_PID" 2>/dev/null || true
    wait "$RUN5_PID" 2>/dev/null || true
  fi
  if [[ -n "${BACKEND_PID:-}" ]]; then
    kill "$BACKEND_PID" 2>/dev/null || true
    wait "$BACKEND_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT

wait_for_url() {
  local url="$1"
  python3 - "$url" <<'PY'
import sys
import time
import urllib.request

url = sys.argv[1]
for _ in range(60):
    try:
        with urllib.request.urlopen(url, timeout=1) as response:
            if response.status == 200:
                raise SystemExit(0)
    except Exception:
        time.sleep(0.5)

print(f"Timed out waiting for {url}", file=sys.stderr)
raise SystemExit(1)
PY
}

ensure_port_is_free() {
  python3 - <<'PY'
import socket
import sys

sock = socket.socket()
sock.settimeout(0.5)
try:
    sock.connect(("127.0.0.1", 3001))
except OSError:
    raise SystemExit(0)
else:
    print("Port 3001 is already in use. Stop the existing backend before running this script.", file=sys.stderr)
    raise SystemExit(1)
finally:
    sock.close()
PY
}

fetch_json() {
  local url="$1"
  python3 - "$url" <<'PY'
import sys
import urllib.request

url = sys.argv[1]
with urllib.request.urlopen(url, timeout=5) as response:
    print(response.read().decode("utf-8"))
PY
}

run_local_job() {
  local collaborator_flag=()
  local project="$1"
  local collaborator="${2:-}"
  local output_log="$3"
  local python_job="$4"

  if [[ -n "$collaborator" ]]; then
    collaborator_flag=(--collaborator "$collaborator")
  fi

  cargo run --manifest-path "$ROOT_DIR/ecocode/Cargo.toml" -- \
    --project "$project" \
    --output local \
    --interval 1 \
    "${collaborator_flag[@]}" \
    -- \
    python3 -c "$python_job" >"$output_log" 2>&1
}

ensure_port_is_free

echo "==> Starting backend with test database: $DB_PATH"
cargo run --manifest-path "$ROOT_DIR/web/backend/Cargo.toml" >"$BACKEND_LOG" 2>&1 &
BACKEND_PID=$!
wait_for_url "http://127.0.0.1:3001/api/hello"

echo "==> Starting first run with collaborator Alice"
LONG_JOB=$'import math\nimport time\nvalue = 0\nfor i in range(2_000_000):\n    value += math.sqrt(i)\ntime.sleep(4)\nprint(value)\n'
run_local_job "admin-demo" "Alice" "$RUN1_LOG" "$LONG_JOB" &
RUN1_PID=$!
sleep 2

ADMIN_JSON="$(fetch_json "http://127.0.0.1:3001/api/admin/projects")"
python3 - "$ADMIN_JSON" <<'PY'
import json
import sys

payload = json.loads(sys.argv[1])
project = next((item for item in payload if item["name"] == "admin-demo"), None)
assert project is not None, payload
assert project["default_collaborator"] == "Alice", project
assert any(
    run["collaborator_name"] == "Alice" and run["status"] == "running"
    for run in project["active_runs"]
), project
print("Verified active Alice run in /api/admin/projects")
PY
wait "$RUN1_PID"

echo "==> Running the same project without --collaborator (should reuse Alice)"
SHORT_JOB=$'import time\nx = sum(i * i for i in range(400_000))\ntime.sleep(2)\nprint(x)\n'
run_local_job "admin-demo" "" "$RUN2_LOG" "$SHORT_JOB"

echo "==> Running again with collaborator Bob (should update project default)"
run_local_job "admin-demo" "Bob" "$RUN3_LOG" "$SHORT_JOB"

echo "==> Starting two concurrent runs on the same project"
run_local_job "parallel-demo" "Charlie" "$RUN4_LOG" "$LONG_JOB" &
RUN4_PID=$!
sleep 1
run_local_job "parallel-demo" "Dana" "$RUN5_LOG" "$LONG_JOB" &
RUN5_PID=$!
sleep 2

ADMIN_JSON="$(fetch_json "http://127.0.0.1:3001/api/admin/projects")"
python3 - "$ADMIN_JSON" <<'PY'
import json
import sys

payload = json.loads(sys.argv[1])
project = next((item for item in payload if item["name"] == "parallel-demo"), None)
assert project is not None, payload
assert len(project["active_runs"]) >= 2, project
names = {run["collaborator_name"] for run in project["active_runs"]}
assert "Charlie" in names and "Dana" in names, project
assert set(project["collaborators"]) == {"Charlie", "Dana"}, project
print("Verified multiple active collaborators in /api/admin/projects")
PY
wait "$RUN4_PID"
wait "$RUN5_PID"

echo "==> Validating persisted collaborator and run lifecycle data"
python3 - "$DB_PATH" <<'PY'
import sqlite3
import sys

db_path = sys.argv[1]
conn = sqlite3.connect(db_path)
conn.row_factory = sqlite3.Row

admin_project = conn.execute(
    "SELECT id, collaborator_name FROM projects WHERE name = ?",
    ("admin-demo",),
).fetchone()
assert admin_project is not None
assert admin_project["collaborator_name"] is None, dict(admin_project)

admin_runs = conn.execute(
    "SELECT collaborator_name, status, started_at, ended_at FROM runs WHERE project_id = ? ORDER BY id ASC",
    (admin_project["id"],),
).fetchall()
assert len(admin_runs) == 3, len(admin_runs)
assert [row["collaborator_name"] for row in admin_runs] == ["Alice", "Alice", "Bob"], admin_runs
assert all(row["status"] == "finished" for row in admin_runs), admin_runs
assert all(row["started_at"] and row["ended_at"] for row in admin_runs), admin_runs

admin_collaborators = conn.execute(
    "SELECT collaborator_name FROM project_collaborators WHERE project_id = ? ORDER BY collaborator_name ASC",
    (admin_project["id"],),
).fetchall()
assert [row["collaborator_name"] for row in admin_collaborators] == ["Alice", "Bob"], admin_collaborators

parallel_project = conn.execute(
    "SELECT id, collaborator_name FROM projects WHERE name = ?",
    ("parallel-demo",),
).fetchone()
assert parallel_project is not None
assert parallel_project["collaborator_name"] is None, dict(parallel_project)

parallel_runs = conn.execute(
    "SELECT collaborator_name, status FROM runs WHERE project_id = ? ORDER BY id ASC",
    (parallel_project["id"],),
).fetchall()
assert len(parallel_runs) == 2, len(parallel_runs)
assert {row["collaborator_name"] for row in parallel_runs} == {"Charlie", "Dana"}, parallel_runs
assert all(row["status"] == "finished" for row in parallel_runs), parallel_runs

parallel_collaborators = conn.execute(
    "SELECT collaborator_name FROM project_collaborators WHERE project_id = ? ORDER BY collaborator_name ASC",
    (parallel_project["id"],),
).fetchall()
assert [row["collaborator_name"] for row in parallel_collaborators] == ["Charlie", "Dana"], parallel_collaborators

summary_count = conn.execute("SELECT COUNT(*) FROM runsummary").fetchone()[0]
assert summary_count >= 5, summary_count

print("SQLite validation passed.")
PY

echo
echo "Smoke test passed."
echo "Backend log: $BACKEND_LOG"
echo "Run logs:"
echo "  $RUN1_LOG"
echo "  $RUN2_LOG"
echo "  $RUN3_LOG"
echo "  $RUN4_LOG"
echo "  $RUN5_LOG"
echo
echo "If you want to inspect the admin UI manually after this script:"
echo "  1. export ECOCODE_DB_URL=\"$ECOCODE_DB_URL\""
echo "  2. cargo run --manifest-path \"$ROOT_DIR/web/backend/Cargo.toml\""
echo "  3. NEXT_PUBLIC_API_URL=http://127.0.0.1:3001 pnpm --dir \"$ROOT_DIR/web/frontend\" dev"
echo "  4. Open http://127.0.0.1:3000/admin"
