#!/usr/bin/env bash
# Reap Critical-Zoomer *test* leftovers only. Fail open. Idempotent.
# workgroup_fitness is not reaped unless CZ_REAP_BENCH=1 (live Criterion).
#
# Safe targets (path-scoped to this repo's target/ or cz session dirs):
#   - target/*/critical_zoomer
#   - target/*/deps/workgroup_fitness-*
#   - screenshot_session under /tmp/cz_* (via scripts/screenshot_session.sh stop)
#   - Xvfb / xvfb-run clearly tied to those sessions
#
# Never touches: /usr/bin/critical_zoomer, or Cursor cursorsandbox parents.
# Headed `target/` / `/tmp/cz_*` sessions **are** reaped (including a
# developer-headed test of the repo binary). That is accepted: slightly
# annoying, but it keeps the assistant from confusing leftover app
# processes with a live headed session. Do not "fix" by sparing headed
# repo binaries.
#
# Usage:
#   .cursor/hooks/kill-test-zombies.sh              # CLI
#   .cursor/hooks/kill-test-zombies.sh --hook-before
#   .cursor/hooks/kill-test-zombies.sh --hook-after
#   .cursor/hooks/kill-test-zombies.sh --hook-stop
set -u
LOG="${CZ_ZOMBIE_KILL_LOG:-/tmp/cz_zombie_kill.log}"
MODE="cli"
case "${1:-}" in
  --hook-before) MODE="before" ;;
  --hook-after) MODE="after" ;;
  --hook-stop) MODE="stop" ;;
  -h|--help)
    sed -n '2,20p' "$0"
    exit 0
    ;;
esac

# Consume hook stdin (JSON). Keep a copy of the command string when present.
HOOK_CMD=""
if [[ "$MODE" != "cli" ]]; then
  HOOK_INPUT="$(cat || true)"
  HOOK_CMD="$(
    HOOK_INPUT="$HOOK_INPUT" python3 - <<'PY' 2>/dev/null || true
import json, os
raw = os.environ.get("HOOK_INPUT", "")
try:
    data = json.loads(raw) if raw.strip() else {}
except Exception:
    data = {}
for k in ("command", "shell_command", "cmd"):
    v = data.get(k)
    if isinstance(v, str) and v.strip():
        print(v)
        break
PY
  )"
fi

ROOT="$(git -C "$(dirname "$0")/../.." rev-parse --show-toplevel 2>/dev/null || true)"
if [[ -z "${ROOT:-}" ]]; then
  ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
fi

ts() { date -Iseconds 2>/dev/null || date; }
log() { echo "$(ts) $*" >>"$LOG" 2>/dev/null || true; }

# Return 0 if cmdline looks like a repo test binary we own.
is_repo_test_cmd() {
  local cmd="$1"
  case "$cmd" in
    *"$ROOT/target/"*critical_zoomer* | *"$ROOT/target/"*workgroup_fitness*)
      return 0
      ;;
  esac
  # Relative cargo-run paths when cwd is the repo
  case "$cmd" in
    *target/release/critical_zoomer* | *target/debug/critical_zoomer* | \
    *target/release/deps/workgroup_fitness* | *target/debug/deps/workgroup_fitness*)
      return 0
      ;;
  esac
  return 1
}

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

reap_matching_pids() {
  local label="$1"
  shift
  local pat pid cmd
  for pat in "$@"; do
    while read -r pid; do
      [[ -n "$pid" ]] || continue
      [[ "$pid" =~ ^[0-9]+$ ]] || continue
      cmd="$(tr '\0' ' ' <"/proc/$pid/cmdline" 2>/dev/null || true)"
      [[ -n "$cmd" ]] || continue
      # Never touch Cursor sandbox helpers or this script.
      case "$cmd" in
        *cursorsandbox* | *kill-test-zombies.sh*) continue ;;
      esac
      if is_repo_test_cmd "$cmd"; then
        log "KILL $label pid=$pid cmd=$cmd"
        term_kill "$pid"
      fi
    done < <(pgrep -f "$pat" 2>/dev/null || true)
  done
}

stop_cz_sessions() {
  local ctl="$ROOT/scripts/screenshot_session.sh"
  [[ -x "$ctl" ]] || return 0
  # Default harness session (screenshot_check without CZ_SESSION_PREFIX).
  "$ctl" stop >>"$LOG" 2>&1 || true
  local d
  for d in /tmp/cz_*; do
    [[ -d "$d" ]] || continue
    if [[ -f "$d/app.pid" || -f "$d/ctl.pid" || -f "$d/daemon.pid" || -f "$d/xvfb_wrapper.pid" ]]; then
      log "screenshot_session stop session=$d"
      CZ_SESSION_PREFIX="$d" "$ctl" stop >>"$LOG" 2>&1 || true
    fi
  done
}

reap_session_xvfb() {
  # Only real Xvfb/xvfb-run that reference a /tmp/cz_ path — never Cursor
  # sandbox wrappers whose argv merely *mentions* those strings.
  local pid cmd base
  while read -r pid; do
    [[ -n "$pid" ]] || continue
    cmd="$(tr '\0' ' ' <"/proc/$pid/cmdline" 2>/dev/null || true)"
    [[ -n "$cmd" ]] || continue
    case "$cmd" in
      *cursorsandbox* | *kill-test-zombies.sh*) continue ;;
    esac
    base="$(basename "$(readlink -f "/proc/$pid/exe" 2>/dev/null || true)" 2>/dev/null || true)"
    case "$base" in
      Xvfb|xvfb-run) ;;
      *) continue ;;
    esac
    case "$cmd" in
      */tmp/cz_*)
        log "KILL xvfb-ish pid=$pid cmd=$cmd"
        term_kill "$pid"
        ;;
    esac
  done < <(pgrep -f 'Xvfb|xvfb-run' 2>/dev/null || true)
}

run_cleanup() {
  log "BEGIN mode=$MODE root=$ROOT hook_cmd=${HOOK_CMD:0:120}"
  stop_cz_sessions
  reap_matching_pids "app" \
    "$ROOT/target/.*/critical_zoomer" \
    'target/release/critical_zoomer' \
    'target/debug/critical_zoomer'
  # Never auto-reap workgroup_fitness: hooks and full_check race live
  # Criterion (1080p especially). Leftovers: CZ_REAP_BENCH=1 this script.
  if [[ "$MODE" == "cli" && "${CZ_REAP_BENCH:-0}" == "1" ]]; then
    reap_matching_pids "bench" \
      "$ROOT/target/.*/workgroup_fitness" \
      'target/release/deps/workgroup_fitness' \
      'target/debug/deps/workgroup_fitness'
  fi
  reap_session_xvfb
  log "END mode=$MODE"
}

# Fail open: never block the agent on cleanup errors.
run_cleanup || true

case "$MODE" in
  before)
    printf '%s\n' '{"permission":"allow"}'
    ;;
  after|stop)
    # Surface a short note when something was logged this run (best-effort).
    printf '%s\n' '{}'
    ;;
  cli)
    echo "kill-test-zombies: done (log $LOG)"
    ;;
esac
exit 0
