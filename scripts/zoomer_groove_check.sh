#!/usr/bin/env bash
# Zoomer-groove checker — JFT_Prompts/skills/zoomer-groove/SKILL.md § Checker Script.
#
# Standard order (CZ mapping):
#   1. tracey check        → tracey query validate (no `tracey check` subcommand)
#   2. cargo check         → cargo check --lib
#   3. cargo test unit     → cargo test --lib -- --skip integration_tier --skip e2e_tier
#   4. cargo test int      → cargo test --lib integration_tier
#   5. cargo test e2e      → cargo test --lib e2e_tier
#   6. manual screenshot   → release build + scripts/screenshot_check.sh (CZ manual test)
#
# Usage: scripts/zoomer_groove_check.sh [--dry-run] [--no-screenshot]
# Stop hook: .cursor/hooks/groove_check_on_stop.sh (always includes screenshot).
set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT" || exit 1
LOG="${CZ_GROOVE_CHECK_LOG:-/tmp/cz_groove_check.log}"
STAMP_OK="${CZ_GROOVE_CHECK_STAMP_OK:-/tmp/cz_groove_check_last_ok}"
STAMP_FAIL="${CZ_GROOVE_CHECK_STAMP_FAIL:-/tmp/cz_groove_check_last_fail}"
EXCERPT="${CZ_GROOVE_CHECK_EXCERPT:-/tmp/cz_groove_check_last_fail_excerpt}"
STEP_LOG="${CZ_GROOVE_CHECK_STEP_LOG:-/tmp/cz_groove_check_step.log}"
LOCK="${CZ_GROOVE_CHECK_LOCK:-/tmp/cz_groove_check.lock}"
SCREENSHOT_OUT="${CZ_GROOVE_SCREENSHOT_OUT:-/tmp/cz_groove_screenshot}"
SCREENSHOT_PATH="${SCREENSHOT_OUT}/home_final.png"
DRY=0
NO_SCREENSHOT=0
for arg in "$@"; do
  case "$arg" in
    --dry-run) DRY=1 ;;
    --no-screenshot) NO_SCREENSHOT=1 ;;
  esac
done

nproc_now="$(nproc 2>/dev/null || echo 4)"
START=$((nproc_now / 4))
END=$((nproc_now * 3 / 4 - 1))
[[ "$END" -lt "$START" ]] && END="$START"

plan() {
  echo "zoomer_groove_check: pin ${START}-${END}  log $LOG"
  echo "  1. tracey check (tracey query validate)"
  echo "  2. cargo check --lib"
  echo "  3. cargo test unit only"
  echo "  4. cargo test integration only (integration_tier)"
  echo "  5. cargo test e2e only (e2e_tier)"
  if [[ "$NO_SCREENSHOT" -eq 0 ]]; then
    echo "  6. manual screenshot → $SCREENSHOT_PATH"
  else
    echo "  6. manual screenshot skipped (--no-screenshot)"
  fi
}

if [[ "$DRY" -eq 1 ]]; then
  plan
  exit 0
fi

exec 9>"$LOCK"
if ! flock -n 9; then
  echo "another zoomer_groove_check is already running (lock $LOCK)" >"$STEP_LOG"
  fail "another zoomer_groove_check is already running"
fi

: >"$LOG"
exec > >(tee -a "$LOG") 2>&1
plan
date -Iseconds
echo

fail() {
  local msg="$1"
  local issue
  issue="$(python3 - "$STEP_LOG" <<'PY'
import sys
path = sys.argv[1]
try:
    lines = open(path, encoding="utf-8", errors="replace").read().splitlines()
except OSError:
    print("(no step output captured)")
    raise SystemExit(0)
markers = (
    "panicked at", "assertion `", "error[E", "error: could not compile",
    "error: test failed", "FAILED.", "tracey binary not",
)
start = 0
for i, line in enumerate(lines):
    if any(m in line for m in markers):
        start = max(0, i - 2)
        break
else:
    for i, line in enumerate(lines):
        if line.strip() == "failures:" or line.startswith("failures:"):
            start = i
            break
    else:
        start = max(0, len(lines) - 40)
snippet = "\n".join(lines[start:])
if len(snippet) > 10000:
    snippet = snippet[-10000:]
print(snippet)
PY
)"
  echo "GROOVE CHECK FAIL: $msg"
  echo "$issue"
  date -Iseconds >"$STAMP_FAIL"
  {
    echo "GROOVE CHECK FAIL: $msg"
    echo ""
    echo "$issue"
  } >"$EXCERPT"
  echo "$msg" >>"$STAMP_FAIL"
  exit 1
}

