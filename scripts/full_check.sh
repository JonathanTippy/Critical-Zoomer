#!/usr/bin/env bash
# Full check: cargo check, full test suite, all Criterion benches,
# fail-closed Tracey validate + status dump.
# Usage: taskset -c 4-11 scripts/full_check.sh
#        scripts/full_check.sh --dry-run
# Log: /tmp/cz_full_check.log  (never in the repo)
set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT" || exit 1
LOG="${CZ_FULL_CHECK_LOG:-/tmp/cz_full_check.log}"
STAMP_OK="${CZ_FULL_CHECK_STAMP_OK:-/tmp/cz_full_check_last_ok}"
STAMP_FAIL="${CZ_FULL_CHECK_STAMP_FAIL:-/tmp/cz_full_check_last_fail}"
DRY=0
[[ "${1:-}" == "--dry-run" ]] && DRY=1

nproc_now="$(nproc 2>/dev/null || echo 4)"
START=$((nproc_now / 4))
END=$((nproc_now * 3 / 4 - 1))
[[ "$END" -lt "$START" ]] && END="$START"
PIN=(taskset -c "${START}-${END}" nice -n 10)

TEST_FLAGS=(--release --all-targets)

plan() {
  echo "full_check: pin ${START}-${END}  log $LOG"
  echo "  1. cargo check --lib"
  echo "  2. cargo test --release --all-targets"
  echo "  3. cargo bench workgroup_fitness shadergroup_fitness my_bench"
  echo "  4. tracey query validate      (fail-closed)"
  echo "  5. tracey query status        (dump; does not fail the check)"
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

run() {
  echo "======== $* ========"
  "${PIN[@]}" "$@"
}

fail() {
  echo "FULL CHECK FAIL: $*"
  date -Iseconds >"$STAMP_FAIL"
  echo "$*" >>"$STAMP_FAIL"
  exit 1
}

.cursor/hooks/kill-test-zombies.sh >>"$LOG" 2>&1 || true

run cargo check --lib || fail "cargo check --lib"
run cargo test "${TEST_FLAGS[@]}" || fail "cargo test ${TEST_FLAGS[*]}"
run cargo bench --bench workgroup_fitness --bench shadergroup_fitness --bench my_bench \
  || fail "cargo bench (workgroup_fitness + shadergroup_fitness + my_bench)"
echo "Criterion ran. Compare medians to docs/assistant/benchmarks.md (~20% FIX NOW)."

if ! command -v tracey >/dev/null 2>&1; then
  fail "tracey binary not on PATH (fail-closed; see docs/assistant/tracey.md)"
fi
run tracey query validate || fail "tracey query validate"
echo "======== tracey query status (coverage debt, not a fail) ========"
tracey query status || echo "tracey query status exited $?"

.cursor/hooks/kill-test-zombies.sh >>"$LOG" 2>&1 || true
date -Iseconds >"$STAMP_OK"
rm -f "$STAMP_FAIL"
echo "FULL CHECK OK"
exit 0
