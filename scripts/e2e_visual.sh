#!/usr/bin/env bash
# Pillar 3: visual oracle compares + capture set for assistant review.
# r[verify cz.e2e.visual-oracle+1]
# r[verify cz.e2e.visual-assistant-review+1]
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=scripts/e2e_assert.sh
source "$ROOT/scripts/e2e_assert.sh"

REVIEW_DIR="${1:-/tmp/cz_e2e_visual_review}"
mkdir -p "$REVIEW_DIR"
trap e2e_stop_session EXIT
e2e_start_session visual || exit 1

# Oracle-backed expectations from known-good code (printed fingerprint for logs).
FP=$(taskset -c "${CZ_CPUSET:-4-11}" cargo test --quiet --lib e2e_oracle::oracle_proving_tests::home_fingerprint_stable -- --exact 2>&1 | tail -5 || true)
echo "oracle_proving_note=$FP"

e2e_send "home"
# B-DISP-1 responsiveness observation only: early capture must not stay flat
# NORES-grey. A transient purple wash may still appear here; final correctness
# waits on structure readiness below, not this deadline.
sleep 1.5
e2e_send "capture grey_regress_guard.png"
e2e_wait_file "$E2E_OUT/grey_regress_guard.png" 20 || { e2e_fail_msg "missing grey_regress_guard.png"; e2e_exit; }
e2e_assert_not_flat_grey "$E2E_OUT/grey_regress_guard.png" 5000

# Final home readiness: structure + optional baseline, stable across consecutive
# captures. Do not treat "few gray holes" alone as complete (flat purple has 0).
BASELINE="$ROOT/scripts/baseline_home_final.png"
if ! e2e_wait_home_ready "$E2E_OUT/vis_home_final.png" 48 "$BASELINE" 12000; then
  e2e_fail_msg "visual home never reached structured readiness"
  e2e_exit
fi
e2e_assert_mean_floor "$E2E_OUT/vis_home_final.png" 1500
HOME_STDEV=$(e2e_stdev "$E2E_OUT/vis_home_final.png")
# Structured Mandelbrot home (oracle: inside+outside) ⇒ high stdev, not flat.
if [ "$HOME_STDEV" -ge 3000 ]; then
  e2e_pass "home oracle-like structure stdev=$HOME_STDEV"
else
  e2e_fail_msg "home lacks structure stdev=$HOME_STDEV"
fi
e2e_assert_few_gray_holes "$E2E_OUT/vis_home_final.png" 2
if [ -f "$BASELINE" ]; then
  CROP_DIR=$(mktemp -d)
  convert "$BASELINE" -gravity Center -crop 720x340+0+0 +repage "$CROP_DIR/baseline_crop.png"
  convert "$E2E_OUT/vis_home_final.png" -gravity Center -crop 720x340+0+0 +repage "$CROP_DIR/current_crop.png"
  e2e_assert_rmse_below "$CROP_DIR/baseline_crop.png" "$CROP_DIR/current_crop.png" 12000 "home-baseline-crop"
  rm -rf "$CROP_DIR"
fi
e2e_assert_center_structure "$E2E_OUT/vis_home_final.png" 2500
e2e_assert_side_structure "$E2E_OUT/vis_home_final.png" 1200

# Tenacious: unfinished deep must not be flat set-black.
e2e_send "goto -2.0 0.0 8"
sleep 0.5
e2e_send "capture deep_early.png"
e2e_wait_file "$E2E_OUT/deep_early.png" 20 || { e2e_fail_msg "missing $E2E_OUT/deep_early.png"; e2e_exit; }
e2e_assert_mean_floor "$E2E_OUT/deep_early.png" 400
EARLY_STDEV=$(e2e_stdev "$E2E_OUT/deep_early.png" || echo 0)
if [ "$EARLY_STDEV" -ge 800 ]; then
  e2e_pass "deep early not flat-black stdev=$EARLY_STDEV"
else
  e2e_fail_msg "deep early flat-blackish stdev=$EARLY_STDEV"
fi
# Confirm goto took effect: frame must differ from home.
e2e_assert_rmse_nonzero "$E2E_OUT/vis_home_final.png" "$E2E_OUT/deep_early.png" "goto-deep"

# Continuity across pan: not cleared to black.
e2e_send "home"
sleep 0.3
e2e_send "settle vis_home2.png 4 2000 1200" || true
e2e_wait_file "$E2E_OUT/vis_home2.png" 25 || true
e2e_send "capture pan_pre.png"
e2e_wait_file "$E2E_OUT/pan_pre.png" 15 || { e2e_fail_msg "missing $E2E_OUT/pan_pre.png"; e2e_exit; }
e2e_send "key Left"
e2e_send "key Left"
sleep 0.35
e2e_send "capture pan_post.png"
e2e_wait_file "$E2E_OUT/pan_post.png" 15 || { e2e_fail_msg "missing $E2E_OUT/pan_post.png"; e2e_exit; }
e2e_assert_mean_floor "$E2E_OUT/pan_post.png" 1200
e2e_assert_rmse_nonzero "$E2E_OUT/pan_pre.png" "$E2E_OUT/pan_post.png" "pan-continuity"

# Copy captures for assistant review (fallible corroboration).
cp -f "$E2E_OUT"/vis_home_final.png "$E2E_OUT"/deep_early.png "$E2E_OUT"/pan_post.png \
  "$REVIEW_DIR"/ 2>/dev/null || true
echo "ASSISTANT_REVIEW_DIR=$REVIEW_DIR" >"$REVIEW_DIR/README.txt"
echo "Review these PNGs; fallible — never sole pass/fail." >>"$REVIEW_DIR/README.txt"
e2e_pass "assistant review captures staged in $REVIEW_DIR"

e2e_exit
