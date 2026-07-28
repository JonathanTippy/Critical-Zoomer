# Unit-test matrix (assistant-owned, non-authoritative)

Phase gate: every row `green`. Soft-skip GPU ≠ green. `D-*` decisions are oracles only when they do not contradict authoritative docs. D-PUB-1: flat 1000 Hz + GPU publisher (no min floor).

Status: `green` | `in-progress` | `blocked-impl`

## A — Tagged non-e2e

| Id | Tests (≥3) | Property | Verify | Status |
|----|------------|----------|--------|--------|
| cz.math.intexp-add-commutative+1 | prop + examples in `intexp.rs` | add commutative | intexp.rs | green |
| cz.math.intexp-mul-associative+1 | prop + examples in `intexp.rs` | mul associative | intexp.rs | green |
| cz.math.homothety-zoom-fill-associative+1 | views.rs prop + cases | associative fill | views.rs | green |
| cz.math.mandelbrot-real-axis-symmetry+1 | naive_cpu + e2e_oracle | conjugate symmetry | naive_cpu / e2e_oracle | green |
| cz.range.guess-biased-nearest+1 | range.rs + calibrated + gpu_tile | stays in bounds | range.rs | green |
| cz.display.window-default-800x480+1 | constants.rs (≥3) | n/a | constants.rs | green |
| cz.display.offscreen-r2-circle+1 | offscreen.rs (≥3) | n/a | offscreen.rs | green |
| cz.display.nores-when-no-proximate+1 | sampling + gpu_tile + shade | n/a | multi | green |
| cz.tenacious.nores-not-flat-black+1 | b_ten_1 + shade | n/a | multi | green |
| cz.hoarding.one-answer-per-point+1 | sampling hoard_tests (≥3) | n/a | sampling.rs | green |
| cz.fast.natural-zoom-2x+1 | inputs + transforms (≥3) | n/a | inputs.rs | green |
| cz.seamless.perturbation-always-on+1 | phase4 + session (≥3) | n/a | multi | green |
| cz.seamless.gpu-preferred+1 | gpu_context + perturb_gpu + uploader | n/a | multi | green |
| cz.seamless.reference-background+1 | reference_worker + session (≥3) | n/a | multi | green |
| cz.seamless.foveated-mag-velocity+1 | tile_scheduler + session (≥3) | n/a | multi | green |
| cz.system.tile-manager-protect-current-lookahead+1 | tile_manager (≥3) | n/a | tile_manager.rs | green |
| cz.system.max-homotheties+1 | tile_manager + sampling (≥3) | n/a | multi | green |
| cz.int.memory-bump+1 | tile_manager + window + unit extras | n/a | multi | in-progress |
| cz.int.hoard-ingest-sample+1 | sampling (≥3 unit) | n/a | sampling.rs | in-progress |
| cz.int.publisher-nores-bias+1 | tile_publisher (≥3) | clamp bounds | tile_publisher.rs | green |
| cz.int.publish-cadence+1 | cadence max 1000; no min floor (D-PUB-1 flat 1000) | n/a | tile_publisher.rs | in-progress |
| cz.int.stencil-retarget+1 | unit extract (≥3) | n/a | stencil / window | in-progress |
| cz.int.session-pipeline+1 | unit extract (≥3) | n/a | tile_session | in-progress |
| cz.shade.escape-continues-to-bailout+1 | shade_tests (≥3) | n/a | shade_tests.rs | green |
| cz.shade.in-filament-slope-inversion+1 | shade_tests (≥3) | n/a | shade_tests.rs | green |
| cz.shade.out-filament-period-step+1 | shade_tests + higher-period side | n/a | shade_tests.rs | in-progress |
| cz.shade.node-smallness-minimum+1 | shade_tests (≥3) | n/a | shade_tests.rs | green |
| cz.shade.small-time-edge-nonzero+1 | shade_tests (≥3) | n/a | shade_tests.rs | green |
| cz.shade.layers-in-script-order+1 | shade_tests (≥3) | GPU↔oracle | shade_tests.rs | green |
| cz.perf.foveation-half-time+1 | tile_session (≥3) | 50/50 time | tile_session / standards_perf | green |
| cz.perf.home-100tps+1 | home fill release (≥3) | ≤5s | tile_session | green |
| cz.perf.min-300m-ips-cpu+1 | standards_perf (≥3) | ≥300M | standards_perf | green |
| cz.perf.min-30b-ips-gpu+1 | perturb_gpu (≥3) | ≥30B | perturb_gpu | green |
| cz.perf.optimal-ipp+1 | standards_perf (≥3) | escape IPP | standards_perf | green |
| cz.perf.headgroup-shaders-2ms+1 | standards_perf (≥3) | ≤2ms | standards_perf | green |
| cz.perf.headgroup-vsync+1 | standards_perf (≥3) | Fifo | window / standards_perf | green |
| cz.ctrl.zoom-in-homothety+1 | transforms (≥3) | pointer-fixed | transforms.rs | green |
| cz.ctrl.scroll-up-zooms-in+1 | inputs (≥3) | polarity | inputs.rs | green |
| cz.fast.settings-100ms+1 | standards_perf (≥3) | ≤100ms | standards_perf | green |
| cz.fast.cosmetic-17ms-1080p+1 | standards_perf (≥3) | ≤17ms | standards_perf | green |
| cz.fast.scroll-10-in-300ms+1 | inputs (≥3) | debt | inputs.rs | green |
| cz.fast.no-tick-backlog+1 | inputs (≥3) | debt | inputs.rs | green |
| cz.fast.shift-space-5bps+1 | inputs (≥3) | ~5/s | inputs.rs | green |
| cz.fast.input-next-frame-17ms+1 | inputs (≥3) | same-turn | inputs.rs | green |
| cz.system.memory-default-1gb+1 | standards_perf (≥3) | 1e9 | settings | green |
| cz.cosmetic.bailout-range-2-255+1 | standards_perf (≥3) | [2,255] | settings | green |
| cz.deep.min-zoom-pot-capacity+1 | standards_perf (≥3) | pot | gear / intexp | green |

