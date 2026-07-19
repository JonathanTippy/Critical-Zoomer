#!/usr/bin/env bash
# Capture a naive-path visual baseline at home (pixel-judged settle).
# Usage:
#   taskset -c 4-11 xvfb-run -a -s "-screen 0 900x500x24" scripts/capture_naive_baseline.sh [out_dir]
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="${1:-/tmp/cz_naive_baseline}"
HOLD_SECS="${HOLD_SECS:-12}"
"$ROOT/scripts/cz_ctl.sh" stop 2>/dev/null || true
rm -rf "$OUT"
mkdir -p "$OUT"
taskset -c "${CZ_CPUSET:-4-11}" xvfb-run -a -s "-screen 0 900x500x24" \
  "$ROOT/scripts/cz_ctl.sh" start "$OUT" &
CTL_PID=$!
sleep 6
"$ROOT/scripts/cz_ctl.sh" send "settle baseline_home.png ${HOLD_SECS} 3000" || true
"$ROOT/scripts/cz_ctl.sh" send "capture baseline_home_final.png"
"$ROOT/scripts/cz_ctl.sh" send quit
wait "$CTL_PID" 2>/dev/null || true
identify -format '%f stdev=%[standard-deviation] mean=%[mean]\n' "$OUT"/*.png 2>/dev/null || ls -la "$OUT"
# Reject near-black frames (stdev alone passed on mostly-black captures).
MEAN=$(identify -format '%[mean]' "$OUT/baseline_home_final.png" 2>/dev/null | awk '{print int($1)}')
if [ -z "$MEAN" ] || [ "$MEAN" -lt 2000 ]; then
  echo "baseline rejected: mean luminance ${MEAN:-missing} (want >= 2000)" >&2
  exit 1
fi
echo "baseline ok mean=$MEAN"
