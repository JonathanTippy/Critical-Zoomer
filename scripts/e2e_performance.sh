#!/usr/bin/env bash
# Pillar 2: performance — <5s home fill; simple perfect; hard lower-res but keeping pace.
# r[verify cz.e2e.perf-home-fill+1]
# r[verify cz.e2e.perf-zoom-simple+1]
# r[verify cz.e2e.perf-zoom-hard+1]
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=scripts/e2e_assert.sh
source "$ROOT/scripts/e2e_assert.sh"

trap e2e_stop_session EXIT
e2e_start_session perf || exit 1

# --- Home fill <5s ---
e2e_send "home"
t0=$(date +%s%3N)
# Mid-wait capture must not be flat black — one shot, then leave the app alone.
sleep 0.45
e2e_send "capture mid_home.png"
e2e_wait_file "$E2E_OUT/mid_home.png" 15 || true
if [ -s "$E2E_OUT/mid_home.png" ]; then
  e2e_assert_mean_floor "$E2E_OUT/mid_home.png" 800
fi
# Let fill run undisturbed: frequent capture+ImageMagick during the window
# was starving Xvfb paint. Probe only in the last ~2.5s of the bar.
deadline_ms=$((t0 + 5000))
quiet_until_ms=$((t0 + 2000))
now=$(date +%s%3N)
if [ "$now" -lt "$quiet_until_ms" ]; then
  sleep_ms=$((quiet_until_ms - now))
  sleep "$(awk -v ms="$sleep_ms" 'BEGIN{printf "%.3f", ms/1000}')" || sleep 2
fi
fill_ok=0
fill_ms=5001
while true; do
  rm -f "$E2E_OUT/home_fill_final.png"
  e2e_send "capture home_fill_final.png"
  e2e_wait_file "$E2E_OUT/home_fill_final.png" 20 || { e2e_fail_msg "missing home_fill_final.png"; e2e_exit; }
  # Match e2e_assert_side_structure: right crop on exterior banding, not the
  # far-right escape-1 plateau (same mid-grey as NORES under default sinus).
  left=$(taskset -c 0-3 convert "$E2E_OUT/home_fill_final.png" -crop 160x200+80+140 +repage -format '%[standard-deviation]' info: 2>/dev/null | awk '{print int($1+0)}' || echo 0)
  right=$(taskset -c 0-3 convert "$E2E_OUT/home_fill_final.png" -crop 160x200+420+140 +repage -format '%[standard-deviation]' info: 2>/dev/null | awk '{print int($1+0)}' || echo 0)
  rmean=$(taskset -c 0-3 convert "$E2E_OUT/home_fill_final.png" -crop 160x200+420+140 +repage -format '%[mean]' info: 2>/dev/null | awk '{print int($1+0)}' || echo 0)
  left=${left:-0}; right=${right:-0}; rmean=${rmean:-0}
  right_ok=0
  if [ "$right" -ge 300 ]; then right_ok=1; fi
  if [ "$rmean" -lt 24500 ] || [ "$rmean" -gt 26000 ]; then right_ok=1; fi
  holes_n=$(taskset -c 0-3 bash -c "source \"$ROOT/scripts/e2e_assert.sh\"; e2e_count_gray_holes \"$E2E_OUT/home_fill_final.png\"")
  echo "home_fill_probe holes=$holes_n left=$left right=$right right_mean=$rmean"
  if [ "$holes_n" -le 2 ] && [ "$left" -ge 1200 ] && [ "$right_ok" -eq 1 ]; then
    fill_ok=1
    t1=$(date +%s%3N)
    fill_ms=$((t1 - t0))
    echo "home_fill_ms=$fill_ms"
    break
  fi
  now=$(date +%s%3N)
  if [ "$now" -ge "$deadline_ms" ]; then
    t1=$now
    fill_ms=$((t1 - t0))
    echo "home_fill_ms=$fill_ms"
    break
  fi
  sleep 0.2 || true
done
e2e_assert_mean_floor "$E2E_OUT/home_fill_final.png" 1500
if [ "$fill_ms" -le 5000 ]; then
  e2e_pass "home fill ${fill_ms}ms (<=5000)"
else
  e2e_fail_msg "home fill ${fill_ms}ms (>5000)"
