#!/usr/bin/env bash
# Isolated Xvfb screenshot check (the only product script in scripts/).
# Policy: scripts/README.md — do not add more scripts for tests or benches.
#
# Usage:
#   taskset -c 4-11 nice -n 15 cargo build --release
#   CZ_CPUSET=4-11 taskset -c 4-11 nice -n 15 \
#     scripts/xvfb_screenshot_check.sh /tmp/cz_screenshot_check
#
# Writes PNGs under the out dir (default /tmp). Never commit those PNGs.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="${1:-/tmp/cz_screenshot_check}"
HOLD_SECS="${HOLD_SECS:-12}"
"$ROOT/scripts/cz_ctl.sh" stop 2>/dev/null || true
rm -rf "$OUT"
mkdir -p "$OUT"
taskset -c "${CZ_CPUSET:-4-11}" xvfb-run -a -s "-screen 0 900x500x24" \
  "$ROOT/scripts/cz_ctl.sh" start "$OUT" &
CTL_PID=$!
sleep 6
"$ROOT/scripts/cz_ctl.sh" send "settle home.png ${HOLD_SECS} 3000" || true
"$ROOT/scripts/cz_ctl.sh" send "capture home_final.png"
"$ROOT/scripts/cz_ctl.sh" send quit
wait "$CTL_PID" 2>/dev/null || true
identify -format '%f stdev=%[standard-deviation] mean=%[mean]\n' "$OUT"/*.png 2>/dev/null || ls -la "$OUT"
MEAN=$(identify -format '%[mean]' "$OUT/home_final.png" 2>/dev/null | awk '{print int($1)}')
if [ -z "$MEAN" ] || [ "$MEAN" -lt 2000 ]; then
  echo "screenshot check rejected: mean luminance ${MEAN:-missing} (want >= 2000)" >&2
  exit 1
fi
echo "screenshot check ok mean=$MEAN out=$OUT"
echo "Assistant: Read $OUT/home_final.png and inspect the image directly."
