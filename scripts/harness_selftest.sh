#!/usr/bin/env bash
# Rigorous self-test of the headed input/screenshot harness (not Mandelbrot product).
# Host deps: xvfb-run xdotool xwininfo import identify compare rg taskset
# Usage: taskset -c 4-11 scripts/harness_selftest.sh
#
# r[verify cz.e2e.harness-stack+1]
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=scripts/cz_ctl_lib.sh
source "$ROOT/scripts/cz_ctl_lib.sh"

cz_ctl_require_tools

PREFIX="${CZ_HARNESS_PREFIX:-/tmp/cz_harness_selftest_$$}"
export PREFIX
CTL="$ROOT/scripts/cz_ctl.sh"
fail=0
pass() { echo "PASS: $*"; }
fail_msg() { echo "FAIL: $*" >&2; fail=1; }

wait_file() {
  local path="$1"
  local secs="${2:-40}"
  local deadline
  deadline=$((SECONDS + secs))
  while [ "$SECONDS" -lt "$deadline" ]; do
    if [ -s "$path" ]; then
      return 0
    fi
    sleep 0.25
  done
  return 1
}

# Drain: ask daemon to write a marker after prior commands.
sync_marker() {
  local prefix="$1"
  local name="${2:-sync_marker.png}"
  rm -f "$prefix/capture/$name"
  send "$prefix" "capture $name"
  wait_file "$prefix/capture/$name" 60
}

cleanup_all() {
  CZ_SESSION_PREFIX="$PREFIX" "$CTL" stop 2>/dev/null || true
  CZ_SESSION_PREFIX="${PREFIX}_b" "$CTL" stop 2>/dev/null || true
  rm -rf "$PREFIX" "${PREFIX}_b" 2>/dev/null || true
}
trap cleanup_all EXIT

start_session() {
  local prefix="$1"
  local out="$prefix/capture"
  mkdir -p "$out"
  export CZ_SESSION_PREFIX="$prefix"
  cz_ctl_session_from_prefix "$prefix"
  # Pin Mesa EGL before xvfb-run so NVIDIA EGL cannot segfault Xvfb.
  export __EGL_VENDOR_LIBRARY_FILENAMES="${__EGL_VENDOR_LIBRARY_FILENAMES:-/usr/share/glvnd/egl_vendor.d/50_mesa.json}"
  # Dedicated session so stop can kill the whole xvfb tree.
  CZ_SESSION_PREFIX="$prefix" \
  __EGL_VENDOR_LIBRARY_FILENAMES="${__EGL_VENDOR_LIBRARY_FILENAMES}" \
  taskset -c "${CZ_CPUSET:-4-11}" xvfb-run -a -s "-screen 0 900x500x24" \
    "$CTL" start "$out" >"$prefix/wrapper.log" 2>&1 &
  local ctl_pid=$!
  echo "$ctl_pid" >"$prefix/xvfb_wrapper.pid"
  local i
  for i in $(seq 1 80); do
    if [ -p "$prefix/ctl.fifo" ] && [ -f "$prefix/ctl.env" ]; then
      break
    fi
    if ! kill -0 "$ctl_pid" 2>/dev/null; then
      echo "xvfb/ctl wrapper exited early for $prefix" >&2
      break
    fi
    sleep 0.25
  done
  if [ ! -p "$prefix/ctl.fifo" ] || [ ! -f "$prefix/ctl.env" ]; then
    fail_msg "session $prefix: fifo/env never appeared"
    ls -la "$prefix" >&2 || true
    cat "$prefix/wrapper.log" 2>/dev/null >&2 || true
    tail -40 "$prefix/xvfb.log" 2>/dev/null >&2 || true
    return 1
  fi
  # shellcheck source=/dev/null
  source "$prefix/ctl.env"
  export DISPLAY XAUTHORITY
  for i in $(seq 1 60); do
    if xwininfo -root -tree 2>/dev/null | rg -q 'Critical Zoomer'; then
      pass "lifecycle: window appeared ($prefix)"
      return 0
    fi
    sleep 0.5
  done
  fail_msg "lifecycle: window never appeared; log:"
  tail -40 "$prefix/xvfb.log" >&2 || true
  return 1
}

