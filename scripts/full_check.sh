#!/usr/bin/env bash
# Full check: cargo check, tests in timeout-pyramid order (unit →
# integration → e2e), all Criterion benches, fail-closed Tracey.
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

plan() {
  echo "full_check: pin ${START}-${END}  log $LOG"
  echo "  1. cargo check --lib"
  echo "  2. cargo test --lib  (unit; skip integration_tier, e2e_tier; debug+opt-3)"
  echo "  3. cargo test --lib integration_tier   (≤10, 15s)"
  echo "  4. cargo test --lib e2e_tier            (park, 60s)"
  echo "  5. cargo test --test pipeline_cadence   (OG + GPU, 60s)"
  echo "  6. cargo bench workgroup_fitness shadergroup_fitness my_bench"
  echo "  7. tracey query validate      (fail-closed)"
  echo "  8. tracey query status        (dump; does not fail the check)"
}

if [[ "$DRY" -eq 1 ]]; then
  plan
  exit 0
fi

# One full_check at a time. Overlapping stop-hooks used to SIGTERM the
# other's Criterion `workgroup_fitness` (CLI reaper + cargo lock fights).
LOCK="${CZ_FULL_CHECK_LOCK:-/tmp/cz_full_check.lock}"
exec 9>"$LOCK"
flock 9

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

# Reap a wgpu lockdir left by a killed harness (Drop never ran). Never steal
# from a live pid — that waiter must wait, not wipe a running cadence graph.
WGPU_LOCKDIR=/tmp/cz_wgpu_test.lockdir
if [[ -d "$WGPU_LOCKDIR" ]]; then
  wpid="$(cat "$WGPU_LOCKDIR/pid" 2>/dev/null || true)"
  if [[ -z "${wpid}" || ! -d "/proc/${wpid}" ]]; then
    rm -rf "$WGPU_LOCKDIR"
    echo "full_check: reaped stale $WGPU_LOCKDIR (pid='${wpid}')"
  else
    echo "full_check: wgpu lock held by live pid ${wpid} — tests will wait"
  fi
fi

run cargo check --lib || fail "cargo check --lib"
# Unit first: cheap 1s tests. Fail here before paying for 15s/60s suites.
run cargo test --lib -- \
  --skip integration_tier --skip e2e_tier \
  || fail "cargo test unit (--lib, skip integration/e2e)"
run cargo test --lib integration_tier \
  || fail "cargo test integration_tier"
run cargo test --lib e2e_tier \
  || fail "cargo test e2e_tier"
run cargo test --test pipeline_cadence -- --test-threads=1 \
  || fail "cargo test pipeline_cadence"
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
