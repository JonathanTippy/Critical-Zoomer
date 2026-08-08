#!/usr/bin/env bash
# Interactive control for a running critical_zoomer session.
#
#   taskset -c 4-11 xvfb-run -a -s "-screen 0 900x500x24" scripts/cz_ctl.sh start [out_dir]
#   scripts/cz_ctl.sh send 'capture a.png'
#   scripts/cz_ctl.sh stop
#
# Session isolation: set CZ_SESSION_PREFIX or individual CZ_OUT/CZ_FIFO/… before start.
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
  echo $$ >"$DAEMON_PIDFILE"
  # Wait until the X display is actually accepting connections (xvfb-run -a
  # can still be bringing the server up when this daemon is backgrounded).
  if [ -z "${DISPLAY:-}" ]; then
    echo "cz_ctl start requires DISPLAY (run under xvfb-run)" >&2
    exit 1
  fi
  # Never pop a window on the developer's real display unless explicitly allowed.
  case "${DISPLAY}" in
    :0|:0.*)
      if [ "${CZ_ALLOW_REAL_DISPLAY:-0}" != "1" ]; then
        echo "cz_ctl refuses DISPLAY=${DISPLAY} (would appear on your screen)." >&2
        echo "Use xvfb-run, or set CZ_ALLOW_REAL_DISPLAY=1 to override." >&2
        exit 1
      fi
      ;;
  esac
  for _ in $(seq 1 150); do
    if xdpyinfo >/dev/null 2>&1; then
      break
    fi
    sleep 0.1
  done
  if ! xdpyinfo >/dev/null 2>&1; then
    echo "X display ${DISPLAY:-unset} never became ready" >&2
    exit 1
  fi
  # Prefer Vulkan (incl. lavapipe). Forcing GL breaks headgroup shade: fragment
  # storage buffers are unsupported (max_storage_buffers_per_shader_stage=0).
  # Under non-:0 displays, pin Mesa EGL so NVIDIA EGL/GBM cannot segfault Xvfb.
  # Do not force VK_ICD — hardware Vulkan often still paints; lavapipe can OOM/crash.
  export WGPU_BACKEND="${WGPU_BACKEND:-vulkan}"
  case "${DISPLAY:-}" in
    :0|:0.0) ;;
    *)
      if [ -z "${__EGL_VENDOR_LIBRARY_FILENAMES:-}" ] && [ -f /usr/share/glvnd/egl_vendor.d/50_mesa.json ]; then
        export __EGL_VENDOR_LIBRARY_FILENAMES=/usr/share/glvnd/egl_vendor.d/50_mesa.json
        echo "cz_ctl: Mesa EGL for DISPLAY=$DISPLAY" >&2
      fi
      # Product default is GPU-preferred (resident bouts). Keep
      # CZ_FORCE_CPU_BOUTS=1 as an explicit escape hatch for starved Xvfb hosts.
      if [ "${CZ_FORCE_CPU_BOUTS:-}" = "1" ]; then
        echo "cz_ctl: CZ_FORCE_CPU_BOUTS=1 (explicit) for DISPLAY=$DISPLAY" >&2
      fi
      ;;
  esac
  # So the app polls the session-isolated harness files.
  export CZ_GOTOFILE="$GOTOFILE"
  export CZ_NAVFILE="$NAVFILE"
  taskset -c "${CZ_CPUSET:-4-11}" "$BIN" --beats 300000 -r 2 >"$XVFB_LOG" 2>&1 &
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
    rm -f "$FIFO" "$PIDFILE" "$DAEMON_PIDFILE" "$ENVFILE"
  }
  trap cleanup EXIT
  printf 'DISPLAY=%s\nXAUTHORITY=%s\nOUT=%s\n' "${DISPLAY:-}" "${XAUTHORITY:-}" "$OUT" >"$ENVFILE"
  while IFS= read -r line || [ -n "$line" ]; do
    cz_ctl_run_line "$line" || {
      code=$?
      if [ "$code" -eq 2 ]; then
        break
      fi
      # Keep serving the fifo: settle/capture asserts may fail without killing the session.
      echo "cz_ctl: command failed code=$code: $line" >&2
    }
  done <&3
  exec 3<&-
}

