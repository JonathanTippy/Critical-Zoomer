#!/usr/bin/env bash
# Intercept raw kill/pkill aimed at test leftovers so Auto-review never sees them.
# Runs the safe reaper, then denies the raw command (fail closed for that cmd only).
set -u
INPUT="$(cat || true)"
export INPUT
CMD="$(
  python3 - <<'PY' 2>/dev/null || true
import json, os
raw = os.environ.get("INPUT", "")
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

# Always allow the approved reaper (and only that kill path).
if [[ "$CMD" == *kill-test-zombies.sh* ]]; then
  printf '%s\n' '{"permission":"allow"}'
  exit 0
fi

TARGETED=0
if echo "$CMD" | grep -Eqi '(^|[[:space:];|&])(pkill|killall)([[:space:]]|$)'; then
  TARGETED=1
fi
if echo "$CMD" | grep -Eqi '(critical_zoomer|workgroup_fitness|Xvfb|xvfb-run|/tmp/cz_)'; then
  if echo "$CMD" | grep -Eqi '(^|[[:space:];|&])(kill|pkill|killall)([[:space:]]|$)'; then
    TARGETED=1
  fi
fi

if [[ "$TARGETED" -eq 0 ]]; then
  printf '%s\n' '{"permission":"allow"}'
  exit 0
fi

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
"$ROOT/.cursor/hooks/kill-test-zombies.sh" >/dev/null 2>&1 || true

python3 - <<'PY'
import json
msg = (
    "Raw kill/pkill for test leftovers is blocked. Cleanup already ran via "
    ".cursor/hooks/kill-test-zombies.sh. Do not retry kill/pkill; hooks reap "
    "before/after cargo test|bench|xvfb_screenshot_check and on agent stop. "
    "Manual sweep: .cursor/hooks/kill-test-zombies.sh only."
)
print(json.dumps({
    "permission": "deny",
    "user_message": "Blocked raw kill/pkill; safe reaper already ran.",
    "agent_message": msg,
}))
PY
exit 0
