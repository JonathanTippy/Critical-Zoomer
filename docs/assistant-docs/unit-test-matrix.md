# Unit-test matrix (assistant-owned, non-authoritative)

Phase gate: every row `green`. Soft-skip GPU ≠ green. `D-*` decisions are oracles only when they do not contradict authoritative docs. D-PUB-1: flat 1000 Hz + GPU publisher (no min floor).

Status: `green` | `in-progress` | `blocked-impl`

## A — Tagged non-e2e

| Id | Tests (≥3) | Property | Verify | Status |
|----|------------|----------|--------|--------|
| cz.math.intexp-add-commutative+1 | prop + examples in `intexp.rs` | add commutative | intexp.rs | green |
| cz.math.intexp-mul-associative+1 | prop + examples in `intexp.rs` | mul associative | intexp.rs | green |
| cz.math.mandelbrot-real-axis-symmetry+1 | naive_cpu + e2e_oracle | conjugate symmetry | naive_cpu / e2e_oracle | green |
| cz.math.perturbation-naive-oracle+1 | perturb_cpu naive parity (≥3 loci) | exact Answer vs doubling naive | perturbation_cpu_worker | green |
| cz.range.guess-biased-nearest+1 | range.rs + calibrated + gpu_tile | stays in bounds | range.rs | green |
| cz.display.window-default-800x480+1 | constants.rs (≥3) | n/a | constants.rs | green |
| cz.display.offscreen-r2-circle+1 | offscreen.rs (≥3) | n/a | offscreen.rs | green |
| cz.display.offscreen-arrows+1 | offscreen classifier + window arrows impl | n/a | offscreen.rs | green |
| cz.display.nores-when-no-proximate+1 | sampling + gpu_tile + shade | n/a | multi | green |
| cz.tenacious.nores-not-flat-black+1 | b_ten_1 + shade | n/a | multi | green |
| cz.tenacious.no-max-iter+1 | tenacity + IPP origin (≥3) | finish without max-iter | perturb_cpu / standards_perf | green |
| cz.hoarding.one-answer-per-point+1 | sampling hoard_tests (≥3) | n/a | sampling.rs | green |
| cz.hoarding.no-compute-settings+1 | D-BAIL-1 recolor-only (≥3) | n/a | shade_tests | green |
| cz.calib.lowres-synthesis+1 | calibrated bias + publisher (≥3) | clamp/synth | multi | green |
| cz.fast.natural-zoom-2x+1 | inputs + transforms (≥3) | n/a | inputs.rs | green |
| cz.seamless.perturbation-always-on+1 | phase4 + session (≥3) | n/a | multi | green |
| cz.seamless.gpu-preferred+1 | gpu_context + perturb_gpu + uploader | n/a | multi | green |
| cz.seamless.reference-background+1 | reference_worker + session (≥3) | n/a | multi | green |
| cz.seamless.foveated-mag-velocity+1 | tile_scheduler + session (≥3) | n/a | multi | green |
| cz.system.tile-manager-protect-current-lookahead+1 | tile_manager (≥3) | n/a | tile_manager.rs | green |
| cz.system.max-homotheties+1 | tile_manager + sampling (≥3) | n/a | multi | green |
| cz.int.memory-bump+1 | tile_manager + sampling + unit extras | n/a | multi | green |
| cz.int.hoard-ingest-sample+1 | sampling (≥3 unit) | n/a | sampling.rs | green |
| cz.int.publisher-nores-bias+1 | tile_publisher (≥3) | clamp bounds | tile_publisher.rs | green |
| cz.int.publish-cadence+1 | cadence max 1000; no min floor (D-PUB-1 flat 1000) | n/a | tile_publisher.rs | green |
| cz.int.stencil-retarget+1 | unit extract (≥3) | n/a | stencil / window | green |
| cz.int.session-pipeline+1 | unit extract (≥3) | n/a | tile_session | green |
| cz.shade.escape-continues-to-bailout+1 | shade_tests (≥3) | n/a | shade_tests.rs | green |
| cz.shade.in-filament-slope-inversion+1 | shade_tests (≥3) | n/a | shade_tests.rs | green |
| cz.shade.out-filament-period-step+1 | shade_tests (≥3: lower/zero/highlight) | n/a | shade_tests.rs | green |
| cz.shade.node-smallness-minimum+1 | shade_tests (≥3) | n/a | shade_tests.rs | green |
| cz.shade.small-time-edge-nonzero+1 | shade_tests (≥3) | n/a | shade_tests.rs | green |
| cz.shade.layers-in-script-order+1 | shade_tests (≥3) | GPU↔oracle | shade_tests.rs | green |
| cz.perf.foveation-half-time+1 | tile_session (≥3) | 50/50 time | tile_session / standards_perf | green |
| cz.perf.home-100tps+1 | home fill release (≥3) | ≤5s | tile_session | green |
| cz.perf.home-10000tps-gpu+1 | GPU home TPS ≥10000 | ≥10k | standards_perf / e2e | blocked-impl |
| cz.perf.min-300m-ips-cpu+2 | standards_perf fullstack (≥3; outside r=2 + inside) + microbench | ≥300M | standards_perf | green |
| cz.perf.min-30b-ips-gpu+1 | standards_perf fullstack (≥3; outside r=2 + inside) + gpu microbench | ≥30B | standards_perf / perturb_gpu | green |
| cz.perf.optimal-ipp+1 | standards_perf (≥3) | escape IPP | standards_perf | green |
| cz.perf.play-minimize+1 | standards_perf play_* (≥3) | continuous delivery | standards_perf | green |
| cz.perf.play-8bump-100ms+1 | standards_perf play_eight_* (≥3) | ≤100ms | standards_perf | green |
| cz.perf.headgroup-shaders-2ms+1 | standards_perf (≥3) | ≤2ms | standards_perf | green |
| cz.perf.headgroup-vsync+1 | standards_perf (≥3) | Fifo | window / standards_perf | green |
| cz.perf.headgroup-stable-path+1 | sample+shade one path (≥3) | pan vs idle | standards_perf | green |
| cz.play.actor-poll+1 | PLAY_INPUT_POLL_MS + actor loops (≥3) | n/a | tile_worker | green |
| cz.play.actor-drain+1 | coalesce full-burst drain (≥3) | n/a | tile_worker | green |
| cz.play.latest-wins+1 | coalesce latest attention/retarget (≥3) | n/a | tile_worker | green |
| cz.ref.zero-orbit-same-path+1 | zero orbit + same path (≥3) | n/a | reference_actor / perturb | green |
| cz.pub.gpu-native-work+1 | gpu_tile handoff (≥3) | no CPU round-trip | gpu_tile.rs | green |
| cz.ctrl.zoom-in-homothety+1 | transforms (≥3) | pointer-fixed | transforms.rs | green |
| cz.ctrl.scroll-up-zooms-in+1 | inputs (≥3) | polarity | inputs.rs | green |
| cz.ctrl.hover-zoom-origin+1 | transforms pointer-fixed (≥3) | hover origin | transforms.rs | green |
| cz.ctrl.drag-anchor+1 | transforms drag bookmark (≥3) | zoom-back | transforms.rs | green |
| cz.ui.coords-parse+1 | coords.rs (≥3 parse forms + reject) | n/a | coords.rs | green |
| cz.ui.coords-apply+1 | apply enable + D-UI-1 (≥3) | n/a | coords.rs | green |
| cz.ui.location-readout+1 | viewport_center / ul_for_center (≥3) | mag+center | coords.rs | green |
| cz.ui.viewport-fill+1 | DEFAULT_WINDOW_RES + 1:1 resize (≥3) | n/a | constants / window | green |
| cz.cosmetic.layer-model+1 | settings layer model (≥3) | n/a | settings.rs | green |
| cz.cosmetic.defaults+1 | D-COLOR-1 exact three layers (≥3) | n/a | settings.rs | green |
| cz.fast.settings-100ms+1 | standards_perf (≥3) | ≤100ms | standards_perf | green |
| cz.fast.cosmetic-17ms-1080p+1 | standards_perf (≥3) | ≤17ms | standards_perf | green |
| cz.fast.scroll-10-in-300ms+1 | inputs (≥3) | debt | inputs.rs | green |
| cz.fast.no-tick-backlog+1 | inputs (≥3) | debt | inputs.rs | green |
| cz.fast.shift-space-5bps+1 | inputs (≥3) | ~5/s | inputs.rs | green |
| cz.fast.input-next-frame-17ms+1 | inputs (≥3) | same-turn | inputs.rs | green |
| cz.system.memory-default-1gb+1 | standards_perf (≥3) | 1e9 | settings | green |
| cz.cosmetic.bailout-range-2-255+1 | standards_perf (≥3) | [2,255] | settings | green |
| cz.deep.min-zoom-pot-capacity+1 | standards_perf (≥3) | pot | gear / intexp | green |
| cz.deep.snappy-at-depth+1 | deep pot + poll cadence (≥3) | snappy | standards_perf | green |

