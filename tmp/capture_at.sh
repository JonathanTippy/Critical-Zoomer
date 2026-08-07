#!/usr/bin/env bash
set -euo pipefail
ROOT="${1:-/home/jonathan/git/Critical-Zoomer}"
LABEL="${2:-head}"
CZ_BIN_OVERRIDE="${3:-}"
E2E_PREFIX="$ROOT/tmp/capture_$LABEL"
rm -rf "$E2E_PREFIX"
mkdir -p "$E2E_PREFIX/capture"
export CZ_SESSION_PREFIX="$E2E_PREFIX"
if [ -n "$CZ_BIN_OVERRIDE" ]; then export CZ_BIN="$CZ_BIN_OVERRIDE"; fi
cleanup() {
  CZ_SESSION_PREFIX="$E2E_PREFIX" "$ROOT/scripts/cz_ctl.sh" stop 2>/dev/null || true
  sleep 0.3
  CZ_SESSION_PREFIX="$E2E_PREFIX" "$ROOT/scripts/cz_ctl.sh" stop 2>/dev/null || true
  # Belt-and-braces: kill any leftover app/Xvfb from this session so repeated
  # captures cannot leave zombie processes behind.
  pkill -f "critical_zoomer" 2>/dev/null || true
  pkill -f "Xvfb" 2>/dev/null || true
  pkill -f "xvfb-run" 2>/dev/null || true
}
trap cleanup EXIT
taskset -c 4-11 xvfb-run -a -s "-screen 0 900x500x24" \
  "$ROOT/scripts/cz_ctl.sh" start "$E2E_PREFIX/capture" >"$E2E_PREFIX/wrapper.log" 2>&1 &
echo $! >"$E2E_PREFIX/xvfb_wrapper.pid"
for i in $(seq 1 80); do
  [ -p "$E2E_PREFIX/ctl.fifo" ] && [ -f "$E2E_PREFIX/ctl.env" ] && break
  sleep 0.25
done
# shellcheck source=/dev/null
source "$E2E_PREFIX/ctl.env"
for i in $(seq 1 60); do
  xwininfo -root -tree 2>/dev/null | rg -q 'Critical Zoomer' && break
  sleep 0.5
done
CZ_SESSION_PREFIX="$E2E_PREFIX" "$ROOT/scripts/cz_ctl.sh" send home
OUT="$E2E_PREFIX/capture/home.png"
# Allow full home fill (not just first structured partial frame).
sleep 25
for i in $(seq 1 120); do
  CZ_SESSION_PREFIX="$E2E_PREFIX" "$ROOT/scripts/cz_ctl.sh" send "capture probe_$i.png"
  for j in $(seq 1 40); do
    [ -s "$E2E_PREFIX/capture/probe_$i.png" ] && break
    sleep 0.25
  done
  stdev=$(identify -format '%[standard-deviation]' "$E2E_PREFIX/capture/probe_$i.png" 2>/dev/null | awk '{print int($1+0)}')
  echo "$LABEL attempt $i stdev=$stdev"
  if [ -n "${stdev:-}" ] && [ "$stdev" -ge 3000 ]; then
    cp -f "$E2E_PREFIX/capture/probe_$i.png" "$OUT"
    break
  fi
  sleep 0.5
done
CZ_SESSION_PREFIX="$E2E_PREFIX" "$ROOT/scripts/cz_ctl.sh" send quit || true
python3 - "$OUT" "$LABEL" <<'PY'
import sys
from PIL import Image
import numpy as np
p, label = sys.argv[1], sys.argv[2]
img = np.array(Image.open(p).convert('L'))
h, w = img.shape
black_cols = sum(1 for x in range(w) if (img[:, x] < 30).mean() > 0.8)
print(f"{label}: mean={img.mean():.1f} black%={(img<30).mean()*100:.1f} black_cols_80={black_cols}")
PY