## B — Untagged requirement slices

| Id | Tests | Property | Status |
|----|-------|----------|--------|
| REQ-CTRL-PARSE | coords.rs (≥3) | n/a | in-progress |
| REQ-CTRL-APPLY | apply enable + D-UI-1 (≥3) | n/a | in-progress |
| REQ-CTRL-ZOOM | inputs/transforms (≥3) | n/a | green |
| REQ-COSMETIC-LAYER | settings layer model (≥3) | n/a | in-progress |
| REQ-COSMETIC-DEFAULT | D-COLOR-1 exact three layers (≥3) | n/a | in-progress |
| REQ-BAILOUT | D-BAIL-1 (≥3) | n/a | in-progress |
| REQ-SYS-MEM | D-MEM-* (≥3) | keep-set det. | in-progress |
| REQ-CALIBRATED | calibrated bias (≥3) | clamp | green |
| REQ-DEEP-GEAR | C gen fail-closed + D-GEAR-1 (≥3) | n/a | in-progress |

## C — Decisions `D-*`

| Id | Tests | Status |
|----|-------|--------|
| D-COLOR-1 | default script length 3; kinds escape/in-fil/out-fil | in-progress |
| D-COLOR-2 | layer field presence | in-progress |
| D-COLOR-3 | alpha-over order | in-progress |
| D-COLOR-4 | highlights in script list | in-progress |
| D-BAIL-1 | recolor-only membership stable | in-progress |
| D-SHADE-1 | INFIL threshold constant applied | in-progress |
| D-SHADE-2 | NODE threshold constant applied | in-progress |
| D-SHADE-3 | higher-period side only | in-progress |
| D-MEM-1 | exact bump | in-progress |
| D-MEM-2 | slider moves | in-progress |
| D-MEM-3 | packed bytes cost | in-progress |
| D-MEM-4 | keep-set property | in-progress |
| D-SCH-1 | mouse column depth 8 | in-progress |
| D-SCH-2 | EWMA mag velocity | in-progress |
| D-SCH-3 | immediate preempt + resume_suspended on drain | in-progress |
| D-PER-1 | twin N=16 | in-progress |
| D-PER-2 | relative ε | in-progress |
| D-PER-3 | POT snapshots | in-progress |
| D-GEAR-1 | no mid-tile escalate API | in-progress |
| D-SERIES-1 | series_skip + absorption (≥3) | in-progress |
| D-CANCEL-1 | cancel keeps hoard | in-progress |
| D-REF-1 | +20 bits | in-progress |
| D-REF-2 | retire last-user or >N=3 | in-progress |
| D-PUB-1 | max 1000; no min floor; GPU publisher | in-progress |
| D-PUB-2 | clamp all-numeric | in-progress |
| D-STEN-1 | mouse+vel+seq fields | in-progress |
| D-WORK-1 | address-only keys | in-progress |
| D-UI-1 | apply enabled when equal | in-progress |

## D — Property roster

| Surface | Property | Status |
|---------|----------|--------|
| IntExp add | commutative | green |
| IntExp mul | associative | green |
| StackedIntExp | agrees IntExp | green |
| FloatExp | production invariant | in-progress |
| Range guess_biased | in bounds | green |
| Homothety fill | associative | green |
| Mandelbrot | conjugate symmetry | green |
| Tile manager | deterministic keep-set | in-progress |
| Sampling nearest | up-left tie | in-progress |
| Publisher clamp | within bounds | green |
| Shade GPU↔oracle | pixel agree | green |
| Work controller locals | enumerated helpers | in-progress |
| Screen worker locals | enumerated helpers | in-progress |
| Gpu uploader | bypass identity | in-progress |

## Auth note

Developer cadence rule: flat **1000/s** ceiling (D-PUB-1). Auth `tile_publisher.md` still mentions ≥30/s — treat as stale until human edits; tests enforce flat 1000. GPU publisher shader is still required.

## Preflight (2026-07-27)

- GPU: `/dev/dri` present; `gpu_context` tests pass.
- Series: `series_skip` exists in `perturbation_cpu_worker.rs` — needs ≥3 dedicated tests + absorption.
- Preempt: PhaseJobTracker exists; live drain resume still **blocked-impl** until wired.
- Default script: 3 layers for D-COLOR-1.
- Publisher: flat `PUBLISH_MAX_HZ=1000` + GPU shader path.
