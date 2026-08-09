#!/usr/bin/env bash
# Home fill fitness gates (generous ceilings — product is much faster).
# r[verify cz.e2e.fill-first-tile-1s+1]
# r[verify cz.e2e.fill-all-tiles-10s+1]
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=scripts/e2e_assert.sh
source "$ROOT/scripts/e2e_assert.sh"

trap e2e_stop_session EXIT
e2e_start_session home_fill_fitness || exit 1

e2e_send "home"
t0_ms=$(date +%s%3N)
first_tile_ms=""
all_tiles_ms=""
deadline_all_ms=$((t0_ms + 10000))
deadline_first_ms=$((t0_ms + 1000))

while true; do
  now_ms=$(date +%s%3N)
  if [ "$now_ms" -ge "$deadline_all_ms" ]; then
    break
  fi
  rm -f "$E2E_OUT/fitness_probe.png"
  e2e_send "capture fitness_probe.png"
  e2e_wait_file "$E2E_OUT/fitness_probe.png" 20 || continue
  holes_n=$(taskset -c 0-3 bash -c "source \"$ROOT/scripts/e2e_assert.sh\"; e2e_count_gray_holes \"$E2E_OUT/fitness_probe.png\"")
  stdev=$(e2e_stdev "$E2E_OUT/fitness_probe.png" || echo 0)
  echo "fitness_probe holes=$holes_n stdev=$stdev elapsed=$((now_ms - t0_ms))ms"
    # First visible tile: structured frame (not flat NORES grey).
  if [ -z "$first_tile_ms" ] && [ "${stdev:-0}" -ge 3000 ]; then
    first_tile_ms=$((now_ms - t0_ms))
    e2e_pass "first tile visible ${first_tile_ms}ms (<=1000 intent)"
  fi
  # All tiles: no NORES-grey holes in the fitness crop.
  if [ "${holes_n:-99}" -le 0 ]; then
    all_tiles_ms=$((now_ms - t0_ms))
    e2e_pass "all tiles done ${all_tiles_ms}ms (<=10000 intent)"
    break
  fi
  if [ "$now_ms" -lt "$deadline_first_ms" ]; then
    sleep 0.08 || true
  else
    sleep 0.15 || true
  fi
done

if [ -z "$first_tile_ms" ]; then
  e2e_fail_msg "no structured tile within 1s of startup (fitness)"
fi
if [ -z "$all_tiles_ms" ]; then
  holes_final=$(taskset -c 0-3 bash -c "source \"$ROOT/scripts/e2e_assert.sh\"; e2e_count_gray_holes \"$E2E_OUT/fitness_probe.png\"")
  e2e_fail_msg "tiles incomplete after 10s (holes=${holes_final:-?})"
fi
if [ -n "$first_tile_ms" ] && [ "$first_tile_ms" -gt 1000 ]; then
  e2e_fail_msg "first tile ${first_tile_ms}ms (>1000 fitness ceiling)"
fi
if [ -n "$all_tiles_ms" ] && [ "$all_tiles_ms" -gt 10000 ]; then
  e2e_fail_msg "all tiles ${all_tiles_ms}ms (>10000 fitness ceiling)"
fi

e2e_exit
