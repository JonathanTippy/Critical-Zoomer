# Supplement: gpu_uploader.md

Pairs with authoritative `docs/design/gpu_uploader.md`. Non-authoritative.

## Behavior (UD-UP-1) — inferred

- Exists for **CPU → GPU** calibrated handoff only.
- Wake on new queue item or at actor minimum wake rate (see `new/actor_graph.md`).
- Drain queue: CPU `Tile<CalibratedAnswer>` → GPU-resident calibrated tile the publisher can bind.
- **Bypass when already GPU-native:** do not enqueue; do not run noop copy. Worker → publisher directly (D-PUB-4). Auth rate may be 0 when fully bypassed.
- Uploader never owns completion, TPS, or Answer authorship. Worker commits terminal calibrated seats + on-device counter (D-GPU-1…5). Publisher turns calibrated → Answer. Full calibrated/Answer readback on the GPU-native path is banned (D-GPU-2).

## Failure / backpressure (UD-UP-2) — inferred

Channel backpressure is incorrect code (architecture Steady State rule). Uploader must not rely on unbounded queue growth; capacities are design (`actor_graph`), not stall sponges.
