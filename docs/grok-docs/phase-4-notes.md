# Phase 4 — classical perturbation CPU (assistant notes)

Non-authoritative. Batch C locked in `he-said/batch-C.md`.

## Landed

- `PerturbationCpuWorker` (`perturbation_cpu_worker.rs`): f64 Worker over `ReferenceCollection` + per-seat `OrbitId`
- Pauldelbrot glitch at `GLITCH_THRESHOLD = 1e-4` → rebind seat to `ZERO_ORBIT_ID` (δc ← c_pixel, δz ← 0)
- Series coefficients filled on nucleus build; series skip before perturbation tail (same phase, second dense step)
- `PeriodicOrbit` index fixed for zero-orbit wrap
- Tests: shallow naive↔perturb match, glitch rebind, period-unknown `0`, seahorse-valley fixture, series nonempty
- `USE_PERTURBATION_CPU = false` in `constants.rs` — live `TileSession` still `NaiveGpuWorker`

## Not yet

- Flip `TileSession` to perturbation CPU (or dual path behind the const)
- Rug high-precision reference build / idle thorough seek
- Perturbation GPU worker (`perturbation_gpu_worker.rs` still empty)
- Eviction / suitability ranking under memory budget
