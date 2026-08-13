#!/usr/bin/env bash
# Region coverage for QC. Ignores GUI/actor shells (headed e2e covers those).
# Skips 15s/60s pyramid tiers (llvm-cov is for unit region coverage).
# Usage: taskset -c 3-8 scripts/coverage.sh
#
# Living artifact (commit this summary): docs/assistant/coverage-baseline.txt
# HTML report (local only): target/llvm-cov/html
# Historical copies may also land under docs/assistant/Trash/.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
NPROC="$(nproc)"
START=$((NPROC / 4))
END=$((NPROC * 3 / 4 - 1))
CZ_CPUSET="${CZ_CPUSET:-${START}-${END}}"
IGNORE='(main\.rs|settings\.rs|window/mod\.rs|widgetize\.rs)'
OUT_DIR="${ROOT}/target/llvm-cov/html"
mkdir -p "$(dirname "$OUT_DIR")"
LIVING="${ROOT}/docs/assistant/coverage-baseline.txt"
TRASH_COPY="${ROOT}/docs/assistant/Trash/coverage-baseline.txt"
mkdir -p "$(dirname "$TRASH_COPY")"

# Dedicated llvm-cov target dir (ignore ambient CARGO_TARGET_DIR from callers).
export CARGO_TARGET_DIR="${CZ_LLVM_COV_TARGET:-${ROOT}/target/llvm-cov-target}"
mkdir -p "$CARGO_TARGET_DIR"
# cargo-llvm-cov may refuse to clean without this tag.
if [[ ! -f "$CARGO_TARGET_DIR/CACHEDIR.TAG" ]]; then
  printf 'Signature: 8a477f597d28d172789f06886806bc55\n# cargo-llvm-cov target\n' >"$CARGO_TARGET_DIR/CACHEDIR.TAG"
fi
{
  echo "coverage: living baseline $(date -Iseconds)"
  echo "coverage: cpuset=${CZ_CPUSET} target=${CARGO_TARGET_DIR}"
  echo "coverage: tip=$(git -C "$ROOT" rev-parse --short HEAD 2>/dev/null || echo unknown)"
} | tee "$LIVING"
# --html already prints a text summary; --summary-only may not combine with --html.
taskset -c "${CZ_CPUSET}" cargo llvm-cov --lib \
  --html --output-dir "$OUT_DIR" \
  --ignore-filename-regex="$IGNORE" \
  -- \
  --test-threads=1 \
  --skip integration_tier \
  --skip e2e_tier \
  2>&1 | tee -a "$LIVING"

echo "HTML: ${OUT_DIR}/index.html" | tee -a "$LIVING"
# Extract Totals from HTML into the living summary (V2V Coverage cite).
HTML_INDEX="${OUT_DIR}/html/index.html"
if [[ ! -f "$HTML_INDEX" ]]; then
  HTML_INDEX="${OUT_DIR}/index.html"
fi
if [[ -f "$HTML_INDEX" ]]; then
  python3 - <<PY | tee -a "$LIVING"
import re
from pathlib import Path
html = Path(r"""$HTML_INDEX""").read_text(errors="ignore")
m = re.search(
    r"Totals</pre></td><td[^>]*>.*?<pre>\s*([\d.]+)%\s*\((\d+)/(\d+)\)</pre>.*?"
    r"<pre>\s*([\d.]+)%\s*\((\d+)/(\d+)\)</pre>.*?"
    r"<pre>\s*([\d.]+)%\s*\((\d+)/(\d+)\)</pre>",
    html,
    re.S,
)
if not m:
    print("coverage: TOTAL parse failed — open HTML for numbers")
else:
    print(
        f"TOTAL function {m.group(1)}% ({m.group(2)}/{m.group(3)}); "
        f"line {m.group(4)}% ({m.group(5)}/{m.group(6)}); "
        f"region {m.group(7)}% ({m.group(8)}/{m.group(9)})"
    )
PY
fi
cp -f "$LIVING" "$TRASH_COPY"
