#!/usr/bin/env bash
# Scoped cargo-mutants. Excludes main/GUI shells; unit-tier tests only.
# Usage: taskset -c 3-8 scripts/mutants.sh [file-glob...]
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
NPROC="$(nproc)"
START=$((NPROC / 4))
END=$((NPROC * 3 / 4 - 1))
CZ_CPUSET="${CZ_CPUSET:-${START}-${END}}"

FILES=(
  "src/utils.rs"
  "src/range.rs"
  "src/floatexp.rs"
)
if [[ $# -gt 0 ]]; then
  FILES=("$@")
fi

FILE_ARGS=()
for f in "${FILES[@]}"; do
  FILE_ARGS+=(--file "$f")
done

echo "mutants: cpuset=${CZ_CPUSET} files=${FILES[*]}"
# Test filter is in .cargo/mutants.toml (`--lib`, skip 15s/60s tiers).
exec taskset -c "${CZ_CPUSET}" cargo mutants \
  --exclude 'src/main.rs' \
  --exclude 'src/assemblies/headgroup/window/mod.rs' \
  --exclude 'src/settings.rs' \
  "${FILE_ARGS[@]}" \
  --timeout 120 \
  --jobs 1
