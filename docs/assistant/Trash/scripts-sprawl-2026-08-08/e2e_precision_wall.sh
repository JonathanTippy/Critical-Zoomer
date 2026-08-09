#!/usr/bin/env bash
# Precision wall: seahorse pot 19 must show filament structure (not a flat square slab),
# and zoom-out / mid-goto must still recover (hangup regression).
# Usage: taskset -c 3-8 scripts/e2e_precision_wall.sh [/tmp/cz_prec_wall]
# r[verify cz.int.session-pipeline+1]
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=scripts/e2e_assert.sh
source "$ROOT/scripts/e2e_assert.sh"

REVIEW_DIR="${1:-/tmp/cz_prec_wall}"
mkdir -p "$REVIEW_DIR"
SEAHORSE_RE="-0.743643887037151"
SEAHORSE_IM="0.131825904205216"
# Flat grey slab has near-zero center contrast; real seahorse filaments are high.
DEEP_CENTER_STDEV_FLOOR="${CZ_PREC_CENTER_STDEV:-1200}"
DEEP_STDEV_FLOOR="${CZ_PREC_DEEP_STDEV:-400}"
HOLD_SECS="${CZ_PREC_HOLD_SECS:-4}"

export CZ_GOTO="$SEAHORSE_RE $SEAHORSE_IM 19"
export CZ_CPUSET="${CZ_CPUSET:-3-8}"

trap e2e_stop_session EXIT
e2e_start_session prec_wall || exit 1

sleep 4

deep_ok=0
for _ in $(seq 1 28); do
  rm -f "$E2E_OUT/deep_19.png"
  e2e_send "capture deep_19.png"
  e2e_wait_file "$E2E_OUT/deep_19.png" 20 || continue
  st=$(e2e_stdev "$E2E_OUT/deep_19.png" || echo 0)
  mean=$(e2e_mean "$E2E_OUT/deep_19.png" || echo 0)
  cst=$(taskset -c "${CZ_CPUSET:-3-8}" convert "$E2E_OUT/deep_19.png" \
    -gravity Center -crop 400x300+0+0 +repage -format '%[fx:int(standard_deviation*65535)]' info: 2>/dev/null || echo 0)
  echo "deep_19_probe stdev=$st mean=$mean center_stdev=$cst"
  if [ "${st:-0}" -ge "$DEEP_STDEV_FLOOR" ] \
    && [ "${mean:-0}" -ge 400 ] \
    && [ "${cst:-0}" -ge "$DEEP_CENTER_STDEV_FLOOR" ]; then
    deep_ok=1
    break
  fi
  sleep 0.5
done
if [ "$deep_ok" -ne 1 ]; then
  e2e_fail_msg "precision wall: deep_19 flat/slab (need center_stdev>=$DEEP_CENTER_STDEV_FLOOR)"
  cp -f "$E2E_OUT/deep_19.png" "$REVIEW_DIR/deep_19.png" 2>/dev/null || true
  e2e_exit
fi
e2e_assert_mean_floor "$E2E_OUT/deep_19.png" 400
e2e_assert_center_structure "$E2E_OUT/deep_19.png" "$DEEP_CENTER_STDEV_FLOOR"
cp "$E2E_OUT/deep_19.png" "$REVIEW_DIR/deep_19.png"

sleep "$HOLD_SECS"
e2e_send "zoomout 10"
sleep 3
e2e_send "capture after_zoomout.png"
e2e_wait_file "$E2E_OUT/after_zoomout.png" 20 || e2e_fail_msg "missing after_zoomout.png"
cp "$E2E_OUT/after_zoomout.png" "$REVIEW_DIR/after_zoomout.png"
e2e_assert_rmse_nonzero "$E2E_OUT/deep_19.png" "$E2E_OUT/after_zoomout.png" "prec-zoomout"

e2e_send "goto $SEAHORSE_RE $SEAHORSE_IM 12"
printf '%s %s 12\n' "$SEAHORSE_RE" "$SEAHORSE_IM" >/tmp/cz_ctl.goto
sleep 3
mid_ok=0
for _ in $(seq 1 24); do
  rm -f "$E2E_OUT/mid_pot_12.png"
  e2e_send "capture mid_pot_12.png"
  e2e_wait_file "$E2E_OUT/mid_pot_12.png" 20 || continue
  cst=$(taskset -c "${CZ_CPUSET:-3-8}" convert "$E2E_OUT/mid_pot_12.png" \
    -gravity Center -crop 400x300+0+0 +repage -format '%[fx:int(standard_deviation*65535)]' info: 2>/dev/null || echo 0)
  echo "mid_pot_probe center_stdev=$cst"
  if [ "${cst:-0}" -ge 800 ]; then
    mid_ok=1
    break
  fi
  sleep 0.75
done
if [ "$mid_ok" -ne 1 ]; then
  e2e_fail_msg "mid_pot_12 never recovered after precision-wall zoom"
fi
cp "$E2E_OUT/mid_pot_12.png" "$REVIEW_DIR/mid_pot_12.png"
e2e_assert_center_structure "$E2E_OUT/mid_pot_12.png" 800

echo "review captures in $REVIEW_DIR"
e2e_exit
