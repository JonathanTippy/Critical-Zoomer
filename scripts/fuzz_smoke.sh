#!/usr/bin/env bash
# Short libFuzzer smoke for QC / local CI. Requires nightly + cargo-fuzz.
# Usage: taskset -c 4-11 scripts/fuzz_smoke.sh [seconds_per_target]
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
SECS="${1:-15}"
NPROC="$(nproc)"
START=$((NPROC / 4))
END=$((NPROC * 3 / 4 - 1))
CZ_CPUSET="${CZ_CPUSET:-${START}-${END}}"
TARGETS=(fuzz_target_1 fuzz_range fuzz_coords_parse fuzz_publisher_clamp)
for t in "${TARGETS[@]}"; do
  echo "fuzz_smoke: ${t} (${SECS}s)"
  taskset -c "${CZ_CPUSET}" cargo +nightly fuzz run "$t" -- \
    -max_total_time="$SECS" -max_len=64
done