cmd_start() {
  local out_dir="${1:-}"
  if [ -n "${CZ_SESSION_PREFIX:-}" ]; then
    cz_ctl_session_from_prefix "$CZ_SESSION_PREFIX"
  fi
  if [ -z "$out_dir" ]; then
    out_dir="${CZ_OUT:-/tmp/cz_ctl_capture}"
  fi
  cz_ctl_init
  if [ -f "$PIDFILE" ] && kill -0 "$(cat "$PIDFILE")" 2>/dev/null; then
    echo "cz_ctl already running pid=$(cat "$PIDFILE")" >&2
    exit 1
  fi
  cz_ctl_daemon "$out_dir"
}

cz_ctl_status_quiet() {
  if [ -n "${CZ_SESSION_PREFIX:-}" ]; then
    cz_ctl_session_from_prefix "$CZ_SESSION_PREFIX"
  fi
  cz_ctl_init
  if [ -f "$ENVFILE" ]; then
    # shellcheck source=/dev/null
    source "$ENVFILE"
  fi
  if [ -f "$PIDFILE" ] && kill -0 "$(cat "$PIDFILE")" 2>/dev/null; then
    echo "running app_pid=$(cat "$PIDFILE") fifo=$FIFO out=$OUT"
    return 0
  fi
  return 1
}

cmd_send() {
  if [ -n "${CZ_SESSION_PREFIX:-}" ]; then
    cz_ctl_session_from_prefix "$CZ_SESSION_PREFIX"
  fi
  cz_ctl_init
  if [ -f "$ENVFILE" ]; then
    # shellcheck source=/dev/null
    source "$ENVFILE"
  fi
  if [ ! -p "$FIFO" ]; then
    echo "cz_ctl not running (no fifo at $FIFO)" >&2
    exit 1
  fi
  if command -v timeout >/dev/null 2>&1; then
    timeout 2 bash -c "printf '%s\n' \"\$1\" >>\"\$2\"" _ "$*" "$FIFO" || {
      echo "cz_ctl send timed out writing fifo $FIFO" >&2
      exit 1
    }
  else
    {
      printf '%s\n' "$*"
    } >>"$FIFO"
  fi
}

cmd_stop() {
  if [ -n "${CZ_SESSION_PREFIX:-}" ]; then
    cz_ctl_session_from_prefix "$CZ_SESSION_PREFIX"
  fi
  cz_ctl_init
  local app_pid="" daemon_pid=""
  if [ -f "$PIDFILE" ]; then
    app_pid="$(cat "$PIDFILE")"
  fi
  if [ -f "$DAEMON_PIDFILE" ]; then
    daemon_pid="$(cat "$DAEMON_PIDFILE")"
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
  # Do not kill unrelated app instances belonging to another session prefix.
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
  # Kill xvfb-run process group for this session if recorded.
  if [ -n "${CZ_SESSION_PREFIX:-}" ] && [ -f "$CZ_SESSION_PREFIX/xvfb_wrapper.pid" ]; then
    local wrap
    wrap="$(cat "$CZ_SESSION_PREFIX/xvfb_wrapper.pid")"
    if [ -n "$wrap" ]; then
      kill -TERM -"$wrap" 2>/dev/null || kill -TERM "$wrap" 2>/dev/null || true
      sleep 0.2
      kill -KILL -"$wrap" 2>/dev/null || kill -KILL "$wrap" 2>/dev/null || true
    fi
  fi
  rm -f "$ENVFILE" "$PIDFILE" "$FIFO" "$DAEMON_PIDFILE"
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
