#!/usr/bin/env bash
set -euo pipefail
ROOT="/home/jonathan/git/Critical-Zoomer"
E2E_PREFIX="$ROOT/tmp/cz_band_probe"
rm -rf "$E2E_PREFIX"
mkdir -p "$E2E_PREFIX/capture"
export CZ_SESSION_PREFIX="$E2E_PREFIX"
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
# Wait for structured fill (same gate as e2e_visual)
for i in $(seq 1 80); do
  CZ_SESSION_PREFIX="$E2E_PREFIX" "$ROOT/scripts/cz_ctl.sh" send "capture probe_$i.png"
  for j in $(seq 1 40); do
    [ -s "$E2E_PREFIX/capture/probe_$i.png" ] && break
    sleep 0.25
  done
  stdev=$(identify -format '%[standard-deviation]' "$E2E_PREFIX/capture/probe_$i.png" 2>/dev/null | awk '{print int($1+0)}')
  echo "probe attempt $i stdev=$stdev"
  if [ -n "${stdev:-}" ] && [ "$stdev" -ge 3000 ]; then
    cp -f "$E2E_PREFIX/capture/probe_$i.png" "$E2E_PREFIX/capture/home_now.png"
    break
  fi
  sleep 0.5
done
[ -s "$E2E_PREFIX/capture/home_now.png" ] || cp -f "$E2E_PREFIX/capture/probe_80.png" "$E2E_PREFIX/capture/home_now.png" 2>/dev/null || true
for i in $(seq 1 60); do
  [ -s "$E2E_PREFIX/capture/home_now.png" ] && break
  sleep 0.25
done
CZ_SESSION_PREFIX="$E2E_PREFIX" "$ROOT/scripts/cz_ctl.sh" send quit || true
sleep 0.5
CZ_SESSION_PREFIX="$E2E_PREFIX" "$ROOT/scripts/cz_ctl.sh" stop 2>/dev/null || true
convert "$E2E_PREFIX/capture/home_now.png" -format 'mean=%[mean] stdev=%[standard-deviation]\n' info:
python3 <<'PY'
from PIL import Image
import numpy as np
p="/home/jonathan/git/Critical-Zoomer/tmp/cz_band_probe/capture/home_now.png"
img=np.array(Image.open(p).convert('L'))
h,w=img.shape
black_cols=sum(1 for x in range(w) if (img[:,x]<30).mean()>0.8)
print(f"shape={w}x{h} mean={img.mean():.1f} black_pct={(img<30).mean()*100:.1f}% black_cols_80pct={black_cols}")
heavy=[i for i in range(w) if (img[:,i]<30).mean()>0.5]
print(f"cols>50%black: {len(heavy)}")
PY
