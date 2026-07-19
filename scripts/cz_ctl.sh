#!/usr/bin/env bash
# Interactive control for a running critical_zoomer session.
#
#   taskset -c 4-11 xvfb-run -a -s "-screen 0 900x500x24" scripts/cz_ctl.sh start [out_dir]
#   scripts/cz_ctl.sh send 'capture a.png'
#   scripts/cz_ctl.sh stop
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=scripts/cz_ctl_lib.sh
source "$ROOT/scripts/cz_ctl_lib.sh"

usage() {
  echo "usage: $0 start [out_dir] | send 'CMD ...' | stop | status" >&2
}

cz_ctl_daemon() {
  local out_dir="$1"
  export CZ_OUT="$out_dir"
  cz_ctl_init
  rm -f "$FIFO"
  mkfifo "$FIFO"
  exec 3<>"$FIFO"
  export CZ_GOTO="${CZ_GOTO:-}"
  taskset -c "${CZ_CPUSET:-4-11}" "$BIN" --beats 300000 -r 2 >/tmp/cz_xvfb.log 2>&1 &
  echo $! >"$PIDFILE"
  cleanup() {
    kill "$(cat "$PIDFILE")" 2>/dev/null || true
    wait "$(cat "$PIDFILE")" 2>/dev/null || true
    rm -f "$FIFO" "$PIDFILE"
  }
  trap cleanup EXIT
  printf 'DISPLAY=%s\nXAUTHORITY=%s\nOUT=%s\n' "${DISPLAY:-}" "${XAUTHORITY:-}" "$OUT" >"${CZ_ENVFILE:-/tmp/cz_ctl.env}"
  while IFS= read -r line || [ -n "$line" ]; do
    cz_ctl_run_line "$line" || {
      code=$?
      if [ "$code" -eq 2 ]; then
        break
      fi
      exit "$code"
    }
  done <&3
  exec 3<&-
}

cmd_start() {
  local out_dir="${1:-/tmp/cz_ctl_capture}"
  if [ -f "${CZ_PIDFILE:-/tmp/cz_ctl.pid}" ] && kill -0 "$(cat "${CZ_PIDFILE:-/tmp/cz_ctl.pid}")" 2>/dev/null; then
    echo "cz_ctl already running pid=$(cat "${CZ_PIDFILE:-/tmp/cz_ctl.pid}")" >&2
    exit 1
  fi
  cz_ctl_daemon "$out_dir"
}

cz_ctl_status_quiet() {
  cz_ctl_init
  if [ -f "${CZ_ENVFILE:-/tmp/cz_ctl.env}" ]; then
    # shellcheck source=/dev/null
    source "${CZ_ENVFILE:-/tmp/cz_ctl.env}"
  fi
  if [ -f "$PIDFILE" ] && kill -0 "$(cat "$PIDFILE")" 2>/dev/null; then
    echo "running app_pid=$(cat "$PIDFILE") fifo=$FIFO out=$OUT"
    return 0
  fi
  return 1
}

cmd_send() {
  cz_ctl_init
  if [ -f "${CZ_ENVFILE:-/tmp/cz_ctl.env}" ]; then
    # shellcheck source=/dev/null
    source "${CZ_ENVFILE:-/tmp/cz_ctl.env}"
  fi
  if [ ! -p "$FIFO" ]; then
    echo "cz_ctl not running (no fifo at $FIFO)" >&2
    exit 1
  fi
  {
    printf '%s\n' "$*"
  } >>"$FIFO"
}

cmd_stop() {
  cz_ctl_init
  if [ -f "$PIDFILE" ]; then
    kill "$(cat "$PIDFILE")" 2>/dev/null || true
  fi
  pkill -f "$BIN" 2>/dev/null || true
  rm -f "${CZ_ENVFILE:-/tmp/cz_ctl.env}" "$PIDFILE" "$FIFO"
}

cmd_status() {
  cz_ctl_status_quiet || { echo "not running"; exit 1; }
}

case "${1:-}" in
  start) shift; cmd_start "${1:-}" ;;
  send) shift; [ $# -gt 0 ] || { usage; exit 1; }; cmd_send "$@" ;;
  stop) cmd_stop ;;
  status) cmd_status ;;
  *) usage; exit 1 ;;
esac