## B — Promoted from former REQ-* (now Tracey-linked)

Former untagged slices now carry `cz.*` ids in section A:

| Former | Tracey id |
|--------|-----------|
| REQ-CTRL-PARSE | `cz.ui.coords-parse+1` |
| REQ-CTRL-APPLY | `cz.ui.coords-apply+1` |
| REQ-CTRL-ZOOM | `cz.ctrl.hover-zoom-origin+1` / natural-zoom / scroll-up |
| REQ-COSMETIC-LAYER | `cz.cosmetic.layer-model+1` |
| REQ-COSMETIC-DEFAULT | `cz.cosmetic.defaults+1` |
| REQ-BAILOUT | `cz.cosmetic.bailout-range-2-255+1` + D-BAIL-1 |
| REQ-SYS-MEM | `cz.system.memory-default-1gb+1` + tile-manager protect |
| REQ-CALIBRATED | `cz.calib.lowres-synthesis+1` |
| REQ-DEEP-GEAR | `cz.deep.min-zoom-pot-capacity+1` + D-GEAR-1 |

## C — Decisions `D-*`

| Id | Tests | Status |
|----|-------|--------|
| D-COLOR-1 | default script length 3; kinds escape/in-fil/out-fil | green |
| D-COLOR-2 | layer field presence | green |
| D-COLOR-3 | alpha-over order | green |
| D-COLOR-4 | highlights in script list | green |
| D-BAIL-1 | recolor-only membership stable | green |
| D-SHADE-1 | INFIL threshold constant applied | green |
| D-SHADE-2 | NODE threshold constant applied | green |
| D-SHADE-3 | higher-period side only (≥3 shade_tests) | green |
| D-MEM-1 | exact bump | green |
| D-MEM-2 | slider moves | green |
| D-MEM-3 | packed bytes cost | green |
| D-MEM-4 | keep-set property | green |
| D-SCH-1 | screen_edge_complete gate (≥3) | green |
| D-SCH-2 | EWMA mag velocity (≥3 inputs) | green |
| D-SCH-3 | immediate preempt + resume_suspended on drain | green |
| D-PER-1 | twin N=`PERIOD_CONFIRMATION_ITERATIONS` (20) | green |
| D-PER-2 | relative ε | green |
| D-PER-3 | POT snapshots | green |
| D-GEAR-1 | no mid-tile escalate API | green |
| D-SERIES-1 | series_skip + absorption (≥3) | green |
| D-CANCEL-1 | cancel keeps hoard | green |
| D-REF-1 | +20 bits | green |
| D-REF-2 | retire last-user or >N=3 | green |
| D-PUB-1 | max 1000; no min floor; GPU publisher | green |
| D-PUB-2 | clamp all-numeric | green |
| D-STEN-1 | mouse+vel+seq fields | green |
| D-WORK-1 | address-only keys | green |
| D-UI-1 | apply enabled when equal | green |

