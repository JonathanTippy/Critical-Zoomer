#!/usr/bin/env bash
# Intercept raw kill/pkill/killall so Auto-review never sees them.
# Runs the safe reaper, then denies the raw command.
# Only `.cursor/hooks/kill-test-zombies.sh` is an allowed kill path.
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

# Approved reaper only.
if [[ "$CMD" == *kill-test-zombies.sh* ]]; then
  printf '%s\n' '{"permission":"allow"}'
  exit 0
fi

# Any raw process-signal command → reap via script, then deny.
if ! echo "$CMD" | grep -Eqi '(^|[[:space:];|&])(kill|pkill|killall)([[:space:]]|$)'; then
  printf '%s\n' '{"permission":"allow"}'
  exit 0
fi

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
"$ROOT/.cursor/hooks/kill-test-zombies.sh" >/dev/null 2>&1 || true

python3 - <<'PY'
import json
msg = (
    "Raw kill/pkill/killall is blocked in this repo so Auto-review is never "
    "prompted. Cleanup already ran via .cursor/hooks/kill-test-zombies.sh. "
    "Hooks also reap before/after cargo test|bench|xvfb_screenshot_check and "
    "on agent stop. Manual sweep: .cursor/hooks/kill-test-zombies.sh only."
)
print(json.dumps({
    "permission": "deny",
    "user_message": "Blocked raw kill/pkill; safe reaper already ran.",
    "agent_message": msg,
}))
PY
exit 0
