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
  local daemon_pidfile="${CZ_DAEMON_PIDFILE:-/tmp/cz_ctl.daemon.pid}"
  export CZ_OUT="$out_dir"
  cz_ctl_init
  rm -f "$FIFO"
  mkfifo "$FIFO"
  exec 3<>"$FIFO"
  export CZ_GOTO="${CZ_GOTO:-}"
  echo $$ >"$daemon_pidfile"
  # Give xvfb a beat before the window actor opens the display.
  sleep 0.5
  # Prefer Vulkan (incl. lavapipe). Forcing GL breaks headgroup shade: fragment
  # storage buffers are unsupported (max_storage_buffers_per_shader_stage=0).
  export WGPU_BACKEND="${WGPU_BACKEND:-vulkan}"
  taskset -c "${CZ_CPUSET:-4-11}" "$BIN" --beats 300000 -r 2 >/tmp/cz_xvfb.log 2>&1 &
  echo $! >"$PIDFILE"
  cleanup() {
    local app
    app="$(cat "$PIDFILE" 2>/dev/null || true)"
    if [ -n "$app" ]; then
      kill -TERM "$app" 2>/dev/null || true
      for _ in $(seq 1 50); do
        kill -0 "$app" 2>/dev/null || break
        sleep 0.1
      done
      kill -KILL "$app" 2>/dev/null || true
      wait "$app" 2>/dev/null || true
    fi
    rm -f "$FIFO" "$PIDFILE" "${daemon_pidfile:-/tmp/cz_ctl.daemon.pid}" "${CZ_ENVFILE:-/tmp/cz_ctl.env}"
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
  local daemon_pidfile="${CZ_DAEMON_PIDFILE:-/tmp/cz_ctl.daemon.pid}"
  local app_pid="" daemon_pid=""
  if [ -f "$PIDFILE" ]; then
    app_pid="$(cat "$PIDFILE")"
  fi
  if [ -f "$daemon_pidfile" ]; then
    daemon_pid="$(cat "$daemon_pidfile")"
  fi
  # Writing to a FIFO blocks forever with no reader. Only nudge a live daemon,
  # and never block the stop path on a stale pipe from a crashed prior run.
  if [ -p "$FIFO" ] && [ -n "$daemon_pid" ] && kill -0 "$daemon_pid" 2>/dev/null; then
    if command -v timeout >/dev/null 2>&1; then
      timeout 1 bash -c "printf 'stop\n' >>\"$FIFO\"" 2>/dev/null || true
    else
      printf 'stop\n' >>"$FIFO" &
      sleep 0.2
      kill $! 2>/dev/null || true
    fi
    for _ in $(seq 1 50); do
      daemon_alive=0
      app_alive=0
      if [ -n "$daemon_pid" ] && kill -0 "$daemon_pid" 2>/dev/null; then
        daemon_alive=1
      fi
      if [ -n "$app_pid" ] && kill -0 "$app_pid" 2>/dev/null; then
        app_alive=1
      fi
      if [ "$daemon_alive" -eq 0 ] && [ "$app_alive" -eq 0 ]; then
        break
      fi
      sleep 0.1
    done
  fi
  if [ -n "$app_pid" ]; then
    kill -TERM "$app_pid" 2>/dev/null || true
    sleep 0.2
    kill -KILL "$app_pid" 2>/dev/null || true
  fi
  if [ -n "$daemon_pid" ]; then
    kill -TERM "$daemon_pid" 2>/dev/null || true
    sleep 0.2
    kill -KILL "$daemon_pid" 2>/dev/null || true
  fi
  while read -r orphan; do
    kill -TERM "$orphan" 2>/dev/null || true
  done < <(pgrep -f "^${BIN}( |$)" || true)
  sleep 0.2
  while read -r orphan; do
    kill -KILL "$orphan" 2>/dev/null || true
  done < <(pgrep -f "^${BIN}( |$)" || true)
  rm -f "${CZ_ENVFILE:-/tmp/cz_ctl.env}" "$PIDFILE" "$FIFO" "$daemon_pidfile"
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
