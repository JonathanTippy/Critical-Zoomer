#!/usr/bin/env bash
# Pillar 1: controls bindings + no-jump (requirements + E2E Addendum).
# r[verify cz.e2e.controls-bindings+1]
# r[verify cz.e2e.controls-no-jump+1]
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=scripts/e2e_assert.sh
source "$ROOT/scripts/e2e_assert.sh"

trap e2e_stop_session EXIT
e2e_start_session controls || exit 1

# --- Binding: home settle ---
e2e_send "home"
e2e_send "settle home.png 12 2500 1500 2000" || true
e2e_wait_file "$E2E_OUT/home.png" 40 || e2e_fail_msg "home settle missing"
e2e_send "capture home_final.png"
e2e_wait_file "$E2E_OUT/home_final.png" 20 || { e2e_fail_msg "missing $E2E_OUT/home_final.png"; e2e_exit; }
e2e_assert_mean_floor "$E2E_OUT/home_final.png" 1500
e2e_assert_center_structure "$E2E_OUT/home_final.png" 2000

# --- Shift zoom in (center) ---
e2e_send "zoomin 4"
sleep 1.2
after_in_ok=0
for _ in 1 2 3 4 5 6 7 8 9 10; do
  rm -f "$E2E_OUT/after_in_final.png"
  e2e_send "capture after_in_final.png"
  e2e_wait_file "$E2E_OUT/after_in_final.png" 15 || continue
  sleep 0.05
  mean=$(taskset -c 0-3 identify -format '%[mean]' "$E2E_OUT/after_in_final.png" 2>/dev/null | awk '{print int($1+0)}' || echo 0)
  tmpc=$(mktemp --suffix=.png)
  taskset -c 0-3 convert "$E2E_OUT/after_in_final.png" -gravity Center -crop 400x300+0+0 +repage "$tmpc" 2>/dev/null || true
  cst=$(taskset -c 0-3 identify -format '%[standard-deviation]' "$tmpc" 2>/dev/null | awk '{print int($1+0)}' || echo 0)
  rm -f "$tmpc"
  echo "after_in_probe mean=$mean center_stdev=$cst"
  if [ "${mean:-0}" -ge 1500 ] && [ "${cst:-0}" -ge 800 ]; then
    cp "$E2E_OUT/after_in_final.png" "$E2E_OUT/after_in_stable.png"
    after_in_ok=1
    break
  fi
  sleep 0.4
done
if [ "$after_in_ok" -ne 1 ]; then
  e2e_fail_msg "after zoomin never gained center structure"
fi
cp "$E2E_OUT/after_in_stable.png" "$E2E_OUT/after_in_final.png"
e2e_assert_mean_floor "$E2E_OUT/after_in_final.png" 1500
e2e_assert_center_structure "$E2E_OUT/after_in_final.png" 800
e2e_assert_rmse_nonzero "$E2E_OUT/home_final.png" "$E2E_OUT/after_in_final.png" "shift-zoomin"

# --- Space zoom out (should move opposite / toward home) ---
e2e_send "zoomout 4"
sleep 1.2
after_out_ok=0
for _ in 1 2 3 4 5 6 7 8 9 10; do
  rm -f "$E2E_OUT/after_out_final.png"
  e2e_send "capture after_out_final.png"
  e2e_wait_file "$E2E_OUT/after_out_final.png" 15 || continue
  sleep 0.05
  mean=$(taskset -c 0-3 identify -format '%[mean]' "$E2E_OUT/after_out_final.png" 2>/dev/null | awk '{print int($1+0)}' || echo 0)
  tmpc=$(mktemp --suffix=.png)
  taskset -c 0-3 convert "$E2E_OUT/after_out_final.png" -gravity Center -crop 400x300+0+0 +repage "$tmpc" 2>/dev/null || true
  cst=$(taskset -c 0-3 identify -format '%[standard-deviation]' "$tmpc" 2>/dev/null | awk '{print int($1+0)}' || echo 0)
  rm -f "$tmpc"
  echo "after_out_probe mean=$mean center_stdev=$cst"
  if [ "${mean:-0}" -ge 1500 ] && [ "${cst:-0}" -ge 800 ]; then
    cp "$E2E_OUT/after_out_final.png" "$E2E_OUT/after_out_stable.png"
    after_out_ok=1
    break
  fi
  sleep 0.4
