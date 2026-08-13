#!/usr/bin/env bash
# Workspace watch for JFT zoomer-groove skill (content stays in JFT_Prompts).
set -u
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
export WATCH_FILE="/home/jonathan/git/JFT_Prompts/skills/zoomer-groove/SKILL.md"
export PENDING="/tmp/jft-zoomer-groove.pending"
export WAKE="/tmp/jft-zoomer-groove.wake"
export PIDFILE="/tmp/jft-zoomer-groove-watch.pid"
export SENTINEL="AGENT_ZOOMER_GROOVE_CHANGED"
LAST="/tmp/jft-zoomer-groove.last"
WATCHER="$ROOT/.cursor/hooks/watch-jft-file.py"
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
  [[ -f "$WATCH_FILE" ]] || return 0
  : >"$WAKE"
  if [[ ! -f "$LAST" ]]; then
    cp -a "$WATCH_FILE" "$LAST"
  fi
  nohup env WATCH_FILE="$WATCH_FILE" PENDING="$PENDING" WAKE="$WAKE" PIDFILE="$PIDFILE" SENTINEL="$SENTINEL" \
    python3 "$WATCHER" >/tmp/jft-zoomer-groove-watch.log 2>&1 &
  disown || true
}

build_msg() {
  local diff
  if [[ -f "$LAST" ]]; then
    diff="$(diff -u "$LAST" "$WATCH_FILE" || true)"
  else
    diff="(no prior snapshot)"
  fi
  cp -a "$WATCH_FILE" "$LAST"
  printf '%s\n' \
    "The file ${WATCH_FILE} has been edited on disk." \
    "The changes follow:" \
    "${diff}" \
    "acknowledge what changed and how that will effect your behavior." \
    "Try to infer what I wanted in writing that. if I change how something is tested or configured, that means I want it to be in the new standard." \
    "If this yields an action item, consider this a interruption and execute it immediately."
}

emit_msg() {
  local msg
  msg="$(build_msg)"
  if [[ "$MODE" == "stop" ]]; then
    python3 -c 'import json,sys; print(json.dumps({"followup_message": sys.argv[1]}))' "$msg"
  else
    python3 -c 'import json,sys; print(json.dumps({"additional_context": sys.argv[1]}))' "$msg"
  fi
}

ensure_watcher

if [[ "$MODE" == "start" ]]; then
  rm -f "$PENDING"
  [[ -f "$WATCH_FILE" ]] && cp -a "$WATCH_FILE" "$LAST"
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
