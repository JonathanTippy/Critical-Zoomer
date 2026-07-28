#!/usr/bin/env bash
# Short libFuzzer smoke for QC / local CI. Requires nightly + cargo-fuzz.
# Usage: taskset -c 4-11 scripts/fuzz_smoke.sh [seconds]
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
SECS="${1:-45}"
taskset -c "${CZ_CPUSET:-4-11}" cargo +nightly fuzz run fuzz_target_1 -- \
  -max_total_time="$SECS" -max_len=64
