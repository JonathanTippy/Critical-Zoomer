#!/usr/bin/env bash
# Region coverage for QC. Ignores GUI/actor shells (headed e2e covers those).
# Skips instrumented-slow home fill under llvm-cov.
# Usage: taskset -c 4-11 scripts/coverage.sh
# Writes summary to docs/assistant-docs/coverage-baseline.txt and HTML under target/llvm-cov/html
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
NPROC="$(nproc)"
START=$((NPROC / 4))
END=$((NPROC * 3 / 4 - 1))
CZ_CPUSET="${CZ_CPUSET:-${START}-${END}}"
IGNORE='(main\.rs|settings\.rs|window/mod\.rs|widgetize\.rs)'
OUT_DIR="${ROOT}/target/llvm-cov/html"
mkdir -p "$(dirname "$OUT_DIR")"
SUMMARY="${ROOT}/docs/assistant-docs/coverage-baseline.txt"

# Debug profile: release/LTO often yields empty (0%) region counters.
# Dedicated target-dir avoids fighting a live mutants campaign on mutants.out/.
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-${ROOT}/target/llvm-cov-target}"
echo "coverage: cpuset=${CZ_CPUSET} target=${CARGO_TARGET_DIR}" | tee "$SUMMARY"
taskset -c "${CZ_CPUSET}" cargo llvm-cov --lib \
  --html --output-dir "$OUT_DIR" \
  --ignore-filename-regex="$IGNORE" \
  --summary-only \
  -- \
  --test-threads=1 \
  --skip home_800x480_fills_within_five_seconds_cpu \
  --skip home_800x480 \
  --skip gpu_ips \
  --skip foveation_balance \
  --skip standards_perf:: \
  --skip cpu_ips_ \
  2>&1 | tee -a "$SUMMARY"

echo "HTML: ${OUT_DIR}/index.html" | tee -a "$SUMMARY"
