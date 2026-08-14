#!/usr/bin/env bash
# Full check: cargo check, tests in timeout-pyramid order (unit →
# integration → e2e), all Criterion benches, fail-closed Tracey.
# Usage: taskset -c 3-8 scripts/full_check.sh
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
  echo "  0. zoomer_groove_check.sh --no-screenshot (tracey check → unit → int → e2e)"
  echo "  1. cargo test --test pipeline_cadence   (OG + GPU, 60s)"
  echo "  2. cargo bench workgroup_fitness shadergroup_fitness my_bench"
  echo "  3. tracey query validate + status (fail-closed after benches)"
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
  # `--` so cargo's own `--` / `--test-threads` is never eaten by taskset/nice
  # (that produced `test-threads=1: command not found`).
  taskset -c "${START}-${END}" nice -n 10 -- "$@"
}

fail() {
  echo "FULL CHECK FAIL: $*"
  date -Iseconds >"$STAMP_FAIL"
  echo "$*" >>"$STAMP_FAIL"
  echo "FULL CHECK FAIL: $*" >/tmp/cz_full_check_last_fail_excerpt
  sleep 0.2
  tail -c 8000 "$LOG" >>/tmp/cz_full_check_last_fail_excerpt 2>/dev/null || true
  exit 1
}

# Never share workspace `target/` with rust-analyzer or agent cargo.
# Concurrent rustc there dies in lld (empty `= note:`, `rustcXXXX` scratch).
export CARGO_TARGET_DIR="${CZ_FULL_CHECK_TARGET_DIR:-/tmp/cz_full_check_cargo_target}"
mkdir -p "$CARGO_TARGET_DIR"
echo "full_check: CARGO_TARGET_DIR=$CARGO_TARGET_DIR"

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

if [[ "${CZ_FULL_CHECK_SKIP_GROOVE:-0}" == 1 ]]; then
  echo "full_check: skipping groove tier (fresh zoomer_groove_check_on_stop)"
else
  CZ_GROOVE_TARGET_DIR="$CARGO_TARGET_DIR" \
    "$ROOT/scripts/zoomer_groove_check.sh" --no-screenshot \
    || fail "zoomer_groove_check (no screenshot)"
fi
# Same floors; one retry after leftover reap (overlapping cargo/GPU load).
cadence_ok=0
for cadence_try in 1 2; do
  echo "======== cargo test --test pipeline_cadence (try ${cadence_try}/2) ========"
  if taskset -c "${START}-${END}" nice -n 10 -- \
    env RUST_TEST_THREADS=1 cargo test --test pipeline_cadence -- --test-threads=1
  then
    cadence_ok=1
    break
  fi
  echo "full_check: cadence try ${cadence_try} missed floors; reap and retry"
  .cursor/hooks/kill-test-zombies.sh >>"$LOG" 2>&1 || true
  sleep 2
done
[[ "$cadence_ok" -eq 1 ]] || fail "cargo test pipeline_cadence"
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
