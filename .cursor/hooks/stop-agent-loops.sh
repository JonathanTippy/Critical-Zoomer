#!/usr/bin/env bash
# Pause agent /loop sleepers for this workspace (approval-free path).
#   stop-agent-loops.sh              — agent loops + stale JFT wake tails
#   stop-agent-loops.sh --loops-only — agent loops only
#   stop-agent-loops.sh --wake-only  — stale JFT wake tails only
# Allowlisted by .cursor/hooks/guard-raw-kill.sh — do not use raw kill/pkill.
#
# Targets only cmdlines that look like Cursor agent loop wakes/ticks:
#   AGENT_LOOP_TICK_* / AGENT_LOOP_WAKE_* / while+sleep+AGENT_LOOP_
# Also stops the outer cursorsandbox wrapper when its argv is that loop.
# Never touches headed apps or test binaries (those stay under kill-test-zombies.sh).
set -u
MODE="${1:-all}"
case "$MODE" in
  --loops-only | --wake-only | all) ;;
  *) MODE="all" ;;
esac
LOG="${CZ_ZOMBIE_KILL_LOG:-/tmp/cz_zombie_kill.log}"
ROOT="$(git -C "$(dirname "$0")/../.." rev-parse --show-toplevel 2>/dev/null || true)"
if [[ -z "${ROOT:-}" ]]; then
  ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
fi

ts() { date -Iseconds 2>/dev/null || date; }
log() { echo "$(ts) stop-agent-loops: $*" >>"$LOG" 2>/dev/null || true; }

term_kill() {
  local pid="$1"
  if ! kill -TERM "$pid" 2>/dev/null; then
    log "TERM failed pid=$pid (EPERM/ESRCH?)"
    return 1
  fi
  local i
  for i in 1 2 3 4 5; do
    kill -0 "$pid" 2>/dev/null || return 0
    sleep 0.1
  done
  if ! kill -KILL "$pid" 2>/dev/null; then
    log "KILL failed pid=$pid"
    return 1
  fi
  return 0
}

is_agent_loop_cmd() {
  local cmd="$1"
  # Live sleeper first — loop prompts often *mention* stop-agent-loops.sh in JSON.
  case "$cmd" in
    *while*true*sleep*AGENT_LOOP_TICK_* | *while*true*sleep*AGENT_LOOP_WAKE_* | \
    *while*true*sleep*AGENT_LOOP_*)
      return 0
      ;;
  esac
  case "$cmd" in
    *stop-agent-loops.sh* | *kill-test-zombies.sh*) return 1 ;;
  esac
  # Include Cursor sandbox wrappers whose argv is an agent loop (otherwise the
  # outer cursorsandbox keeps the sleeper alive after the inner bash dies).
  case "$cmd" in
    *AGENT_LOOP_TICK_* | *AGENT_LOOP_WAKE_* ) return 0 ;;
  esac
  return 1
}

n=0
if [[ "$MODE" != "--wake-only" ]]; then
# Scan /proc directly — pgrep -f can miss very long Cursor wrapper cmdlines.
for cmdline in /proc/[0-9]*/cmdline; do
  pid="${cmdline%/cmdline}"
  pid="${pid#/proc/}"
  [[ "$pid" =~ ^[0-9]+$ ]] || continue
  # Skip self and parents of this stop script.
  [[ "$pid" == "$$" || "$pid" == "$PPID" ]] && continue
  [[ -r "$cmdline" ]] || continue
  cmd="$(tr '\0' ' ' <"$cmdline" 2>/dev/null || true)"
  [[ -n "$cmd" ]] || continue
  if is_agent_loop_cmd "$cmd"; then
    log "KILL pid=$pid cmd=${cmd:0:240}"
    if term_kill "$pid"; then
      n=$((n + 1))
    fi
  fi
done
fi

if [[ "$MODE" != "--loops-only" ]]; then
# Also stop orphan `sleep N` children whose parent was the loop bash (best-effort).
for cmdline in /proc/[0-9]*/cmdline; do
  pid="${cmdline%/cmdline}"
  pid="${pid#/proc/}"
  [[ "$pid" =~ ^[0-9]+$ ]] || continue
  [[ -r "$cmdline" ]] || continue
  cmd="$(tr '\0' ' ' <"$cmdline" 2>/dev/null || true)"
  [[ -n "$cmd" ]] || continue
  case "$cmd" in
    sleep\ 6[0-9][0-9]* | sleep\ [1-9][0-9][0-9][0-9]*)
      ppid="$(awk '/^PPid:/{print $2}' "/proc/$pid/status" 2>/dev/null || true)"
      [[ -n "$ppid" ]] || continue
      pcmd="$(tr '\0' ' ' <"/proc/$ppid/cmdline" 2>/dev/null || true)"
      if is_agent_loop_cmd "$pcmd" || [[ "$pcmd" == *while*true*sleep*AGENT_LOOP_* ]]; then
        log "KILL sleep-child pid=$pid ppid=$ppid"
        if term_kill "$pid"; then
          n=$((n + 1))
        fi
      fi
      ;;
  esac
done
fi

is_wake_follower_cmd() {
  local cmd="$1"
  case "$cmd" in
    *jft-agents-md.wake* | *jft-debugging.wake* | *jft-zoomer-groove.wake*)
      return 0
      ;;
  esac
  return 1
}

n_wake=0
if [[ "$MODE" != "--loops-only" ]]; then
for cmdline in /proc/[0-9]*/cmdline; do
  pid="${cmdline%/cmdline}"
  pid="${pid#/proc/}"
  [[ "$pid" =~ ^[0-9]+$ ]] || continue
  [[ "$pid" == "$$" || "$pid" == "$PPID" ]] && continue
  [[ -r "$cmdline" ]] || continue
  cmd="$(tr '\0' ' ' <"$cmdline" 2>/dev/null || true)"
  [[ -n "$cmd" ]] || continue
  if is_wake_follower_cmd "$cmd"; then
    log "KILL wake-follower pid=$pid cmd=${cmd:0:240}"
    if term_kill "$pid"; then
      n_wake=$((n_wake + 1))
    fi
  fi
done
fi

echo "stop-agent-loops: stopped≈$n (wake-followers≈$n_wake) mode=$MODE (log $LOG)"
exit 0
