#!/usr/bin/env bash
# Deliver zoomer-groove skill notifier msg to Cursor hooks (workspace).
set -u
TARGET="/home/jonathan/git/JFT_Prompts/skills/zoomer-groove/SKILL.md"
PENDING="/tmp/jft-zoomer-groove.pending"
WAKE="/tmp/jft-zoomer-groove.wake"
PIDFILE="/tmp/jft-zoomer-groove-watch.pid"
BASELINE="/tmp/jft-zoomer-groove.baseline"
LATEST="/tmp/jft-zoomer-groove.latest.msg"
WATCHER="$(cd "$(dirname "$0")" && pwd)/watch-zoomer-groove.py"
MODE="${1:-post}"

cat >/dev/null || true

watcher_alive() {
  [[ -f "$PIDFILE" ]] || return 1
  local pid
  pid="$(tr -d '[:space:]' < "$PIDFILE")"
  [[ -n "$pid" ]] || return 1
  [[ -d "/proc/$pid" ]]
}

ensure_watcher() {
  watcher_alive && return 0
  [[ -f "$TARGET" ]] || return 0
  : >"$WAKE"
  cp -a "$TARGET" "$BASELINE"
  nohup python3 "$WATCHER" >/tmp/jft-zoomer-groove-watch.log 2>&1 &
  disown || true
}

emit_msg() {
  local msg
  if [[ ! -f "$LATEST" ]]; then
    echo '{}'
    return 0
  fi
  msg="$(cat "$LATEST")"
  if [[ "$MODE" == "stop" ]]; then
    python3 -c 'import json,sys; print(json.dumps({"followup_message": sys.argv[1]}))' "$msg"
  else
    python3 -c 'import json,sys; print(json.dumps({"additional_context": sys.argv[1]}))' "$msg"
  fi
}

ensure_watcher

if [[ "$MODE" == "start" ]]; then
  rm -f "$PENDING"
  cp -a "$TARGET" "$BASELINE"
  echo '{}'
  exit 0
fi

if [[ -f "$PENDING" ]]; then
  rm -f "$PENDING"
  emit_msg
  exit 0
fi

echo '{}'
exit 0
