# Supplement: tile_publisher.md

Pairs with authoritative `docs/design/tile_publisher.md`. Non-authoritative.

## Role (UD-PUB-1) — inferred

Last workgroup stage before headgroup: calibrated/honest tiles → best-effort **Answer** tiles with no agnostic seats. Runs as a **GPU shader**: proximate bias over hoard + new calibrated work. Output is GPU-resident Answer tiles for the headgroup.

## Rate (UD-PUB-2) — D-PUB-1 / D-PUB-5

**[20, 100000] Hz** while incomplete (20 Hz floor, 100k publish-Hz ceiling); idle when complete (0). Publish rate is not TPS.

Wake source: **worker notify** after calibrated commits (D-PUB-5), Steady State style — not silent timer discovery. Cadence still bounds how often notify may produce a publish.

## Bias / calibrated → answer (UD-PUB-3) — D-PUB-2

When proximate data is disproven by new bounds: **clamp** each numeric field into the proven range. All fields are numeric; no non-numeric branch.

If no proximate data: emit **nores** (infinity answer).

## Continuity (UD-PUB-4) — inferred

Only edit when disproven; otherwise keep proximate for visual continuity. Guesses must not invent inside/outside membership beyond proven calibrated result.

## Not completion (UD-PUB-5) — D-PUB-3 / D-GPU-*

Publisher is **continuity of output only**. It must not observe or declare tile completion or TPS. Worker on-device counters own completion (D-GPU-3/4). WIP calibrated updates must not be mistaken for tile completion.

## GPU-native hot path (UD-PUB-6) — D-PUB-4 / D-PUB-6 / D-GPU-7

Publisher sits after the **calibrated source** (D-PUB-6):

**GPU-native:** worker → publisher (bypass).  
**CPU:** worker → uploader → publisher.

1. Worker writes **GPU-resident calibrated** seats after every bout — progressed seats, including partial ranges (D-GPU-7), then **notifies** the publisher (D-PUB-5). Partial writes ≠ TPS done.
2. Publisher binds that calibrated buffer **directly** (no CPU copy, no uploader on the GPU-native path).
3. Publisher GPU shader biases with proximate hoard → GPU Answer tile → headgroup.

The publisher protocol does not change between paths; only residency of the calibrated input does.