# Run one step; tee stdout+stderr to LOG and STEP_LOG so fail() has the real error.
run_step() {
  local label="$1"
  shift
  echo "======== $label ========"
  : >"$STEP_LOG"
  set +e
  taskset -c "${START}-${END}" nice -n 10 -- "$@" 2>&1 | tee -a "$LOG" "$STEP_LOG"
  local ec=${PIPESTATUS[0]}
  set -e
  return "$ec"
}

export CARGO_TARGET_DIR="${CZ_GROOVE_TARGET_DIR:-/tmp/cz_groove_cargo_target}"
mkdir -p "$CARGO_TARGET_DIR"
echo "zoomer_groove_check: CARGO_TARGET_DIR=$CARGO_TARGET_DIR"

"$ROOT/.cursor/hooks/kill-test-zombies.sh" >>"$LOG" 2>&1 || true

if ! command -v tracey >/dev/null 2>&1; then
  echo "tracey binary not on PATH (see docs/assistant/tracey.md)" >"$STEP_LOG"
  fail "tracey check: binary not on PATH"
fi

# 1. tracey check
run_step "tracey check (tracey query validate)" tracey query validate \
  || fail "tracey check (tracey query validate)"

# 2. cargo check
run_step "cargo check --lib" cargo check --lib \
  || fail "cargo check --lib"

# 3. cargo test (unit only)
unit_ok=0
for unit_try in 1 2 3; do
  if run_step "cargo test unit only (try ${unit_try}/3)" \
    cargo test --lib -- --skip integration_tier --skip e2e_tier
  then
    unit_ok=1
    break
  fi
  if [[ "$unit_try" -lt 3 ]]; then
    find "$CARGO_TARGET_DIR" -maxdepth 4 -type d -name 'rustc*' -prune -exec rm -rf {} + 2>/dev/null || true
    sleep 8
  fi
done
[[ "$unit_ok" -eq 1 ]] || fail "cargo test unit only"

# 4. cargo test (integration only)
run_step "cargo test integration only" cargo test --lib integration_tier \
  || fail "cargo test integration only"

# 5. cargo test (e2e only)
run_step "cargo test e2e only" cargo test --lib e2e_tier \
  || fail "cargo test e2e only"

# 6. manual screenshot (CZ requires image inspect; see manual-testing.md)
if [[ "$NO_SCREENSHOT" -eq 0 ]]; then
  run_step "cargo build --release (screenshot)" cargo build --release \
    || fail "cargo build --release (screenshot)"
  rm -rf "$SCREENSHOT_OUT"
  run_step "manual screenshot (screenshot_check.sh)" \
    env CZ_CPUSET="${START}-${END}" "$ROOT/scripts/screenshot_check.sh" "$SCREENSHOT_OUT" \
    || fail "manual screenshot (screenshot_check.sh)"
  if [[ ! -f "$SCREENSHOT_PATH" ]]; then
    echo "expected PNG missing: $SCREENSHOT_PATH" >"$STEP_LOG"
    fail "manual screenshot missing: $SCREENSHOT_PATH"
  fi
  echo "GROOVE_SCREENSHOT_PATH=$SCREENSHOT_PATH"
fi

"$ROOT/.cursor/hooks/kill-test-zombies.sh" >>"$LOG" 2>&1 || true
date -Iseconds >"$STAMP_OK"
rm -f "$STAMP_FAIL" "$EXCERPT"
echo "GROOVE CHECK OK"
exit 0
