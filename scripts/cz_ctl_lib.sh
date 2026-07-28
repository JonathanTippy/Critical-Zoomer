#!/usr/bin/env bash
# Control helpers for Critical Zoomer under xvfb / live DISPLAY.
#
# HARNESS_FROZEN: re-run `taskset -c 4-11 scripts/harness_selftest.sh` after any
# change to command surface, geometry, capture, or settle. Do not edit casually.
#
# Frozen command surface: wait focus pointer click key keydown keyup hold scroll
# zoomin zoomout goto navigate capture settle home quit|exit|stop
#
# Host deps: xvfb-run xdotool xwininfo import identify compare rg taskset
# Pixel-judged settle via ImageMagick stdev + mean floor. No research-HUD coupling.
set -euo pipefail

cz_ctl_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd
}

cz_ctl_require_tools() {
  local missing=0 t
  for t in xvfb-run xdotool xwininfo xdpyinfo import identify compare rg taskset; do
    if ! command -v "$t" >/dev/null 2>&1; then
      echo "missing required tool: $t" >&2
      missing=1
    fi
  done
  return "$missing"
}

# Apply a private session prefix (OUT/FIFO/PID/ENV/GOTO/NAV/DAEMON/LOG).
# Always overwrites CZ_* so a prior session cannot stick.
cz_ctl_session_from_prefix() {
  local prefix="$1"
  mkdir -p "$prefix"
  export CZ_OUT="$prefix/capture"
  export CZ_FIFO="$prefix/ctl.fifo"
  export CZ_PIDFILE="$prefix/app.pid"
  export CZ_ENVFILE="$prefix/ctl.env"
  export CZ_GOTOFILE="$prefix/ctl.goto"
  export CZ_NAVFILE="$prefix/ctl.navigate"
  export CZ_DAEMON_PIDFILE="$prefix/daemon.pid"
  export CZ_XVFB_LOG="$prefix/xvfb.log"
}

cz_ctl_init() {
  ROOT="$(cz_ctl_root)"
  # Prefer explicit override, then workspace release, then alternate build dir.
  if [ -n "${CZ_BIN:-}" ] && [ -x "${CZ_BIN}" ]; then
    BIN="$CZ_BIN"
  elif [ -x "$ROOT/target/release/critical_zoomer" ]; then
    BIN="$ROOT/target/release/critical_zoomer"
  elif [ -x "/tmp/cz_build_target/release/critical_zoomer" ]; then
    BIN="/tmp/cz_build_target/release/critical_zoomer"
  else
    BIN="$ROOT/target/release/critical_zoomer"
  fi
  OUT="${CZ_OUT:-/tmp/cz_ctl_capture}"
  FIFO="${CZ_FIFO:-/tmp/cz_ctl.fifo}"
  PIDFILE="${CZ_PIDFILE:-/tmp/cz_ctl.pid}"
  GOTOFILE="${CZ_GOTOFILE:-/tmp/cz_ctl.goto}"
  NAVFILE="${CZ_NAVFILE:-/tmp/cz_ctl.navigate}"
  ENVFILE="${CZ_ENVFILE:-/tmp/cz_ctl.env}"
  DAEMON_PIDFILE="${CZ_DAEMON_PIDFILE:-/tmp/cz_ctl.daemon.pid}"
  XVFB_LOG="${CZ_XVFB_LOG:-/tmp/cz_xvfb.log}"
  export WINIT_X11_SCALE_FACTOR=1
  mkdir -p "$OUT"
  WIN=""
  WIN_W=""
  WIN_H=""
}

cz_ctl_xenv() {
  if [ -n "${DISPLAY:-}" ]; then
    export DISPLAY
  fi
  if [ -n "${XAUTHORITY:-}" ]; then
    export XAUTHORITY
  fi
}

cz_ctl_find_window() {
  cz_ctl_xenv
  xwininfo -root -tree 2>/dev/null \
    | rg -o '0x[0-9a-f]+ "Critical Zoomer"' \
    | head -1 \
    | awk '{print $1}'
}

cz_ctl_window_geom() {
  cz_ctl_require_window
  local info
  info=$(xwininfo -id "$WIN" 2>/dev/null) || return 1
  WIN_W=$(printf '%s\n' "$info" | awk '/Width:/ {print $2; exit}')
  WIN_H=$(printf '%s\n' "$info" | awk '/Height:/ {print $2; exit}')
  if [ -z "$WIN_W" ] || [ -z "$WIN_H" ] || [ "$WIN_W" -lt 2 ] || [ "$WIN_H" -lt 2 ]; then
    echo "bad window geometry for $WIN" >&2
    return 1
  fi
}

