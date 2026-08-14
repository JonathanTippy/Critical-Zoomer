#!/usr/bin/env bash
# Workspace hook: zoomer-groove skill notifier. Canonical scripts live in JFT_Prompts/hooks.
set -u
export JFT_WATCH_TARGET="/home/jonathan/git/JFT_Prompts/skills/zoomer-groove/SKILL.md"
export JFT_WATCH_PENDING="/tmp/jft-zoomer-groove.pending"
export JFT_WATCH_WAKE="/tmp/jft-zoomer-groove.wake"
export JFT_WATCH_PIDFILE="/tmp/jft-zoomer-groove-watch.pid"
export JFT_WATCH_BASELINE="/tmp/jft-zoomer-groove.baseline"
export JFT_WATCH_LATEST="/tmp/jft-zoomer-groove.latest.msg"
export JFT_WATCH_TOKEN="AGENT_ZOOMER_GROOVE_CHANGED"
export JFT_WATCH_LOG="/tmp/jft-zoomer-groove-watch.log"
CANON="/home/jonathan/git/JFT_Prompts/hooks/notify-jft-file.sh"
exec bash "$CANON" "${1:-post}"
