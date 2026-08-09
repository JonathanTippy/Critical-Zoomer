#!/usr/bin/env bash
# Stop agent /loop sleepers for this workspace (approval-free path).
# Allowlisted by .cursor/hooks/guard-raw-kill.sh — do not use raw kill/pkill.
#
# Targets only cmdlines that look like Cursor agent loop wakes/ticks:
#   AGENT_LOOP_TICK_* / AGENT_LOOP_WAKE_* / while+sleep+AGENT_LOOP_
# Never touches cursorsandbox parents, headed apps, or test binaries
# (those stay under kill-test-zombies.sh).
set -u
LOG="${CZ_ZOMBIE_KILL_LOG:-/tmp/cz_zombie_kill.log}"
ROOT="$(git -C "$(dirname "$0")/../.." rev-parse --show-toplevel 2>/dev/null || true)"
if [[ -z "${ROOT:-}" ]]; then
  ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
fi

ts() { date -Iseconds 2>/dev/null || date; }
log() { echo "$(ts) stop-agent-loops: $*" >>"$LOG" 2>/dev/null || true; }

term_kill() {
  local pid="$1"
  kill -TERM "$pid" 2>/dev/null || return 0
  local i
  for i in 1 2 3 4 5; do
    kill -0 "$pid" 2>/dev/null || return 0
    sleep 0.1
  done
  kill -KILL "$pid" 2>/dev/null || true
}

is_agent_loop_cmd() {
  local cmd="$1"
  case "$cmd" in
    *stop-agent-loops.sh* | *kill-test-zombies.sh*) return 1 ;;
  esac
  # Include Cursor sandbox wrappers whose argv is an agent loop (otherwise the
  # outer cursorsandbox keeps the sleeper alive after the inner bash dies).
  case "$cmd" in
    *AGENT_LOOP_TICK_* | *AGENT_LOOP_WAKE_* ) return 0 ;;
    *while*true*sleep*AGENT_LOOP_*) return 0 ;;
  esac
  return 1
}

n=0
while read -r pid; do
  [[ -n "$pid" ]] || continue
  [[ "$pid" =~ ^[0-9]+$ ]] || continue
  cmd="$(tr '\0' ' ' <"/proc/$pid/cmdline" 2>/dev/null || true)"
  [[ -n "$cmd" ]] || continue
  if is_agent_loop_cmd "$cmd"; then
    log "KILL pid=$pid cmd=$cmd"
    term_kill "$pid"
    n=$((n + 1))
  fi
done < <(pgrep -f 'AGENT_LOOP_(TICK|WAKE)_|while true; do[[:space:]]*sleep' 2>/dev/null || true)

# Also stop orphan `sleep N` children whose parent was the loop bash (best-effort):
# if cmdline is exactly sleep with a long delay and PPID cmdline was AGENT_LOOP.
while read -r pid; do
  [[ -n "$pid" ]] || continue
  cmd="$(tr '\0' ' ' <"/proc/$pid/cmdline" 2>/dev/null || true)"
  case "$cmd" in
    sleep\ 6[0-9][0-9]* | sleep\ [1-9][0-9][0-9][0-9]*)
      ppid="$(awk '/^PPid:/{print $2}' "/proc/$pid/status" 2>/dev/null || true)"
      [[ -n "$ppid" ]] || continue
      pcmd="$(tr '\0' ' ' <"/proc/$ppid/cmdline" 2>/dev/null || true)"
      if is_agent_loop_cmd "$pcmd" || [[ "$pcmd" == *AGENT_LOOP_* ]]; then
        log "KILL sleep-child pid=$pid ppid=$ppid"
        term_kill "$pid"
        n=$((n + 1))
      fi
      ;;
  esac
done < <(pgrep -x sleep 2>/dev/null || true)

echo "stop-agent-loops: stopped≈$n (log $LOG)"
exit 0
