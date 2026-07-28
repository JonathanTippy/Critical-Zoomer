#!/usr/bin/env bash
# Region coverage for QC. Ignores GUI/actor shells (headed e2e covers those).
# Skips instrumented-slow home fill under llvm-cov.
# Usage: taskset -c 4-11 scripts/coverage.sh
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
IGNORE='(main\.rs|settings\.rs|window/mod\.rs|widgetize\.rs)'
taskset -c "${CZ_CPUSET:-4-11}" cargo llvm-cov --lib --release --summary-only \
  --ignore-filename-regex="$IGNORE" \
  -- \
  --test-threads=1 \
  --skip home_800x480_fills_within_five_seconds_cpu
