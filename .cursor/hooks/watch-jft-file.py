#!/usr/bin/env python3
"""Block on inotify for one file. Paths/sentinel from env. Debounced. No polling."""
from __future__ import annotations

import ctypes
import os
import struct
import sys
import threading
import time

WATCH_FILE = os.environ["WATCH_FILE"]
PENDING = os.environ["PENDING"]
WAKE = os.environ["WAKE"]
PIDFILE = os.environ["PIDFILE"]
SENTINEL = os.environ.get("SENTINEL", "AGENT_FILE_CHANGED")
DEBOUNCE_S = float(os.environ.get("DEBOUNCE_S", "0.25"))

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


def _emit_now() -> None:
    stamp = f"{time.time():.6f}"
    with open(PENDING, "w", encoding="utf-8") as f:
        f.write(stamp + "\n")
    with open(WAKE, "a", encoding="utf-8") as f:
        f.write(f"{SENTINEL} {stamp}\n")
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
    if not os.path.isfile(WATCH_FILE):
        return 1
    directory = os.path.dirname(WATCH_FILE) or "."
    basename = os.path.basename(WATCH_FILE).encode()

    fd = libc.inotify_init()
    if fd < 0:
        return 1
    wd = libc.inotify_add_watch(fd, directory.encode(), MASK)
    if wd < 0:
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