fi
if [ "$fill_ok" -ne 1 ]; then
  e2e_fail_msg "home fill never reached few gray holes + side structure within 5s"
fi
STDEV=$(e2e_stdev "$E2E_OUT/home_fill_final.png" || echo 0)
if [ "$STDEV" -ge 2000 ]; then
  e2e_pass "home fill structured stdev=$STDEV"
else
  e2e_fail_msg "home fill too flat stdev=$STDEV"
fi
holes_final=$(taskset -c 0-3 bash -c "source \"$ROOT/scripts/e2e_assert.sh\"; e2e_count_gray_holes \"$E2E_OUT/home_fill_final.png\"")
echo "gray_holes=$holes_final (max 2) path=$E2E_OUT/home_fill_final.png"
if [ "$holes_final" -le 2 ]; then
  e2e_pass "few gray holes ($holes_final <= 2) $E2E_OUT/home_fill_final.png"
else
  e2e_fail_msg "too many gray holes ($holes_final > 2) $E2E_OUT/home_fill_final.png"
fi
e2e_assert_center_structure "$E2E_OUT/home_fill_final.png" 2500
e2e_assert_side_structure "$E2E_OUT/home_fill_final.png" 1200

# --- Simple area: exterior zoom stays high quality while keeping pace ---
e2e_send "goto 1.5 0.0 -1"
sleep 0.4
e2e_send "settle simple0.png 4 1500 1000" || true
e2e_wait_file "$E2E_OUT/simple0.png" 25 || true
e2e_send "capture simple_pre.png"
e2e_wait_file "$E2E_OUT/simple_pre.png" 20 || { e2e_fail_msg "missing simple_pre.png"; e2e_exit; }
e2e_send "zoomin 6"
sleep 0.2
e2e_send "capture simple_mid.png"
e2e_wait_file "$E2E_OUT/simple_mid.png" 20 || { e2e_fail_msg "missing simple_mid.png"; e2e_exit; }
e2e_send "settle simple_post.png 4 1500 1000" || true
e2e_wait_file "$E2E_OUT/simple_post.png" 25 || true
e2e_send "capture simple_final.png"
e2e_wait_file "$E2E_OUT/simple_final.png" 20 || { e2e_fail_msg "missing simple_final.png"; e2e_exit; }
e2e_assert_mean_floor "$E2E_OUT/simple_mid.png" 1000
e2e_assert_mean_floor "$E2E_OUT/simple_final.png" 1200
e2e_assert_rmse_nonzero "$E2E_OUT/simple_pre.png" "$E2E_OUT/simple_final.png" "simple-zoom"
# Perfect-ish: mid frame not a black stall during zoom.
MID_STDEV=$(e2e_stdev "$E2E_OUT/simple_mid.png" || echo 0)
if [ "$MID_STDEV" -ge 1500 ]; then
  e2e_pass "simple zoom mid structured stdev=$MID_STDEV"
else
  e2e_fail_msg "simple zoom mid too flat stdev=$MID_STDEV"
fi

# --- Hard area: seahorse — lower res OK, must keep pace / not empty ---
e2e_send "goto -0.743643887037151 0.131825904205216 12"
sleep 0.3
e2e_send "capture hard_t0.png"
e2e_wait_file "$E2E_OUT/hard_t0.png" 25 || { e2e_fail_msg "missing hard_t0.png"; e2e_exit; }
e2e_send "zoomin 4"
sleep 0.15
e2e_send "capture hard_mid.png"
e2e_wait_file "$E2E_OUT/hard_mid.png" 25 || { e2e_fail_msg "missing hard_mid.png"; e2e_exit; }
sleep 2
e2e_send "capture hard_later.png"
e2e_wait_file "$E2E_OUT/hard_later.png" 25 || { e2e_fail_msg "missing hard_later.png"; e2e_exit; }
e2e_assert_mean_floor "$E2E_OUT/hard_t0.png" 500
e2e_assert_mean_floor "$E2E_OUT/hard_mid.png" 500
e2e_assert_mean_floor "$E2E_OUT/hard_later.png" 500
# Keeping pace: mid differs from t0 (view moved) even if low-res.
e2e_assert_rmse_nonzero "$E2E_OUT/hard_t0.png" "$E2E_OUT/hard_mid.png" "hard-zoom-pace"

e2e_exit