## D — Property roster

| Surface | Property | Status |
|---------|----------|--------|
| IntExp add | commutative | green |
| IntExp mul | associative | green |
| StackedIntExp | agrees IntExp | green |
| FloatExp | production invariant | green |
| Range guess_biased | in bounds | green |
| Mandelbrot | conjugate symmetry | green |
| Tile manager | deterministic keep-set | green |
| Sampling (static tiles → stencil) | nearest + up-left; no remapping chains | green |
| Publisher clamp | within bounds | green |
| Shade GPU↔oracle | pixel agree | green |
| Work controller locals | enumerated helpers | green |
| Screen worker locals | enumerated helpers | green |
| Gpu uploader | bypass identity | green |

## Auth note

Developer cadence rule: flat **1000/s** ceiling (D-PUB-1). Auth `tile_publisher.md` still mentions ≥30/s — treat as stale until human edits; tests enforce flat 1000. GPU publisher shader is still required.

## Preflight (2026-07-31)

- GPU: `/dev/dri` present; `gpu_context` tests pass.
- Rebased onto origin `fast` (7ff5ef5): keep origin production budgets / GPU publisher path; retain unit-test matrix greens from `all green?` where still valid.
- D-REF-1: `reference_precision_bits` / `discrimination_bits_for_mag` locked; f64 orbit builder remains for interactive path.
- Period: D-PER-1 green at N=20 (`PERIOD_CONFIRMATION_ITERATIONS`).
- Tracey linking pass: standards.md in styx; shade-rules.md; product/standards gaps annotated; new ids honest `in-progress` / `blocked-impl` until ≥3 dedicated verifies land.
- Next phase after unit/integration verify on this tip: end-to-end testing.
