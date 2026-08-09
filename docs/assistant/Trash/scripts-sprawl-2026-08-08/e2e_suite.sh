#!/usr/bin/env bash
# E2E suite orchestrator: harness freeze gate, then pillar scripts.
# Usage: taskset -c 4-11 scripts/e2e_suite.sh
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CPUSET="${CZ_CPUSET:-4-11}"

# Avoid NVIDIA EGL/GBM crashing Xvfb (do not force lavapipe ICD).
export __EGL_VENDOR_LIBRARY_FILENAMES="${__EGL_VENDOR_LIBRARY_FILENAMES:-/usr/share/glvnd/egl_vendor.d/50_mesa.json}"

echo "=== harness_selftest (freeze gate) ==="
taskset -c "$CPUSET" "$ROOT/scripts/harness_selftest.sh"

echo "=== oracle proving (cargo) ==="
taskset -c "$CPUSET" cargo test --lib e2e_oracle -- --test-threads=1

# Between pillar scripts, give prior EXIT traps time to stop sessions cleanly.
sleep 2

echo "=== e2e_controls ==="
taskset -c "$CPUSET" "$ROOT/scripts/e2e_controls.sh"

sleep 2

echo "=== e2e_performance ==="
taskset -c "$CPUSET" "$ROOT/scripts/e2e_performance.sh"

sleep 2

echo "=== e2e_visual ==="
taskset -c "$CPUSET" "$ROOT/scripts/e2e_visual.sh"

echo "=== e2e_suite OK ==="
