#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=scripts/e2e_assert.sh
source "$ROOT/scripts/e2e_assert.sh"
trap e2e_stop_session EXIT
e2e_start_session gpuprobe
sleep 3
e2e_send "home"
sleep 6
e2e_send "capture probe.png"
e2e_wait_file "$E2E_OUT/probe.png" 30
identify -format 'full mean=%[mean] stdev=%[standard-deviation]\n' "$E2E_OUT/probe.png"
convert "$E2E_OUT/probe.png" -gravity Center -crop 400x300+0+0 +repage -format 'center mean=%[mean] stdev=%[standard-deviation]\n' info:
echo "OUT=$E2E_OUT"
tail -50 "$E2E_PREFIX/xvfb.log" || true
cp -f "$E2E_OUT/probe.png" /tmp/cz_e2e_visual_review/probe.png
