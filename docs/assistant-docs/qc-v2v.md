# QC — V2V evaluation (assistant-owned)

Evaluated under the V2V skill (strict). Evidence dated 2026-07-31 / 2026-08-01 tip after Tracey annotation repair + unit/e2e matrices green (GPU soft-skip).

## Dimension scores (0–10)

| Item | Score | Evidence |
|------|-------|----------|
| Spec | 7.8 | `docs/requirements.md` + `standards.md` + `design/*` versioned `r[]`; architecture allocation table. Auth `tile_publisher` ≥30/s wording still stale vs D-PUB-1 (known non-blocking). |
| Tracey | 7.8 | `tracey query status`: **76/77** covered, **0** untested, **0** stale. Sole uncovered: `cz.perf.home-10000tps-gpu` (soft-skip / blocked-impl). |
| Coverage | 6.0 | CI runs `cargo llvm-cov`; local summary this pass aborted under fat/stacky tests before TOTAL. `scripts/coverage.sh` exists with GUI ignores. No fresh ≥98% region proof. |
| Properties | 7.0 | IntExp commute/assoc, Range guess_biased, Mandelbrot conjugate, shade GPU↔oracle props; roster in unit-test matrix. Not majority of all correctness paths. |
| SubsystemsDesign | 6.8 | Clear headgroup/workgroup actor split; `tile_session.rs` ~2001 lines (at skill’s soft ceiling); large perturb/GPU files remain. |
| DeliveryPipeline | 7.5 | bacon jobs (tracey/mutants-core), CI llvm-cov + scoped mutants, headed e2e suite + matrices. |
| Fuzz | 6.5 | `fuzz/` targets: coords_parse, publisher_clamp, range, fuzz_target_1. |
| Mutants | 5.0 | Scoped campaign on range/intexp documented; `mutants.out` empty/stale lock — **no finished kill-rate claim**. |
| SystemsDesign | 7.4 | Stencil/tile/foveation/publisher architecture coherent; performance north stars in standards. |

## Weighted result

Baseline (60%): (7.8+7.8+6.0+7.0+6.8+7.5)/6 ≈ **7.15**  
S-tier (40%): (6.5+5.0+7.4)/3 ≈ **6.30**  
**Overall ≈ 0.6×7.15 + 0.4×6.30 = 6.81 → rounds to B only if Coverage/Mutants are not underweighted; strict call: B− / 6.9.**

Delivery QC gate is **B (7.0–7.9)**. This evaluation is **borderline**. Treat as **QC not yet solidly passed** until:
1. A clean `scripts/coverage.sh` region summary is captured, and/or
2. Mutants core campaign reports a kill rate (or justified MISSED list).

## Fixes landed during this QC pass

- Foveation half-time: `play_need_visible` stuck true after `set_mag_velocity`, starving lookahead — cleared once `seats_done > 0` or unsent publish exists (`tile_session.rs`). `foveation_balance_both_halves_within_factor_two` green again.

## Next QC actions

1. Run `taskset -c 4-11 scripts/coverage.sh` and record TOTAL region % in `coverage-baseline.txt`.
2. Revisit mutants campaign when lock is free; triage MISSED.
3. Do not claim S; do not soft-pass DAT.

**V2V Tier: B- (6.9)**  
**Strongest areas:** Spec, Tracey, DeliveryPipeline, e2e matrices  
**Most urgent gaps:** Coverage summary proof; Mutants kill-rate  
**Recommended next actions:**  
- Capture llvm-cov via `scripts/coverage.sh`  
- Finish or triage scoped mutants  
- Keep GPU 10k TPS as soft-skip until implementable  
