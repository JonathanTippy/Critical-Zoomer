#!/usr/bin/env bash
# Fail-open pre/post-edit reminder for workgroup/colorer edits.
# Injects a short invariant blurb as additional_context when the edited path
# matches screen_worker/ or colorer/. Never blocks the edit.
set -u
INPUT="$(cat || true)"
RESULT="$(
  INPUT="$INPUT" python3 - <<'PY' 2>/dev/null || true
import json, os, re
raw = os.environ.get("INPUT", "")
try:
    data = json.loads(raw) if raw.strip() else {}
except Exception:
    data = {}

def path_from(d):
    for k in ("file_path", "path", "filePath"):
        v = d.get(k)
        if isinstance(v, str) and v.strip():
            return v
    inp = d.get("input") or d.get("tool_input") or {}
    if isinstance(inp, dict):
        for k in ("path", "file_path", "filePath", "target_notebook"):
            v = inp.get(k)
            if isinstance(v, str) and v.strip():
                return v
    return ""

path = path_from(data)
if not re.search(r"(screen_worker|/colorer/)", path.replace("\\", "/")):
    print("{}")
    raise SystemExit(0)

blurb = (
    "Workgroup/colorer edit — keep v0.0.9 invariants: BoutCap (no unbounded call), "
    "one LiveTarget, whole-snapshot publishes, pivot flush-then-announce, "
    "Delivery provisional≠final, small interruptible bouts. "
    "See docs/assistant/design/workgroup-virtues.md and "
    "docs/assistant/tracey/craftsmanship-rules.md. Prefer types over comments."
)
print(json.dumps({"additional_context": blurb}))
PY
)"
if [[ -z "${RESULT:-}" ]]; then
  RESULT='{}'
fi
printf '%s\n' "$RESULT"
exit 0
