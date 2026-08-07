#!/usr/bin/env bash
# Shared asserts for headed e2e scripts (frozen harness consumers).
# shellcheck source=scripts/cz_ctl_lib.sh
set -euo pipefail

e2e_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd
}

# shellcheck source=scripts/cz_ctl_lib.sh
source "$(e2e_root)/scripts/cz_ctl_lib.sh"

e2e_fail=0
e2e_pass() { echo "PASS: $*"; }
e2e_fail_msg() { echo "FAIL: $*" >&2; e2e_fail=1; }

e2e_wait_file() {
  local path="$1"
  local secs="${2:-60}"
  local deadline
  deadline=$((SECONDS + secs))
  while [ "$SECONDS" -lt "$deadline" ]; do
    if [ -s "$path" ]; then
      return 0
    fi
    sleep 0.25
  done
  return 1
}

e2e_mean() { cz_ctl_image_mean "$1"; }
e2e_stdev() { cz_ctl_image_stdev "$1"; }
e2e_rmse() { cz_ctl_rmse "$1" "$2"; }
e2e_rmse_num() { e2e_rmse "$1" "$2" | awk '{print $1}'; }

e2e_assert_mean_floor() {
  local path="$1"
  local floor="${2:-1500}"
  local m
  m=$(e2e_mean "$path" || echo 0)
  if [ "$m" -ge "$floor" ]; then
    e2e_pass "mean floor $path mean=$m (>=$floor)"
  else
    e2e_fail_msg "near-black $path mean=$m (<$floor)"
  fi
}

e2e_assert_rmse_nonzero() {
  local a="$1" b="$2" label="${3:-diff}"
  local r
  r=$(e2e_rmse_num "$a" "$b")
  if [ -n "$r" ] && awk -v r="$r" 'BEGIN{exit !(r+0 > 0)}'; then
    e2e_pass "$label RMSE=$r (nonzero)"
  else
    e2e_fail_msg "$label RMSE=$r (expected nonzero)"
  fi
}

e2e_assert_rmse_lt() {
  local a="$1" b="$2" c="$3" d="$4" label="${5:-ordering}"
  # RMSE(a,b) < RMSE(c,d)
  local r1 r2
  r1=$(e2e_rmse_num "$a" "$b")
  r2=$(e2e_rmse_num "$c" "$d")
  if awk -v x="$r1" -v y="$r2" 'BEGIN{exit !(x+0 < y+0)}'; then
    e2e_pass "$label RMSE $r1 < $r2"
  else
    e2e_fail_msg "$label expected RMSE($a,$b)=$r1 < RMSE($c,$d)=$r2"
  fi
}

# Count near-uniform NORES-gray blocks in a 6x4 center crop (stdout: integer).
# Flat NORES mid-gray (RGB~100 → ImageMagick mean ~25700).
# Only the left 4 of 6 columns are counted: under the default sinus escape wash,
# far-right exterior that escapes in one iteration paints the same mid-grey, and
# must not be treated as an unfinished NORES hole.
e2e_count_gray_holes() {
  local path="$1"
  local out
  # One convert: center crop then 120x85 tiles (6×4 on 720×340).
  out=$(convert "$path" -gravity Center -crop 720x340+0+0 +repage \
    -crop 120x85 -format '%[fx:int(mean*65535)] %[fx:int(standard_deviation*65535)]\n' info: 2>/dev/null \
    | awk 'NR<=24 { col=(NR-1)%6; if (col<4 && $2 < 400 && $1 > 25000 && $1 < 26500) c++ } END { print c+0 }')
  echo "${out:-99}"
}

# Fail if too many near-uniform NORES-gray blocks in the viewport crop.
e2e_assert_few_gray_holes() {
  local path="$1"
  local max_holes="${2:-8}"
  local holes
  holes=$(e2e_count_gray_holes "$path")
  echo "gray_holes=$holes (max $max_holes) path=$path"
  if [ "$holes" -le "$max_holes" ]; then
    e2e_pass "few gray holes ($holes <= $max_holes) $path"
  else
    e2e_fail_msg "too many gray holes ($holes > $max_holes) $path"
  fi
}

