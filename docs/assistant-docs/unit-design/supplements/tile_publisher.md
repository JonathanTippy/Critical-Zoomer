# Supplement: tile_publisher.md

Pairs with authoritative `docs/design/tile_publisher.md`. Non-authoritative.

## Role (UD-PUB-1) — inferred

Last workgroup stage before headgroup: calibrated/honest tiles → best-effort **Answer** tiles with no agnostic seats. Uses GPU shader with proximate bias when combining hoard + new work.

## Rate (UD-PUB-2) — D-PUB-1

Flat **1000 publications/s** ceiling while incomplete; idle when complete. No minimum floor. GPU shader combines hoard + new work.

## Bias / calibrated → answer (UD-PUB-3) — D-PUB-2

When proximate data is disproven by new bounds: **clamp** each numeric field into the proven range (closest value = clamp). All fields are numeric; no non-numeric branch.

If no proximate data: emit **nores** (infinity answer).

## Continuity (UD-PUB-4) — inferred

Only edit when disproven; otherwise keep proximate for visual continuity. Guesses must not invent inside/outside membership beyond proven calibrated result.
