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
# B-DISP-1 grey-screen regression: must show structure within ~1.5s (not flat NORES).
sleep 1.5
e2e_send "capture grey_regress_guard.png"
e2e_wait_file "$E2E_OUT/grey_regress_guard.png" 20 || { e2e_fail_msg "missing grey_regress_guard.png"; e2e_exit; }
e2e_assert_not_flat_grey "$E2E_OUT/grey_regress_guard.png" 5000
# Quiet fill then poll until few gray holes (same bar as performance).
sleep 1.0
fill_ok=0
for _ in 1 2 3 4 5 6 7 8 9 10; do
  rm -f "$E2E_OUT/vis_home_final.png"
  e2e_send "capture vis_home_final.png"
  e2e_wait_file "$E2E_OUT/vis_home_final.png" 20 || continue
  holes_n=$(taskset -c 0-3 bash -c "source \"$ROOT/scripts/e2e_assert.sh\"; e2e_count_gray_holes \"$E2E_OUT/vis_home_final.png\"")
  echo "vis_home_probe holes=$holes_n"
  if [ "${holes_n:-99}" -le 2 ]; then
    fill_ok=1
    break
  fi
  sleep 0.25
done
if [ "$fill_ok" -ne 1 ]; then
  e2e_fail_msg "visual home never cleared gray holes"
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
