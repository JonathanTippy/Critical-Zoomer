#!/usr/bin/env bash
# Region coverage for QC. Ignores GUI/actor shells (headed e2e covers those).
# Skips instrumented-slow home fill under llvm-cov.
# Usage: taskset -c 4-11 scripts/coverage.sh
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
IGNORE='(main\.rs|do_work\.rs|settings\.rs|window/mod\.rs|widgetize\.rs|shade\.rs|escaper\.rs|colorer/mod\.rs|shadergroup/structs\.rs|screen_worker/|work_collector\.rs|work_controller\.rs|naive_gpu_worker\.rs)'
taskset -c "${CZ_CPUSET:-4-11}" cargo llvm-cov --lib --release --summary-only \
  --ignore-filename-regex="$IGNORE" \
  -- \
  --test-threads=1 \
  --skip home_800x480_fills_within_five_seconds_cpu
