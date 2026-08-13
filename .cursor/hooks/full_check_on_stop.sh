#!/usr/bin/env bash
# After an agent turn: run scripts/full_check.sh if the turn touched
# testable files; follow up if red.
set -u
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT" || { echo '{}'; exit 0; }
LOG="${CZ_FULL_CHECK_LOG:-/tmp/cz_full_check.log}"
INPUT="$(cat || true)"
export INPUT
python3 - <<'PY' >/tmp/cz_full_check_stop_meta.json 2>/dev/null || true
import json, os
raw = os.environ.get("INPUT", "")
try:
    data = json.loads(raw) if raw.strip() else {}
except Exception:
    data = {}
status = str(data.get("status") or "")
try:
    loop = int(data.get("loop_count") or 0)
except Exception:
    loop = 0
print(json.dumps({"status": status, "loop_count": loop}))
PY

STATUS=""
LOOP=0
if [[ -f /tmp/cz_full_check_stop_meta.json ]]; then
  STATUS="$(python3 -c 'import json; print(json.load(open("/tmp/cz_full_check_stop_meta.json")).get("status",""))' 2>/dev/null || true)"
  LOOP="$(python3 -c 'import json; print(json.load(open("/tmp/cz_full_check_stop_meta.json")).get("loop_count",0))' 2>/dev/null || true)"
fi

emit() {
  python3 -c 'import json,sys; print(json.dumps({"followup_message": sys.argv[1]}))' "$1"
}

if [[ "${CZ_FULL_CHECK:-1}" == "0" ]]; then
  echo '{}'
  exit 0
fi
if [[ "$STATUS" == "aborted" ]]; then
  echo '{}'
  exit 0
fi
if [[ "${LOOP:-0}" -ge 3 ]]; then
  echo '{}'
  exit 0
fi

CHANGED="$(
  git diff --name-only HEAD 2>/dev/null
  git diff --name-only --cached 2>/dev/null
  git ls-files --others --exclude-standard 2>/dev/null
)"
RELEVANT="$(printf '%s\n' "$CHANGED" | grep -E '^(src/|benches/|tests/|fuzz/|Cargo\.(toml|lock)|build\.rs|docs/assistant/tracey/|scripts/full_check)' || true)"
if [[ -z "$RELEVANT" && ! -f /tmp/cz_full_check_last_fail ]]; then
  echo '{}'
  exit 0
fi

if ! "$ROOT/scripts/full_check.sh"; then
  TAIL="$(cat /tmp/cz_full_check_last_fail_excerpt 2>/dev/null || true)"
  if [[ -z "$TAIL" ]]; then
    TAIL="$(tail -c 6000 "$LOG" 2>/dev/null || echo '(no full_check log)')"
  fi
  MSG="$(printf '%s\n' \
    "scripts/full_check.sh failed. Do not ignore or soft-skip. Fix it, then stop so this hook re-runs." \
    "Full log: $LOG" \
    "--- tail ---" \
    "$TAIL")"
  emit "$MSG"
  exit 0
fi
echo '{}'
exit 0