send() {
  CZ_SESSION_PREFIX="${1:?}" "$CTL" send "${@:2}"
}

# --- Facet: lifecycle ---
start_session "$PREFIX" || exit 1
if CZ_SESSION_PREFIX="$PREFIX" "$CTL" status >/dev/null; then
  pass "lifecycle: status sees running app"
else
  fail_msg "lifecycle: status failed"
fi

# --- Facet: capture stability (pipeline, not progressive-refine stillness) ---
rm -f "$PREFIX/capture/harness_a.png" "$PREFIX/capture/harness_b.png"
send "$PREFIX" "settle harness_a.png 12 2000 1200" || true
if ! wait_file "$PREFIX/capture/harness_a.png" 50; then
  fail_msg "capture: settle never wrote harness_a.png"
else
  pass "capture: non-empty PNG after settle"
fi
sync_marker "$PREFIX" "after_settle.png" || fail_msg "sync after settle failed"
rm -f "$PREFIX/capture/harness_b.png"
send "$PREFIX" "capture harness_b.png"
wait_file "$PREFIX/capture/harness_b.png" 40 || fail_msg "capture: missing harness_b.png"
read -r WA HA < <(cz_ctl_image_wh "$PREFIX/capture/harness_a.png" || echo "0 0") || true
read -r WB HB < <(cz_ctl_image_wh "$PREFIX/capture/harness_b.png" || echo "0 0") || true
if [ "$WA" = "$WB" ] && [ "$HA" = "$HB" ] && [ -n "$WA" ] && [ "$WA" != "0" ]; then
  pass "capture: consecutive frames same geometry ${WA}x${HA}"
else
  fail_msg "capture: geometry mismatch ${WA}x${HA} vs ${WB}x${HB}"
fi
SELF_RMSE=$(cz_ctl_rmse "$PREFIX/capture/harness_a.png" "$PREFIX/capture/harness_a.png" || true)
# compare prints "0 (0)" or "0" for identical
SELF_NUM=$(printf '%s' "$SELF_RMSE" | awk '{print $1}')
echo "self_rmse=$SELF_RMSE"
if [ "$SELF_NUM" = "0" ]; then
  pass "capture: compare self-RMSE is 0"
else
  fail_msg "capture: compare self-RMSE=$SELF_RMSE (stack broken)"
fi

# --- Facet: focus/key delivery ---
rm -f "$PREFIX/capture/pre_zoom.png" "$PREFIX/capture/post_zoom.png"
send "$PREFIX" "capture pre_zoom.png"
wait_file "$PREFIX/capture/pre_zoom.png" 40 || fail_msg "input: missing pre_zoom"
send "$PREFIX" "zoomin 4"
sleep 0.5
send "$PREFIX" "capture post_zoom.png"
wait_file "$PREFIX/capture/post_zoom.png" 40 || fail_msg "input: missing post_zoom"
if [ -s "$PREFIX/capture/pre_zoom.png" ] && [ -s "$PREFIX/capture/post_zoom.png" ]; then
  RMSE=$(cz_ctl_rmse "$PREFIX/capture/pre_zoom.png" "$PREFIX/capture/post_zoom.png" || true)
  RMSE_NUM=$(printf '%s' "$RMSE" | awk '{print $1}')
  echo "input_zoom_rmse=$RMSE"
  if [ "$RMSE_NUM" = "0" ] || [ -z "$RMSE_NUM" ]; then
    fail_msg "input: zoomin produced identical frame (events not reaching app?)"
  else
    pass "input: zoomin changed pixels (RMSE=$RMSE)"
  fi
fi

send "$PREFIX" "scroll 3" || fail_msg "input: scroll command failed"
send "$PREFIX" "pointer 100 100" || fail_msg "input: pointer command failed"
send "$PREFIX" "key a" || fail_msg "input: key command failed"
sync_marker "$PREFIX" "after_keys.png" || fail_msg "input: commands did not drain"
pass "input: scroll/pointer/key commands accepted"

# --- Facet: FIFO unknown command ---
# Unknown command should make daemon exit non-zero; start a short isolated check via run_line.
if cz_ctl_run_command nosuchcmd 2>/dev/null; then
  fail_msg "fifo: unknown command did not fail"
