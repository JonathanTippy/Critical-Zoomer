#!/usr/bin/env bash
set -euo pipefail

# Need inotifywait
command -v inotifywait >/dev/null 2>&1 || {
  echo "inotify-tools required."
  exit 1
}

while true; do
  # start build
  cargo build --release &
  pid=$!

  # start watcher (blocks until any file change event)
  inotifywait -qr -e modify,create,delete,move src Cargo.toml Cargo.lock >/dev/null 2>&1

  # kill build instantly on first change
  echo "Change detected. Killing build $pid."
  kill -9 $pid 2>/dev/null || true

  # wait for process to actually die before restarting
  wait $pid 2>/dev/null || true
done