cz_ctl_center_xy() {
  cz_ctl_window_geom
  echo "$((WIN_W / 2)) $((WIN_H / 2))"
}

cz_ctl_require_window() {
  if [ -z "${WIN:-}" ]; then
    WIN=$(cz_ctl_find_window || true)
  fi
  if [ -z "${WIN:-}" ]; then
    echo "no Critical Zoomer window" >&2
    return 1
  fi
}

cz_ctl_focus_window() {
  local cx cy
  cz_ctl_require_window
  xdotool windowfocus --sync "$WIN" 2>/dev/null || true
  read -r cx cy < <(cz_ctl_center_xy)
  xdotool mousemove --window "$WIN" "$cx" "$cy" 2>/dev/null || true
  xdotool click --window "$WIN" 1 2>/dev/null || true
}

cz_ctl_image_mean() {
  local path="$1"
  identify -format '%[mean]' "$path" 2>/dev/null \
    | awk '{print int($1)}'
}

cz_ctl_image_stdev() {
  local path="$1"
  identify -format '%[standard-deviation]' "$path" 2>/dev/null \
    | awk '{print int($1)}'
}

cz_ctl_image_wh() {
  local path="$1"
  identify -format '%w %h' "$path" 2>/dev/null
}

cz_ctl_rmse() {
  local a="$1" b="$2"
  compare -metric RMSE "$a" "$b" null: 2>&1 | awk '{print $1}' || true
}

cz_ctl_capture_to() {
  local name="$1"
  local path wh iw ih
  cz_ctl_require_window
  cz_ctl_window_geom
  path="$OUT/$name"
  if ! import -silent -window "$WIN" "$path"; then
    echo "capture failed for $path" >&2
    return 1
  fi
  if [ ! -s "$path" ]; then
    echo "capture produced empty $path" >&2
    return 1
  fi
  read -r iw ih < <(cz_ctl_image_wh "$path")
  if [ -z "${iw:-}" ] || [ -z "${ih:-}" ]; then
    echo "capture identify failed for $path" >&2
    return 1
  fi
  # Allow 1px slack for window decorations / rounding.
  if [ "$iw" -lt $((WIN_W - 2)) ] || [ "$ih" -lt $((WIN_H - 2)) ]; then
    echo "capture size ${iw}x${ih} smaller than window ${WIN_W}x${WIN_H}" >&2
    return 1
  fi
  echo "wrote $path (${iw}x${ih})"
}

cz_ctl_image_center_stdev() {
  local path="$1"
  local tmp crop
  tmp=$(mktemp --suffix=.png)
  if ! convert "$path" -gravity Center -crop 400x300+0+0 +repage "$tmp" 2>/dev/null; then
    rm -f "$tmp"
    echo 0
    return
  fi
  crop=$(identify -format '%[standard-deviation]' "$tmp" 2>/dev/null | awk '{printf "%d", $1}')
  rm -f "$tmp"
  echo "${crop:-0}"
}

cz_ctl_image_settled() {
  local path="$1"
  local min_stdev="${2:-4500}"
  local min_mean="${3:-1500}"
  # Fourth optional arg: min center-crop stdev (rejects HUD-only frames).
  local min_center="${4:-2000}"
  local stdev mean center
  stdev=$(cz_ctl_image_stdev "$path")
  mean=$(cz_ctl_image_mean "$path")
  center=$(cz_ctl_image_center_stdev "$path")
  [ -n "$stdev" ] && [ "$stdev" -ge "$min_stdev" ] \
    && [ -n "$mean" ] && [ "$mean" -ge "$min_mean" ] \
    && [ -n "$center" ] && [ "$center" -ge "$min_center" ]
}

