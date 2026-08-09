#!/usr/bin/env bash
# Count r[verify …] tags per id and flag those with fewer than 3.
# Soft-skip GPU early-returns are not detected here — spot-check GPU rows manually.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
tmp="$(mktemp)"
grep -rhoE 'r\[verify [^]]+\]' src/ | sed 's/^r\[verify //;s/\]$//' | sort | uniq -c | sort -k2 >"$tmp"
echo "=== verify counts (<3 flagged) ==="
awk '{
  count=$1; $1=""; id=substr($0,2);
  flag=(count<3)?"  << UNDER":"";
  printf "%4d  %s%s\n", count, id, flag
}' "$tmp"
echo
echo "=== decision/REQ comment markers (informational) ==="
grep -rhoE '// (D-[A-Z0-9-]+|REQ-[A-Z0-9-]+)' src/ 2>/dev/null \
  | sed 's|^// ||' | sort | uniq -c | sort -rn | head -60 || true
rm -f "$tmp"
