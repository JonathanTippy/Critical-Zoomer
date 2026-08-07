# Phase 3 — reference orbit collection (assistant notes)

Non-authoritative. Batch B locked in `he-said/batch-B.md`.

## Landed

- `OrbitId = u32`, `ZERO_ORBIT_ID = 0` immortal zero orbit in `ReferenceCollection`
- `ReferenceOrbit` / `PeriodicOrbit` pub in `workcore/mandelbrot/mod.rs`
- f64 build via `detect_period_for_c` + short interactive seek (`REFERENCE_NUCLEUS_SEEK_ITERS_INTERACTIVE`); ~512MB budget constant
- `TileSession` owns collection + per-seat `OrbitId` (default zero); throttled attention probe when relative C generator allows

## Not yet (Phase 4+)

- Rug high-precision orbit build
- Idle thorough nucleus seek
- Eviction / suitability ranking under memory budget
- Tile-only workcore cutover (`he-said/tile-workcore-boundary.md`)
- Live `TileSession` flip to perturbation (see `phase-4-notes.md`; CPU worker landed, `USE_PERTURBATION_CPU=false`)
