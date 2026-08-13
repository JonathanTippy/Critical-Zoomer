#!/usr/bin/env bash
# Scoped cargo-mutants. Excludes main/GUI shells; skips slow release hard-bars.
# Usage: taskset -c 3-8 scripts/mutants.sh [file-glob...]
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
NPROC="$(nproc)"
START=$((NPROC / 4))
END=$((NPROC * 3 / 4 - 1))
CZ_CPUSET="${CZ_CPUSET:-${START}-${END}}"

FILES=(
  "src/intexp.rs"
  "src/range.rs"
  "src/floatexp.rs"
  "src/assemblies/workgroup/tile_manager.rs"
  "src/assemblies/workgroup/tile_publisher.rs"
)
if [[ $# -gt 0 ]]; then
  FILES=("$@")
fi

FILE_ARGS=()
for f in "${FILES[@]}"; do
  FILE_ARGS+=(--file "$f")
done

echo "mutants: cpuset=${CZ_CPUSET} files=${FILES[*]}"
# Pass test filters after `--` so each mutant runs a fast lib subset
# (unit tier only; do not skip `gpu_ips` — that name matches the live IPS probe).
exec taskset -c "${CZ_CPUSET}" cargo mutants \
  --exclude 'src/main.rs' \
  --exclude 'src/assemblies/headgroup/window/mod.rs' \
  --exclude 'src/settings.rs' \
  "${FILE_ARGS[@]}" \
  --timeout 120 \
  --jobs 1 \
  -- \
  --lib \
  -- \
  --test-threads=1 \
  --skip integration_tier \
  --skip e2e_tier
