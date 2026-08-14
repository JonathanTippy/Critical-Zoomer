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

: >"$LOG"
exec > >(tee -a "$LOG") 2>&1
plan
date -Iseconds
echo

fail() {
  echo "GROOVE CHECK FAIL: $*"
  date -Iseconds >"$STAMP_FAIL"
  echo "$*" >>"$STAMP_FAIL"
  echo "GROOVE CHECK FAIL: $*" >/tmp/cz_groove_check_last_fail_excerpt
  tail -c 8000 "$LOG" >>/tmp/cz_groove_check_last_fail_excerpt 2>/dev/null || true
  exit 1
}

run() {
  echo "======== $* ========"
  taskset -c "${START}-${END}" nice -n 10 -- "$@"
}

export CARGO_TARGET_DIR="${CZ_GROOVE_TARGET_DIR:-/tmp/cz_groove_cargo_target}"
mkdir -p "$CARGO_TARGET_DIR"
echo "zoomer_groove_check: CARGO_TARGET_DIR=$CARGO_TARGET_DIR"

"$ROOT/.cursor/hooks/kill-test-zombies.sh" >>"$LOG" 2>&1 || true

if ! command -v tracey >/dev/null 2>&1; then
  fail "tracey check: binary not on PATH (see docs/assistant/tracey.md)"
fi

# 1. tracey check
run tracey query validate || fail "tracey check (tracey query validate)"

# 2. cargo check
run cargo check --lib || fail "cargo check --lib"

# 3. cargo test (unit only)
unit_ok=0
for unit_try in 1 2 3; do
  echo "======== cargo test unit only (try ${unit_try}/3) ========"
  if taskset -c "${START}-${END}" nice -n 10 -- \
    cargo test --lib -- --skip integration_tier --skip e2e_tier
  then
    unit_ok=1
    break
  fi
  find "$CARGO_TARGET_DIR" -maxdepth 4 -type d -name 'rustc*' -prune -exec rm -rf {} + 2>/dev/null || true
  sleep 8
done
[[ "$unit_ok" -eq 1 ]] || fail "cargo test unit only"

# 4. cargo test (integration only)
run cargo test --lib integration_tier || fail "cargo test integration only"

# 5. cargo test (e2e only)
run cargo test --lib e2e_tier || fail "cargo test e2e only"

# 6. manual screenshot (CZ requires image inspect; see manual-testing.md)
if [[ "$NO_SCREENSHOT" -eq 0 ]]; then
  echo "======== manual screenshot (release + screenshot_check) ========"
  taskset -c "${START}-${END}" nice -n 15 -- cargo build --release \
    || fail "cargo build --release (screenshot)"
  rm -rf "$SCREENSHOT_OUT"
  CZ_CPUSET="${START}-${END}" taskset -c "${START}-${END}" nice -n 15 \
    "$ROOT/scripts/screenshot_check.sh" "$SCREENSHOT_OUT" \
    || fail "manual screenshot (screenshot_check.sh)"
  [[ -f "$SCREENSHOT_PATH" ]] || fail "manual screenshot missing: $SCREENSHOT_PATH"
  echo "GROOVE_SCREENSHOT_PATH=$SCREENSHOT_PATH"
fi

"$ROOT/.cursor/hooks/kill-test-zombies.sh" >>"$LOG" 2>&1 || true
date -Iseconds >"$STAMP_OK"
rm -f "$STAMP_FAIL"
echo "GROOVE CHECK OK"
exit 0