# B-DISP-1: flat NORES grey (tps:0 symptom) must not persist after home fill time.
e2e_assert_not_flat_grey() {
  local path="$1"
  local min_stdev="${2:-5000}"
  local stdev
  stdev=$(e2e_stdev "$path" || echo 0)
  if [ "$stdev" -ge "$min_stdev" ]; then
    e2e_pass "not flat grey $path stdev=$stdev (>=$min_stdev)"
  else
    e2e_fail_msg "flat/grey screen B-DISP-1 $path stdev=$stdev (<$min_stdev)"
  fi
}

e2e_assert_rmse_below() {
  local a="$1" b="$2" max="$3" label="${4:-baseline}"
  local r
  r=$(e2e_rmse_num "$a" "$b")
  if awk -v r="$r" -v m="$max" 'BEGIN{exit !(r+0 <= m+0)}'; then
    e2e_pass "$label RMSE=$r (<=$max)"
  else
    e2e_fail_msg "$label RMSE=$r (max $max)"
  fi
}

# Left mid-viewport must show structure (set/boundary). Right crop sits on the
# exterior banding (not the far-right escape-1 plateau, which under the default
# sinus wash is the same mid-grey as NORES and is not a fill failure).
e2e_assert_side_structure() {
  local path="$1"
  local min_left="${2:-1200}"
  local tmp left right lmean rmean
  tmp=$(mktemp -d)
  convert "$path" -crop 160x200+80+140 +repage "$tmp/left.png" 2>/dev/null || {
    e2e_fail_msg "left crop failed $path"
    rm -rf "$tmp"
    return
  }
  convert "$path" -crop 160x200+420+140 +repage "$tmp/right.png" 2>/dev/null || {
    e2e_fail_msg "right crop failed $path"
    rm -rf "$tmp"
    return
  }
  left=$(identify -format '%[standard-deviation]' "$tmp/left.png" 2>/dev/null | awk '{print int($1+0)}')
  right=$(identify -format '%[standard-deviation]' "$tmp/right.png" 2>/dev/null | awk '{print int($1+0)}')
  lmean=$(identify -format '%[mean]' "$tmp/left.png" 2>/dev/null | awk '{print int($1+0)}')
  rmean=$(identify -format '%[mean]' "$tmp/right.png" 2>/dev/null | awk '{print int($1+0)}')
  rm -rf "$tmp"
  echo "side left_stdev=$left left_mean=$lmean right_stdev=$right right_mean=$rmean path=$path"
  local left_ok=0 right_ok=0
  if [ -n "$left" ] && [ "$left" -ge "$min_left" ]; then
    left_ok=1
  fi
  # Exterior banding: prefer structure; allow non-NORES means as a fallback.
  if [ -n "$right" ] && [ "$right" -ge 300 ]; then
    right_ok=1
  elif [ -n "$rmean" ] && { [ "$rmean" -lt 24500 ] || [ "$rmean" -gt 26000 ]; }; then
    right_ok=1
  fi
  if [ "$left_ok" -eq 1 ] && [ "$right_ok" -eq 1 ]; then
    e2e_pass "side structure left_stdev=$left right_stdev=$right right_mean=$rmean"
  else
    e2e_fail_msg "side empty/strip-only left_stdev=${left:-missing} right_stdev=${right:-missing} right_mean=${rmean:-missing} $path"
  fi
}

# Center crop must show Mandelbrot structure (HUD-only frames fail this).
# Baseline home center stdev ~10000; empty gray ~0–100.
e2e_assert_center_structure() {
  local path="$1"
  local min_stdev="${2:-2500}"
  local tmp crop_stdev
  tmp=$(mktemp --suffix=.png)
  convert "$path" -gravity Center -crop 400x300+0+0 +repage "$tmp" 2>/dev/null || {
    e2e_fail_msg "center crop failed $path"
    rm -f "$tmp"
    return
  }
  crop_stdev=$(identify -format '%[standard-deviation]' "$tmp" 2>/dev/null | awk '{print int($1)}')
  rm -f "$tmp"
  echo "center_stdev=$crop_stdev (min $min_stdev) path=$path"
  if [ -n "$crop_stdev" ] && [ "$crop_stdev" -ge "$min_stdev" ]; then
    e2e_pass "center structure stdev=$crop_stdev (>=$min_stdev)"
  else
    e2e_fail_msg "center empty/HUD-only stdev=${crop_stdev:-missing} (<$min_stdev) $path"
  fi
}

