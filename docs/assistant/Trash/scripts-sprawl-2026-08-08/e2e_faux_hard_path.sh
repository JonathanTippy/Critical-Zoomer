#!/usr/bin/env bash
# Faux-user zoom path to the hard minibrot that showed classic perturbation
# glitch blobs when sticky references were carried without coverage.
# Location: -0.161913425661 + 1.035546905361i  mag 2^20
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=scripts/e2e_assert.sh
source "$ROOT/scripts/e2e_assert.sh"

REVIEW_DIR="${1:-/tmp/cz_e2e_faux_hard}"
mkdir -p "$REVIEW_DIR"
trap e2e_stop_session EXIT
e2e_start_session faux_hard || exit 1

snip_req="${CZ_SNIPREQ:-$E2E_PREFIX/ctl.snip}"
export CZ_SNIPREQ="$snip_req"

e2e_snip() {
  local name="$1"
  local dest="$E2E_OUT/$name"
  # Prefer in-app viewport PPM snip; fall back to X capture for PNG review.
  printf '%s\n' "${dest%.png}.ppm" >"$snip_req"
  sleep 0.4
  if [ -s "${dest%.png}.ppm" ]; then
    if command -v convert >/dev/null 2>&1; then
      convert "${dest%.png}.ppm" "$dest" || true
    fi
    e2e_pass "snip wrote ${dest%.png}.ppm"
  else
    e2e_send "capture $name"
    e2e_wait_file "$dest" 30 || e2e_fail_msg "missing $dest"
  fi
}

e2e_send "home"
sleep 1.0
e2e_snip "faux_00_home.png"

# Stepwise zoom toward the hard location (navigation path, not dead-reckon).
for pot in 0 4 8 12 16 20; do
  e2e_send "goto -0.161913425661 1.035546905361 $pot"
  sleep 0.8
  e2e_snip "faux_${pot}_step.png"
done

# Final settle at the developer repro.
e2e_send "goto -0.161913425661 + 1.035546905361i mag 2^20"
sleep 2.5
e2e_snip "faux_20_final.png"

if [ -f "$E2E_OUT/faux_20_final.png" ]; then
  e2e_assert_mean_floor "$E2E_OUT/faux_20_final.png" 400
  STDEV=$(e2e_stdev "$E2E_OUT/faux_20_final.png" || echo 0)
  if [ "$STDEV" -ge 800 ]; then
    e2e_pass "hard final has structure stdev=$STDEV"
  else
    e2e_fail_msg "hard final looks flat/glitched stdev=$STDEV"
  fi
elif [ -f "$E2E_OUT/faux_20_final.ppm" ]; then
  e2e_pass "hard final PPM present for assistant review"
else
  e2e_fail_msg "no hard final capture"
fi

e2e_exit
