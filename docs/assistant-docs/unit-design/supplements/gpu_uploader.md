# Supplement: gpu_uploader.md

Pairs with authoritative `docs/design/gpu_uploader.md`. Non-authoritative.

## Behavior (UD-UP-1) — inferred

- Wake on new queue item or at actor minimum wake rate (see `new/actor_graph.md`).
- Drain queue: CPU `Tile<CalibratedAnswer>` → GPU-resident `GPUTile<GPUCalibratedAnswer>` (or post-publisher answer encoding per `new/tile_and_answer.md`).
- Bypass when work is already GPU-native: do not enqueue noop uploads; publisher still receives the **calibrated tile** (GPU-resident) and biases with proximate — same as CPU (D-PUB-4); continuity only for completion (D-PUB-3).
- GPU-native completion is **not** the uploader’s job: worker commits terminal **calibrated** seats + on-device completion counter (D-GPU-1…5); full calibrated/Answer buffer readback banned on that path.

## Failure / backpressure (UD-UP-2) — inferred

Channel backpressure is incorrect code (architecture Steady State rule). Uploader must not rely on unbounded queue growth; capacities are design (actor_graph), not tuning knobs to absorb stalls.