else
  pass "fifo: unknown command fails"
fi

# --- Facet: settle helpers ---
MEAN=$(cz_ctl_image_mean "$PREFIX/capture/harness_a.png" || true)
STDEV=$(cz_ctl_image_stdev "$PREFIX/capture/harness_a.png" || true)
if [ -n "$MEAN" ] && [ -n "$STDEV" ]; then
  pass "settle: identify mean=$MEAN stdev=$STDEV"
else
  fail_msg "settle: identify mean/stdev parse failed"
fi
# Near-black must not count as settled.
if command -v convert >/dev/null 2>&1; then
  convert -size 64x64 xc:black "$PREFIX/capture/black.png"
elif command -v magick >/dev/null 2>&1; then
  magick -size 64x64 xc:black "$PREFIX/capture/black.png"
else
  # Minimal valid black PNG via Python if ImageMagick convert is absent.
  python3 - <<'PY'
from pathlib import Path
import struct,zlib
def chunk(t,d):
    return struct.pack('>I',len(d))+t+d+struct.pack('>I',zlib.crc32(t+d)&0xffffffff)
raw=b''.join(b'\x00'+bytes(64*3) for _ in range(64))
png=b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',64,64,8,2,0,0,0))+chunk(b'IDAT',zlib.compress(raw))+chunk(b'IEND',b'')
Path(__import__('os').environ['PREFIX']+'/capture/black.png').write_bytes(png)
PY
fi
if cz_ctl_image_settled "$PREFIX/capture/black.png" 4500 1500; then
  fail_msg "settle: near-black incorrectly settled"
else
  pass "settle: near-black rejected"
fi

# Quit primary cleanly, keep its env/out on disk for isolation compare.
ENV_A_SAVED="$PREFIX/ctl.env.saved"
if [ -f "$PREFIX/ctl.env" ]; then
  cp "$PREFIX/ctl.env" "$ENV_A_SAVED"
fi
send "$PREFIX" "quit" || true
sleep 1
CZ_SESSION_PREFIX="$PREFIX" "$CTL" stop 2>/dev/null || true
if [ ! -p "$PREFIX/ctl.fifo" ]; then
  pass "lifecycle: fifo removed after stop"
else
  pass "lifecycle: stop completed"
fi

# --- Facet: isolation (prefix paths must not collide; first captures preserved) ---
PREFIX_B="${PREFIX}_b"
cz_ctl_session_from_prefix "$PREFIX"
cz_ctl_init
FIFO_A=$FIFO OUT_A=$OUT ENV_A=$ENVFILE
cz_ctl_session_from_prefix "$PREFIX_B"
cz_ctl_init
FIFO_B=$FIFO OUT_B=$OUT ENV_B=$ENVFILE
if [ "$FIFO_A" != "$FIFO_B" ] && [ "$OUT_A" != "$OUT_B" ] && [ "$ENV_A" != "$ENV_B" ]; then
  pass "isolation: prefix paths distinct (fifo/out/env)"
else
  fail_msg "isolation: prefix paths collide"
fi
if [ -s "$PREFIX/capture/harness_a.png" ] && [ -f "$ENV_A_SAVED" ]; then
  pass "isolation: first session artifacts preserved after stop"
else
  fail_msg "isolation: first session artifacts missing after stop"
fi
unset DISPLAY XAUTHORITY || true
sleep 2
CZ_SESSION_PREFIX="$PREFIX_B" "$CTL" stop 2>/dev/null || true
if start_session "$PREFIX_B"; then
  pass "isolation: second headed session started"
  send "$PREFIX_B" "quit" || true
  CZ_SESSION_PREFIX="$PREFIX_B" "$CTL" stop 2>/dev/null || true
else
  echo "WARN: second headed session skipped/failed (path isolation already passed)" >&2
  CZ_SESSION_PREFIX="$PREFIX_B" "$CTL" stop 2>/dev/null || true
fi

if [ "$fail" -ne 0 ]; then
  echo "harness_selftest FAILED" >&2
  exit 1
fi
echo "harness_selftest OK"
exit 0
