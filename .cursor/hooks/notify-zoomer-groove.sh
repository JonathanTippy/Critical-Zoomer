#!/usr/bin/env bash
# DISABLED 2026-08-14: file-change hooks off (predictability). Manual JFT_Prompts review only.
set -u
PF="/tmp/jft-zoomer-groove-watch.pid"
if [[ -f "$PF" ]]; then
  pid="$(tr -d '[:space:]' <"$PF")"
  if [[ -n "$pid" && -d "/proc/$pid" ]]; then
    kill -TERM "$pid" 2>/dev/null || true
  fi
  rm -f "$PF"
fi
exit 0
