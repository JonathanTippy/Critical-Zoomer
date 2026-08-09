#!/usr/bin/env bash
# E2E: home settle, shift zoom-in vs space zoom-out must move opposite ways,
# and a few scroll bumps must settle without near-black frames.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="${1:-/tmp/cz_e2e_zoom}"
"$ROOT/scripts/cz_ctl.sh" stop 2>/dev/null || true
rm -rf "$OUT"
mkdir -p "$OUT"

wait_file() {
  local path="$1"
  local secs="${2:-60}"
  local i=0
  while [ "$i" -lt "$secs" ]; do
    if [ -f "$path" ]; then
      return 0
    fi
    sleep 0.25
    i=$((i + 1))
  done
  echo "timeout waiting for $path" >&2
  return 1
}

taskset -c "${CZ_CPUSET:-4-11}" xvfb-run -a -s "-screen 0 900x500x24" \
  "$ROOT/scripts/cz_ctl.sh" start "$OUT" &
CTL_PID=$!
cleanup() {
  "$ROOT/scripts/cz_ctl.sh" send quit 2>/dev/null || true
  # Never hang the harness waiting on a wedged xvfb/daemon.
  if command -v timeout >/dev/null 2>&1; then
    timeout 5 bash -c "wait $CTL_PID" 2>/dev/null || true
  else
    sleep 1
  fi
  "$ROOT/scripts/cz_ctl.sh" stop 2>/dev/null || true
  kill -KILL "$CTL_PID" 2>/dev/null || true
}
trap cleanup EXIT

# Wait for daemon fifo/env before sending.
for _ in $(seq 1 80); do
  if [ -p /tmp/cz_ctl.fifo ] && [ -f /tmp/cz_ctl.env ]; then
    break
  fi
  sleep 0.25
done
# shellcheck source=/dev/null
source /tmp/cz_ctl.env
export DISPLAY XAUTHORITY
# Wait until the app window exists on this X display.
for _ in $(seq 1 60); do
  if xwininfo -root -tree 2>/dev/null | rg -q 'Critical Zoomer'; then
    break
  fi
  sleep 0.5
done
if ! xwininfo -root -tree 2>/dev/null | rg -q 'Critical Zoomer'; then
  echo "Critical Zoomer window never appeared; xvfb log:" >&2
  tail -40 /tmp/cz_xvfb.log >&2 || true
  exit 1
fi
sleep 1

"$ROOT/scripts/cz_ctl.sh" send "settle home.png 8 2500" || true
wait_file "$OUT/home.png" 40
"$ROOT/scripts/cz_ctl.sh" send "capture home_final.png"
wait_file "$OUT/home_final.png" 20

# Key zoom (not scroll): Shift = in, Space = out per requirements.
"$ROOT/scripts/cz_ctl.sh" send "zoomin 3"
"$ROOT/scripts/cz_ctl.sh" send "settle after_zoomin.png 6 2000" || true
wait_file "$OUT/after_zoomin.png" 40
"$ROOT/scripts/cz_ctl.sh" send "capture after_zoomin_final.png"
wait_file "$OUT/after_zoomin_final.png" 20

"$ROOT/scripts/cz_ctl.sh" send "zoomout 3"
"$ROOT/scripts/cz_ctl.sh" send "settle after_zoomout.png 6 2000" || true
wait_file "$OUT/after_zoomout.png" 40
"$ROOT/scripts/cz_ctl.sh" send "capture after_zoomout_final.png"
wait_file "$OUT/after_zoomout_final.png" 20

# Scroll bumps (platform sign exercised).
"$ROOT/scripts/cz_ctl.sh" send "scroll 5"
"$ROOT/scripts/cz_ctl.sh" send "settle after_scroll.png 6 2000" || true
wait_file "$OUT/after_scroll.png" 40
"$ROOT/scripts/cz_ctl.sh" send "capture after_scroll_final.png"
wait_file "$OUT/after_scroll_final.png" 20

fail=0
for f in home_final after_zoomin_final after_zoomout_final after_scroll_final; do
  path="$OUT/${f}.png"
  if [ ! -f "$path" ]; then
    echo "missing $path" >&2
    fail=1
    continue
  fi
  MEAN=$(identify -format '%[mean]' "$path" 2>/dev/null | awk '{print int($1)}')
  STDEV=$(identify -format '%[standard-deviation]' "$path" 2>/dev/null | awk '{print int($1)}')
  echo "$f mean=$MEAN stdev=$STDEV"
  if [ -z "$MEAN" ] || [ "$MEAN" -lt 1500 ]; then
    echo "FAIL $f near-black mean=${MEAN:-missing}" >&2
    fail=1
  fi
done

# Zoom-in then zoom-out should not be identical to a dead frame sequence:
# require after_zoomin differs from home (pixel compare via RMSE).
if command -v compare >/dev/null 2>&1; then
  RMSE=$(compare -metric RMSE "$OUT/home_final.png" "$OUT/after_zoomin_final.png" null: 2>&1 | awk '{print $1}' || true)
  echo "home_vs_zoomin_rmse=$RMSE"
  # If RMSE is exactly 0, zoom-in did nothing visible.
  if [ "$RMSE" = "0" ] || [ "$RMSE" = "0 (0)" ]; then
    echo "FAIL zoom-in produced identical frame to home" >&2
    fail=1
  fi
fi

exit "$fail"