# Start isolated headed session. Sets E2E_PREFIX, E2E_OUT, exports CZ_SESSION_PREFIX.
e2e_start_session() {
  local name="$1"
  local root
  root="$(e2e_root)"
  E2E_PREFIX="${CZ_E2E_ROOT:-/tmp}/cz_e2e_${name}_$$"
  E2E_OUT="$E2E_PREFIX/capture"
  mkdir -p "$E2E_OUT"
  export CZ_SESSION_PREFIX="$E2E_PREFIX"
  cz_ctl_session_from_prefix "$E2E_PREFIX"
  export __EGL_VENDOR_LIBRARY_FILENAMES="${__EGL_VENDOR_LIBRARY_FILENAMES:-/usr/share/glvnd/egl_vendor.d/50_mesa.json}"
  CZ_SESSION_PREFIX="$E2E_PREFIX" \
  __EGL_VENDOR_LIBRARY_FILENAMES="${__EGL_VENDOR_LIBRARY_FILENAMES}" \
  taskset -c "${CZ_CPUSET:-4-11}" \
    xvfb-run -a -s "-screen 0 900x500x24" \
    "$root/scripts/cz_ctl.sh" start "$E2E_OUT" >"$E2E_PREFIX/wrapper.log" 2>&1 &
  echo $! >"$E2E_PREFIX/xvfb_wrapper.pid"
  local i
  for i in $(seq 1 80); do
    if [ -p "$E2E_PREFIX/ctl.fifo" ] && [ -f "$E2E_PREFIX/ctl.env" ]; then
      break
    fi
    sleep 0.25
  done
  if [ ! -p "$E2E_PREFIX/ctl.fifo" ] || [ ! -f "$E2E_PREFIX/ctl.env" ]; then
    echo "e2e_start_session: fifo/env never appeared" >&2
    cat "$E2E_PREFIX/wrapper.log" >&2 || true
    return 1
  fi
  # shellcheck source=/dev/null
  source "$E2E_PREFIX/ctl.env"
  export DISPLAY XAUTHORITY
  for i in $(seq 1 60); do
    if xwininfo -root -tree 2>/dev/null | rg -q 'Critical Zoomer'; then
      return 0
    fi
    sleep 0.5
  done
  echo "e2e_start_session: window never appeared" >&2
  tail -40 "$E2E_PREFIX/xvfb.log" >&2 || true
  return 1
}

e2e_send() {
  local timeout_s=15
  case "${1:-}" in
    settle|settle\ *) timeout_s=90 ;;
  esac
  # `settle` holds the ctl daemon for many seconds; allow queued sends to wait.
  if command -v timeout >/dev/null 2>&1; then
    timeout "$timeout_s" env CZ_SESSION_PREFIX="$E2E_PREFIX" "$(e2e_root)/scripts/cz_ctl.sh" send "$@" || true
  else
    CZ_SESSION_PREFIX="$E2E_PREFIX" "$(e2e_root)/scripts/cz_ctl.sh" send "$@" || true
  fi
}

e2e_stop_session() {
  if [ -n "${E2E_PREFIX:-}" ]; then
    CZ_SESSION_PREFIX="$E2E_PREFIX" "$(e2e_root)/scripts/cz_ctl.sh" send quit 2>/dev/null || true
    sleep 0.3
    CZ_SESSION_PREFIX="$E2E_PREFIX" "$(e2e_root)/scripts/cz_ctl.sh" stop 2>/dev/null || true
  fi
}

e2e_exit() {
  if [ "$e2e_fail" -ne 0 ]; then
    echo "E2E FAILED" >&2
    exit 1
  fi
  echo "E2E OK"
  exit 0
}
