#!/usr/bin/env bash
# Zoomer-groove stop hook (skill § Procedure).
# Parks → scripts/zoomer_groove_check.sh (full standard, with screenshot).
# Error → followup to fix. Success → silent except manual screenshot followup.
set -u
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT" || { echo '{}'; exit 0; }
LOG="${CZ_GROOVE_CHECK_LOG:-/tmp/cz_groove_check.log}"
SHOT="${CZ_GROOVE_SCREENSHOT_OUT:-/tmp/cz_groove_screenshot}/home_final.png"
INPUT="$(cat || true)"
export INPUT
python3 - <<'PY' >/tmp/cz_groove_check_stop_meta.json 2>/dev/null || true
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
if [[ -f /tmp/cz_groove_check_stop_meta.json ]]; then
  STATUS="$(python3 -c 'import json; print(json.load(open("/tmp/cz_groove_check_stop_meta.json")).get("status",""))' 2>/dev/null || true)"
  LOOP="$(python3 -c 'import json; print(json.load(open("/tmp/cz_groove_check_stop_meta.json")).get("loop_count",0))' 2>/dev/null || true)"
fi

emit() {
  python3 -c 'import json,sys; print(json.dumps({"followup_message": sys.argv[1]}))' "$1"
}

if [[ "${CZ_GROOVE_CHECK:-1}" == "0" ]]; then
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
RELEVANT="$(printf '%s\n' "$CHANGED" | grep -E '^(src/|benches/|tests/|fuzz/|Cargo\.(toml|lock)|build\.rs|docs/assistant/tracey/|scripts/)' || true)"
if [[ -z "$RELEVANT" && ! -f /tmp/cz_groove_check_last_fail ]]; then
  echo '{}'
  exit 0
fi

if ! OUT="$("$ROOT/scripts/zoomer_groove_check.sh" 2>&1)"; then
  BODY="$(cat "${CZ_GROOVE_CHECK_EXCERPT:-/tmp/cz_groove_check_last_fail_excerpt}" 2>/dev/null || true)"
  if [[ -z "$BODY" ]]; then
    BODY="$OUT"
  fi
  MSG="zoomer_groove_check failed. Fix it, then stop so this hook re-runs.

$BODY"
  emit "$MSG"
  exit 0
fi

if [[ ! -f "$SHOT" ]]; then
  emit "zoomer_groove_check passed automated steps but manual screenshot missing at $SHOT — fix screenshot_check, then stop."
  exit 0
fi

emit "zoomer_groove_check passed. Screenshot: $SHOT — does this look right? Read and inspect the image directly."
echo '{}'
exit 0
