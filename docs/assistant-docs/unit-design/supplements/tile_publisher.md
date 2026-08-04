# Supplement: tile_publisher.md

Pairs with authoritative `docs/design/tile_publisher.md`. Non-authoritative.

## Role (UD-PUB-1) — inferred

Last workgroup stage before headgroup: calibrated/honest tiles → best-effort **Answer** tiles with no agnostic seats. Uses GPU shader with proximate bias when combining hoard + new work.

## Rate (UD-PUB-2) — D-PUB-1

**[20, 100000] Hz** while incomplete (20 Hz refresh floor, max 100k); idle when complete (0). GPU shader combines hoard + new work. Matches auth `docs/design/tile_publisher.md`.

## Bias / calibrated → answer (UD-PUB-3) — D-PUB-2

When proximate data is disproven by new bounds: **clamp** each numeric field into the proven range (closest value = clamp). All fields are numeric; no non-numeric branch.

If no proximate data: emit **nores** (infinity answer).

## Continuity (UD-PUB-4) — inferred

Only edit when disproven; otherwise keep proximate for visual continuity. Guesses must not invent inside/outside membership beyond proven calibrated result.

## Not completion (UD-PUB-5) — D-PUB-3 / D-GPU-*

Publisher responsibility is **continuity of output** (hoard + new work → best-effort Answer tiles at cadence). It must **not** be the observer or authority for tile completion or TPS. A tile may be complete (escaped/repeated on all seats; on-device counter) while the publisher is still emitting continuity frames, and WIP publish must not be mistaken for completion.

## Same contract as CPU (UD-PUB-6) — D-PUB-4

GPU-native bypass does **not** invent a new publisher protocol:

1. Worker sends a **calibrated tile** (on the bypass path: GPU-resident calibrated buffer + handle/binding — still a calibrated tile, not Answers).
2. Publisher biases that new calibrated work with a **proximate sampled** tile from the hoard (GPU shader), same calibrated→Answer idea as CPU (D-PUB-2).
3. Uploader bypass = skip CPU→GPU *copy* only; publisher still runs; completion stays worker-side (D-GPU-*).