cz_ctl_wait_settled() {
  local name="$1"
  local max_secs="${2:-30}"
  local min_stdev="${3:-4500}"
  local min_mean="${4:-1500}"
  local min_center="${5:-2000}"
  local i=0
  while [ "$i" -lt "$max_secs" ]; do
    cz_ctl_capture_to "$name"
    if cz_ctl_image_settled "$OUT/$name" "$min_stdev" "$min_mean" "$min_center"; then
      echo "settled mean=$(cz_ctl_image_mean "$OUT/$name") stdev=$(cz_ctl_image_stdev "$OUT/$name") center=$(cz_ctl_image_center_stdev "$OUT/$name")"
      return 0
    fi
    sleep 1
    i=$((i + 1))
  done
  echo "not settled after ${max_secs}s mean=$(cz_ctl_image_mean "$OUT/$name" 2>/dev/null || echo 0) stdev=$(cz_ctl_image_stdev "$OUT/$name" 2>/dev/null || echo 0) center=$(cz_ctl_image_center_stdev "$OUT/$name" 2>/dev/null || echo 0)" >&2
  return 1
}

cz_ctl_send_home() {
  # Prefer goto home stencil; icon click is geometry-relative fallback only.
  cz_ctl_focus_window
  cz_ctl_send_goto -2 -2 -2
  sleep 0.25
}

cz_ctl_send_key() {
  cz_ctl_focus_window
  xdotool key --clearmodifiers --window "$WIN" "$1"
}

cz_ctl_send_keydown() {
  cz_ctl_focus_window
  xdotool keydown --window "$WIN" "$1"
}

cz_ctl_send_keyup() {
  cz_ctl_focus_window
  xdotool keyup --window "$WIN" "$1"
}

cz_ctl_send_hold() {
  local key="$1"
  local secs="$2"
  cz_ctl_send_keydown "$key"
  sleep "$secs"
  cz_ctl_send_keyup "$key"
}

cz_ctl_send_pointer() {
  cz_ctl_focus_window
  xdotool mousemove --window "$WIN" "$1" "$2"
}

cz_ctl_send_scroll() {
  local n="$1"
  local btn count i
  cz_ctl_focus_window
  if [ "$n" -gt 0 ]; then
    btn=4
    count="$n"
  else
    btn=5
    count=$((-n))
  fi
  i=0
  while [ "$i" -lt "$count" ]; do
    xdotool click --window "$WIN" "$btn"
    sleep 0.05
    i=$((i + 1))
  done
}

cz_ctl_send_zoomin() {
  local n="${1:-1}"
  local i=0
  cz_ctl_focus_window
  while [ "$i" -lt "$n" ]; do
    xdotool key --clearmodifiers --window "$WIN" Shift_L
    sleep 0.15
    i=$((i + 1))
  done
}

cz_ctl_send_zoomout() {
  local n="${1:-1}"
  local i=0
  cz_ctl_focus_window
  while [ "$i" -lt "$n" ]; do
    xdotool key --clearmodifiers --window "$WIN" space
    sleep 0.15
    i=$((i + 1))
  done
}

cz_ctl_send_navigate() {
  printf '%s\n' "$*" >"$NAVFILE"
  sleep 0.1
}

cz_ctl_send_goto() {
  printf '%s\n' "$*" >"$GOTOFILE"
  sleep 0.1
}

cz_ctl_run_command() {
  local cmd="$1"
  shift
  case "$cmd" in
    wait) sleep "$1" ;;
    focus) cz_ctl_focus_window ;;
    pointer) cz_ctl_send_pointer "$1" "$2" ;;
    click) cz_ctl_focus_window; xdotool click --window "$WIN" 1 ;;
    key) cz_ctl_send_key "$1" ;;
    keydown) cz_ctl_send_keydown "$1" ;;
    keyup) cz_ctl_send_keyup "$1" ;;
    hold) cz_ctl_send_hold "$1" "$2" ;;
    scroll) cz_ctl_send_scroll "$1" ;;
    zoomin) cz_ctl_send_zoomin "${1:-1}" ;;
    zoomout) cz_ctl_send_zoomout "${1:-1}" ;;
    goto) cz_ctl_send_goto "$@" ;;
    navigate) cz_ctl_send_navigate "$@" ;;
    capture) cz_ctl_capture_to "$1" ;;
    settle) cz_ctl_wait_settled "$1" "${2:-30}" "${3:-4500}" "${4:-1500}" "${5:-2000}" ;;
    home) cz_ctl_send_home ;;
    quit|exit|stop) return 2 ;;
    *)
      echo "unknown command: $cmd" >&2
      return 1
      ;;
  esac
}

cz_ctl_run_line() {
  local line="$1"
  line="${line%%#*}"
  line="${line#"${line%%[![:space:]]*}"}"
  [ -z "$line" ] && return 0
  set -- $line
  cz_ctl_run_command "$@"
}