done
if [ "$after_out_ok" -ne 1 ]; then
  e2e_fail_msg "after zoomout never gained center structure"
fi
cp "$E2E_OUT/after_out_stable.png" "$E2E_OUT/after_out_final.png"
e2e_assert_mean_floor "$E2E_OUT/after_out_final.png" 1500
e2e_assert_center_structure "$E2E_OUT/after_out_final.png" 800
e2e_assert_rmse_nonzero "$E2E_OUT/after_in_final.png" "$E2E_OUT/after_out_final.png" "space-zoomout"
# After equal in/out counts, closer to home than the deep-in frame.
e2e_assert_rmse_lt \
  "$E2E_OUT/home_final.png" "$E2E_OUT/after_out_final.png" \
  "$E2E_OUT/home_final.png" "$E2E_OUT/after_in_final.png" \
  "zoomout nearer home than zoomin"

# --- Scroll bumps (hover at center after focus) ---
# r[verify cz.ctrl.scroll-up-zooms-in+1] (headed): button 4 = scroll-up = zoom in.
e2e_send "home"
sleep 0.3
e2e_send "settle home2.png 5 2000 1200" || true
e2e_wait_file "$E2E_OUT/home2.png" 30 || true
e2e_send "capture pre_scroll.png"
e2e_wait_file "$E2E_OUT/pre_scroll.png" 20 || { e2e_fail_msg "missing $E2E_OUT/pre_scroll.png"; e2e_exit; }
# 10 bumps within ~300ms class: fire quickly (requirements tick sustain).
start_ms=$(date +%s%3N)
e2e_send "scroll 10"
end_ms=$(date +%s%3N)
elapsed=$((end_ms - start_ms))
echo "scroll10_elapsed_ms=$elapsed"
sleep 1.0
e2e_send "capture post_scroll.png"
e2e_wait_file "$E2E_OUT/post_scroll.png" 20 || { e2e_fail_msg "missing $E2E_OUT/post_scroll.png"; e2e_exit; }
e2e_assert_rmse_nonzero "$E2E_OUT/pre_scroll.png" "$E2E_OUT/post_scroll.png" "scroll10"
# Tick sustain: command accepted in under 2s wall (harness overhead); product wants 300ms input accounting.
if [ "$elapsed" -le 2000 ]; then
  e2e_pass "scroll10 dispatch wall ${elapsed}ms (<=2000 harness bound)"
else
  e2e_fail_msg "scroll10 dispatch too slow ${elapsed}ms"
fi
# Polarity: scroll-up (positive count) must deepen like Shift — farther from home than after equal scroll-down.
e2e_send "scroll -10"
sleep 1.0
e2e_send "capture post_scroll_out.png"
e2e_wait_file "$E2E_OUT/post_scroll_out.png" 20 || { e2e_fail_msg "missing $E2E_OUT/post_scroll_out.png"; e2e_exit; }
e2e_assert_rmse_lt \
  "$E2E_OUT/pre_scroll.png" "$E2E_OUT/post_scroll_out.png" \
  "$E2E_OUT/pre_scroll.png" "$E2E_OUT/post_scroll.png" \
  "scroll-up deeper than scroll-down return"

# --- Pan: arrow key ---
e2e_send "capture pre_pan.png"
e2e_wait_file "$E2E_OUT/pre_pan.png" 15 || { e2e_fail_msg "missing $E2E_OUT/pre_pan.png"; e2e_exit; }
e2e_send "key Right"
sleep 0.4
e2e_send "capture post_pan.png"
e2e_wait_file "$E2E_OUT/post_pan.png" 15 || { e2e_fail_msg "missing $E2E_OUT/post_pan.png"; e2e_exit; }
e2e_assert_mean_floor "$E2E_OUT/post_pan.png" 1200
e2e_assert_rmse_nonzero "$E2E_OUT/pre_pan.png" "$E2E_OUT/post_pan.png" "arrow-pan"

e2e_exit
