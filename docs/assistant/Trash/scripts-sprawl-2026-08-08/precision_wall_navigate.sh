#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="${1:-/tmp/cz_wall_nav}"
SEAHORSE_RE="-0.743643887037151"
SEAHORSE_IM="0.131825904205216"
ZOOM_POT=17
HOLD_SECS=25
export CZ_NAV=1
export CZ_GOTO="$SEAHORSE_RE $SEAHORSE_IM $ZOOM_POT"
"$ROOT/scripts/cz_ctl.sh" stop 2>/dev/null || true
rm -rf "$OUT"
mkdir -p "$OUT"
taskset -c "${CZ_CPUSET:-4-11}" xvfb-run -a -s "-screen 0 900x500x24" \
  "$ROOT/scripts/cz_ctl.sh" start "$OUT" &
CTL_PID=$!
sleep 20
"$ROOT/scripts/cz_ctl.sh" send "capture wall_nav_t0.png"
sleep "$HOLD_SECS"
"$ROOT/scripts/cz_ctl.sh" send "capture wall_nav_t${HOLD_SECS}.png"
"$ROOT/scripts/cz_ctl.sh" send quit
wait "$CTL_PID" 2>/dev/null || true
identify -format '%f stdev=%[standard-deviation]\n' "$OUT"/wall_nav_*.png
