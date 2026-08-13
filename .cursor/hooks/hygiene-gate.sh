#!/usr/bin/env bash
# One-shot hygiene gate: bacon-equivalent check, full test suite, all
# Criterion benches, fail-closed Tracey validate + status dump.
# Log: /tmp/cz_hygiene.log  (never in the repo)
#
# Usage:
#   .cursor/hooks/hygiene-gate.sh           # run the gate
#   .cursor/hooks/hygiene-gate.sh --dry-run # print plan, exit 0
# Env:
#   CZ_HYGIENE_RELEASE=1  → cargo test --release --all-targets
set -u
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT" || exit 1
LOG="${CZ_HYGIENE_LOG:-/tmp/cz_hygiene.log}"
STAMP_OK="${CZ_HYGIENE_STAMP_OK:-/tmp/cz_hygiene_last_ok}"
STAMP_FAIL="${CZ_HYGIENE_STAMP_FAIL:-/tmp/cz_hygiene_last_fail}"
DRY=0
[[ "${1:-}" == "--dry-run" ]] && DRY=1

nproc_now="$(nproc 2>/dev/null || echo 4)"
START=$((nproc_now / 4))
END=$((nproc_now * 3 / 4 - 1))
[[ "$END" -lt "$START" ]] && END="$START"
PIN=(taskset -c "${START}-${END}" nice -n 10)

TEST_FLAGS=(--all-targets)
[[ "${CZ_HYGIENE_RELEASE:-0}" == "1" ]] && TEST_FLAGS=(--release --all-targets)

plan() {
  echo "hygiene-gate: pin ${START}-${END}  log $LOG"
  echo "  1. cargo check --lib          (bacon jobs.check)"
  echo "  2. cargo test ${TEST_FLAGS[*]}"
  echo "  3. cargo bench workgroup_fitness shadergroup_fitness my_bench"
  echo "  4. tracey query validate      (fail-closed; no soft-skip)"
  echo "  5. tracey query status        (feedback dump; does not fail the gate)"
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
  echo "HYGIENE FAIL: $*"
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
echo "HYGIENE OK"
exit 0
