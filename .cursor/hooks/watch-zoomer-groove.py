#!/usr/bin/env python3
"""inotify watch on zoomer-groove SKILL.md → notifier msg + wake. No polling."""
from __future__ import annotations

import ctypes
import datetime as dt
import os
import struct
import subprocess
import sys
import threading
import time

TARGET = "/home/jonathan/git/JFT_Prompts/skills/zoomer-groove/SKILL.md"
PENDING = "/tmp/jft-zoomer-groove.pending"
WAKE = "/tmp/jft-zoomer-groove.wake"
PIDFILE = "/tmp/jft-zoomer-groove-watch.pid"
BASELINE = "/tmp/jft-zoomer-groove.baseline"
LATEST = "/tmp/jft-zoomer-groove.latest.msg"
DEBOUNCE_S = 0.15

IN_CLOSE_WRITE = 0x00000008
IN_MOVED_TO = 0x00000080
IN_CREATE = 0x00000100
IN_MODIFY = 0x00000002
MASK = IN_CLOSE_WRITE | IN_MOVED_TO | IN_CREATE | IN_MODIFY
EVENT_FMT = "iIII"
EVENT_SIZE = struct.calcsize(EVENT_FMT)
libc = ctypes.CDLL("libc.so.6", use_errno=True)

_lock = threading.Lock()
_timer: threading.Timer | None = None


def _diff() -> str:
    if not os.path.isfile(BASELINE):
        return "(no baseline yet)"
    r = subprocess.run(
        ["diff", "-u", BASELINE, TARGET],
        check=False,
        capture_output=True,
        text=True,
    )
    return r.stdout or ""


def _build_msg(diff: str) -> str:
    when = dt.datetime.now().strftime("%Y-%m-%d %H:%M:%S %z")
    return "\n".join(
        [
            f"Agent Config Notifier {when}",
            f"The file {TARGET} has been edited on disk.",
            "The changes follow:",
            diff,
            "acknowledge what changed and how that will effect your behavior.",
            "Try to infer what I wanted in writing that. if I change how something is tested or configured, that means I want it to be in the new standard.",
            "If this yields an action item, consider this a interruption and execute it immediately.",
            "",
        ]
    )


def _emit_now() -> None:
    try:
        with open(TARGET, "rb") as f:
            new = f.read()
    except OSError:
        return
    old = b""
    if os.path.isfile(BASELINE):
        try:
            with open(BASELINE, "rb") as f:
                old = f.read()
        except OSError:
            old = b""
    if new == old:
        return
    diff = _diff()
    if not diff.strip():
        with open(BASELINE, "wb") as f:
            f.write(new)
        return
    msg = _build_msg(diff)
    with open(LATEST, "w", encoding="utf-8") as f:
        f.write(msg)
    with open(BASELINE, "wb") as f:
        f.write(new)
    stamp = f"{time.time():.6f}"
    with open(PENDING, "w", encoding="utf-8") as f:
        f.write(stamp + "\n")
    with open(WAKE, "a", encoding="utf-8") as f:
        f.write(f"AGENT_ZOOMER_GROOVE_CHANGED {stamp}\n")
        f.flush()
        os.fsync(f.fileno())


def schedule_emit() -> None:
    global _timer
    with _lock:
        if _timer is not None:
            _timer.cancel()
        _timer = threading.Timer(DEBOUNCE_S, _emit_now)
        _timer.daemon = True
        _timer.start()


def main() -> int:
    if not os.path.isfile(TARGET):
        return 1
    if not os.path.isfile(BASELINE):
        with open(TARGET, "rb") as src, open(BASELINE, "wb") as dst:
            dst.write(src.read())
    directory = os.path.dirname(TARGET) or "."
    basename = os.path.basename(TARGET).encode()
    fd = libc.inotify_init()
    if fd < 0:
        return 1
    if libc.inotify_add_watch(fd, directory.encode(), MASK) < 0:
        os.close(fd)
        return 1
    with open(PIDFILE, "w", encoding="utf-8") as f:
        f.write(str(os.getpid()) + "\n")
    try:
        while True:
            buf = os.read(fd, 4096)
            if not buf:
                time.sleep(0.05)
                continue
            off = 0
            while off + EVENT_SIZE <= len(buf):
                _wd, mask, _cookie, name_len = struct.unpack_from(EVENT_FMT, buf, off)
                off += EVENT_SIZE
                name = buf[off : off + name_len].split(b"\x00", 1)[0] if name_len else b""
                off += name_len
                if name == basename or name == b"":
                    schedule_emit()
    finally:
        try:
            os.remove(PIDFILE)
        except OSError:
            pass
        os.close(fd)
    return 0


if __name__ == "__main__":
    sys.exit(main())
