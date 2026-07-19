# Phase 0–1 — status (assistant)

Quotes: `he-said/phase-0-1-applied.md`.

## What landed

- `scripts/cz_ctl.sh`, `cz_ctl_lib.sh`, `capture_naive_baseline.sh`, `precision_wall_*.sh` / probe input; CPU affinity `4-11` by default.
- `TILE_EDGE_LENGTH_POT = 6`, `Tile<Answer>`, naive CPU worker on tiles, `temporary_color32_bridge`, `TileSession` via screen_worker → collector/escaper/colorer.
- Goto/navigate file + `CZ_GOTO` / `CZ_NAV` harness support.

## Batch B — answer before Phase 2 (still open)

1. **OrbitId representation:** dense `u32` index into a Vec, or generational handle?
2. **Zero orbit identity:** single global immortal id `0`, or normal allocation that is never evicted?
3. **First nucleus search budget:** cap wall-time / candidate count for interactive use?

**Defaults if unanswered:** (1) dense `u32`, (2) immortal id `0`, (3) short interactive cap + background thorough seek on idle.

## Sequence

- Phase 1.5 period determination before orbit management.
- Batch B (OrbitId) after 1.5, before Phase 2.
- Ideal tile-only workcore deferred to Phase 4 (`he-said/tile-workcore-boundary.md`).
