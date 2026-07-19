#!/usr/bin/env bash
# Control helpers for Critical Zoomer under xvfb / live DISPLAY.
# Pixel-judged settle via ImageMagick stdev. No research-HUD coupling.
set -euo pipefail

cz_ctl_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd
}

cz_ctl_init() {
  ROOT="$(cz_ctl_root)"
  if [ -x "/tmp/cz_build_target/release/critical_zoomer" ]; then
    BIN="/tmp/cz_build_target/release/critical_zoomer"
  elif [ -x "$ROOT/target/release/critical_zoomer" ]; then
    BIN="$ROOT/target/release/critical_zoomer"
  else
    BIN="$ROOT/target/release/critical_zoomer"
  fi
  OUT="${CZ_OUT:-/tmp/cz_ctl_capture}"
  FIFO="${CZ_FIFO:-/tmp/cz_ctl.fifo}"
  PIDFILE="${CZ_PIDFILE:-/tmp/cz_ctl.pid}"
  export WINIT_X11_SCALE_FACTOR=1
  mkdir -p "$OUT"
  WIN=""
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

cz_ctl_require_window() {
  if [ -z "$WIN" ]; then
    WIN=$(cz_ctl_find_window || true)
  fi
  if [ -z "$WIN" ]; then
    echo "no Critical Zoomer window" >&2
    return 1
  fi
}

cz_ctl_focus_window() {
  cz_ctl_require_window
  xdotool windowfocus --sync "$WIN" 2>/dev/null || true
  xdotool mousemove --window "$WIN" 450 250 2>/dev/null || true
  xdotool click --window "$WIN" 1 2>/dev/null || true
}

cz_ctl_capture_to() {
  local name="$1"
  cz_ctl_require_window
  import -silent -window "$WIN" "$OUT/$name"
  echo "wrote $OUT/$name"
}

cz_ctl_image_stdev() {
  local path="$1"
  identify -format '%[standard-deviation]' "$path" 2>/dev/null \
    | awk '{print int($1)}'
}

cz_ctl_image_settled() {
  local path="$1"
  local min_stdev="${2:-4500}"
  local stdev
  stdev=$(cz_ctl_image_stdev "$path")
  [ -n "$stdev" ] && [ "$stdev" -ge "$min_stdev" ]
}

cz_ctl_wait_settled() {
  local name="$1"
  local max_secs="${2:-30}"
  local min_stdev="${3:-4500}"
  local i=0
  while [ "$i" -lt "$max_secs" ]; do
    cz_ctl_capture_to "$name"
    if cz_ctl_image_settled "$OUT/$name" "$min_stdev"; then
      echo "settled stdev=$(cz_ctl_image_stdev "$OUT/$name")"
      return 0
    fi
    sleep 1
    i=$((i + 1))
  done
  echo "not settled after ${max_secs}s stdev=$(cz_ctl_image_stdev "$OUT/$name" 2>/dev/null || echo 0)" >&2
  return 1
}

cz_ctl_send_home() {
  cz_ctl_focus_window
  # Prefer goto; icon click coords are resolution-fragile.
  cz_ctl_send_goto -2 -2 -2
  sleep 0.2
  xdotool mousemove --window "$WIN" 840 20
  xdotool click --window "$WIN" 1
  sleep 0.2
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
  printf '%s\n' "$*" >"${CZ_NAVFILE:-/tmp/cz_ctl.navigate}"
  sleep 0.1
}

cz_ctl_send_goto() {
  printf '%s\n' "$*" >"${CZ_GOTOFILE:-/tmp/cz_ctl.goto}"
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
    settle) cz_ctl_wait_settled "$1" "${2:-30}" "${3:-4500}" ;;
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
