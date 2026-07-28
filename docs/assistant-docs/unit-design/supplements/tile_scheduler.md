# Supplement: tile_scheduler.md

Pairs with authoritative `docs/design/tile_scheduler.md`. Non-authoritative.

## Magnification velocity (UD-TS-1) — D-SCH-2

`v_mag` = EWMA of scroll/key zoom **bumps per second**.

**Assumed EWMA:** α = 1/8 per wake (half-life ~5 wakes at 60Hz-ish input sampling). Sign of `v_mag` selects mode; magnitude may weight lookahead aggressiveness but mode thresholds use sign primarily.

## Modes (UD-TS-2) — inferred

| v_mag | Focus |
|-------|--------|
| > 0 | Foveated lookahead |
| = 0 | Foveated screen fill + lookahead |
| < 0 | Low-res backtracking |

## Foveation geometry (UD-TS-3) — D-SCH-1

- Screen fill: spiral out from **mouse** tile.
- Lookahead column: tile containing the mouse at mag, then the same spatial column at mag+1 … mag+8 (**depth-first** single-tile column), then spiral out one tile at a time at the working depth.

## Output (UD-TS-4) — inferred + D-WORK-1

Emits **tile addresses** to the tile worker. Stencil is desire only; hoard keys are addresses.
