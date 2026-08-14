#!/usr/bin/env python3
"""Thin wrapper: canonical watcher is JFT_Prompts/hooks/watch-jft-file.py."""
import os
import sys

os.environ.setdefault("JFT_WATCH_TARGET", "/home/jonathan/git/JFT_Prompts/skills/zoomer-groove/SKILL.md")
os.environ.setdefault("JFT_WATCH_PENDING", "/tmp/jft-zoomer-groove.pending")
os.environ.setdefault("JFT_WATCH_WAKE", "/tmp/jft-zoomer-groove.wake")
os.environ.setdefault("JFT_WATCH_PIDFILE", "/tmp/jft-zoomer-groove-watch.pid")
os.environ.setdefault("JFT_WATCH_BASELINE", "/tmp/jft-zoomer-groove.baseline")
os.environ.setdefault("JFT_WATCH_LATEST", "/tmp/jft-zoomer-groove.latest.msg")
os.environ.setdefault("JFT_WATCH_TOKEN", "AGENT_ZOOMER_GROOVE_CHANGED")

CANON = "/home/jonathan/git/JFT_Prompts/hooks/watch-jft-file.py"
sys.argv[0] = CANON
with open(CANON, encoding="utf-8") as f:
    code = compile(f.read(), CANON, "exec")
exec(code, {"__name__": "__main__", "__file__": CANON})
