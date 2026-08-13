#!/usr/bin/env bash
# Workspace watch for JFT zoomer-groove skill (content stays in JFT_Prompts).
set -u
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
export WATCH_FILE="/home/jonathan/git/JFT_Prompts/skills/zoomer-groove/SKILL.md"
export PENDING="/tmp/jft-zoomer-groove.pending"
export WAKE="/tmp/jft-zoomer-groove.wake"
export PIDFILE="/tmp/jft-zoomer-groove-watch.pid"
export SENTINEL="AGENT_ZOOMER_GROOVE_CHANGED"
WATCHER="$ROOT/.cursor/hooks/watch-jft-file.py"
MODE="${1:-post}"
MSG="JFT zoomer-groove skill changed. Re-read /home/jonathan/git/JFT_Prompts/skills/zoomer-groove/SKILL.md now; acknowledge what changed and how it affects behavior."

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
  [[ -f "$WATCH_FILE" ]] || return 0
  : >"$WAKE"
  nohup env WATCH_FILE="$WATCH_FILE" PENDING="$PENDING" WAKE="$WAKE" PIDFILE="$PIDFILE" SENTINEL="$SENTINEL" \
    python3 "$WATCHER" >/tmp/jft-zoomer-groove-watch.log 2>&1 &
  disown || true
}

emit_msg() {
  if [[ "$MODE" == "stop" ]]; then
    python3 -c 'import json,sys; print(json.dumps({"followup_message": sys.argv[1]}))' "$MSG"
  else
    python3 -c 'import json,sys; print(json.dumps({"additional_context": sys.argv[1]}))' "$MSG"
  fi
}

ensure_watcher

if [[ "$MODE" == "start" ]]; then
  rm -f "$PENDING"
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
