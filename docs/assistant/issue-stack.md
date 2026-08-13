# Bug / todo stack (live)

**2026-08-06: codebase reverted to v0.0.9 (e6a0560).** The tile machine that generated most of this stack is no longer in the live tree. All tile-machine bugs and design gaps are **closed by revert** — their mechanisms (batches, tile versions, publisher gates, mag columns, GPU waits) do not exist at v0.0.9, which is exactly the point: see `docs/assistant/design/workgroup-virtues.md` for why those failure shapes have no handles in the golden design. Full tile-era detail: `Trash/issue-stack-tile-era.md`.

Two lists: **true bugs** (incorrect behavior) vs **design gaps** (intended design not yet on the live path). Do not clear an item until verified fixed (tests and/or visual).

PO quotes archive: `Trash/stale/less stale but still stale/grok-docs/he-said/`.

## Standing rule

Any new workgroup work (GPU, depth, perturbation) must not re-break the v0.0.9 invariants: one live target, one truth package, shared remap transform, small interruptible bouts, whole-snapshot publishes, fixed pivot order, no competing versions. A regression of these is a fail regardless of feature gain. Reference: `docs/assistant/design/workgroup-virtues.md` (enshrined).

**Living (2026-08-12):** headed default compute kernel is **Manual gear + Naive** (CPU DirectKernel). Naive GPU and Perturbation stay in settings; they are too buggy to run from the automatic PPS race. Uncheck Manual gear to restore that race.

**Quality grind (2026-08-09):** no soft-skip / soft floor / `#[ignore]` on hard bars (`quality-doctrine.md`). Headgroup/shadergroup edits outside location+HUD need a note here first (`headgroup-charter.md`). **Charter note (2026-08-11 shade unjam):** shadergroup GPU colorer persistent buffers + redundant-upload skip (not skip-send); f32 GPU escaper (R=2→bailout, resident answers) with manual escape gear + HUD `escape:`. **Charter note (2026-08-12):** escaper and colorer each own a wgpu device (per-actor GPU compartmentalization; no shared shade `shade_ops`). Dual-device did **not** fix live `esc:~9` / escaper 100% CPU — pin via dummy-head pipeline cadence tests (`steady_state_pipeline_cadence_*`). **RCA + within-actor opts (2026-08-12, not a headed fix):** channel not full; convert was ~35 ms (per-pixel stencil/`IntExp`) + GPU body ~21 ms (second pack walk). Stencil rule: O(1) touch per loop (two `IntExp`s grow at design depth). Convert hoist + same-walk `gpu_answer_pack` once printed dummy-head GPU **esc ~60 Hz** (body ~7 ms) — **snapshot / harness only**, not a standing rate (dummy-head GPU esc ≥40 on debug+opt-3 `cargo test`; unoptimized debug misses it). Strong escaper↔colorer actor API kept; cross-actor fusion parked unless unavoidable. **Headed `esc:` with `escape:GPU` not re-checked; do not treat live rates as fixed.** No lighting redesign. Workgroup TTFP / stops-on-GPU-color parked after this pass. **Charter note (2026-08-11 pipeline policy):** only screen worker parks; shade always runs on content beat; collector absorbs all WorkUpdates and always publishes on the content beat; `dirty`/`clean` tokens banned in `src`/`benches` via `build.rs`; deferred settings viewport kept. **Charter note (tick 16):** shadergroup `colorer` thought-kill only — pure helpers (`layer_colors`, neighborhood predicates, `safe_sample`); no lighting/policy redesign. **Charter note (tick 17):** shadergroup `escaper` thought-kill only — `diff`/`difff32` helpers. **Charter note (tick 26):** colorer thought-kill deepen only — `is_changed` / `slope_sign_changed` / get_* accessors; still no lighting redesign. Criterion `time_to_*` previously inflated by context construction (`b.iter`); fixed to `iter_custom` — first-publish “~96 ms vs ~40 ms” was largely measurement, not a DirectKernel slip. Pins: `home_workshift_first_publish_within_20pct_of_direct_kernel`, `home_workshift_full_frame_within_20pct_of_direct_kernel`. **PPS bar:** hard floor ≥1× CPU; aspiration ~160×. **Living llvm-cov:** `docs/assistant/coverage-baseline.txt` (region ~78.7% as of tip; regenerate via `scripts/coverage.sh`). Fuzz scaffolds: `fuzz/` (`coords_goto`, `admission_edges`). **Mutants (2026-08-10 night):** tip scoped `utils` verify finished — **352 caught / 2 missed (same mutant twice) / 42 unviable / 0 timeout** in 60m (`/tmp/cz_mutants_utils_tip`). The miss was `other << other_shift` → `>>` on the *old* dual-shift `Add` where `other_shift==0` makes `<<0`≡`>>0`. Tip already rewrote `IntExp` Add/Sub to shift only the higher-exp operand, so that survivor is gone structurally. Thought-kill also deepened across core math, policy, coords, Oracle, HUD, Delivery/`BoutCap`, colorer, sampling remap index, escaper diffs, `classify_motion` (zoom≻pan), work-collector identity remap, and naive `bailout`/`iterate_with_c`/`period_partials` (`mutant_kill_*`). **Scoped `cargo mutants -f src/gearbox/policy.rs` (tick 19):** 26/26 caught under `--lib mutant_kill_policy` (offline). Do not share the live `CARGO_TARGET_DIR` with mutants mid-suite — stale mutated test binaries can false-fail; rebuild/libtest after mutants. Isolated target dirs still fail closed on `steady_state` viz-lite download (tick 20 oracle attempt). Full crate cargo-mutants still too slow for routine ticks. Thought-kill also covers neighbor-queue bounds/delivered-skip and `verified_period(0)` (tick 20 pin `mutant_kill_queue_neighbors_and_verified_period_zero`; full suite + both Criterion benches green). **Tick 21:** edge detection / in+edge queues / square-ring geometry / `apply_generator_admission` Absolute↔Relative + undelivered invalidate (`mutant_kill_edge_spiral_and_generator_admission`); Oracle `conclude` starts at `z=c` + zero-bout Unfinished. **Tick 22:** gear HUD ranks / no demotion below `view_gear` / Mixed conflict, absolute+relative C helpers, `direct_completion` escape vs repeat, attention spiral reset-on-anchor-change (`mutant_kill_gear_coords_and_completion`); IntExp Display `exp==0` integer branch. **Tick 23:** pitch ε abs×1/256, bout loop/escape/`BoutCap`, smallness strict-min, spiral skip-delivered (`mutant_kill_pitch_loop_spiral_and_bout`); sticky-select filters escapes/undelivered + inclusive coverage corners + new-view precision. **Tick 24:** location goto parse (`a+bi`/comma/`i`, SetZoom≻SetPos, mag word-boundary) + `view_gear_from_generators` / `f64_delta_admitted` bounds. **Tick 25:** `dispatch_kernel` priority / PPS probe relative-lock / `invalidate_stale_deliveries` gen match (`mutant_kill_dispatch_pps_and_stale_invalidate`); FloatExp `new`/abs/square/ord deepen. **Tick 26:** colorer neighborhood `is_changed`/`slope_sign_changed`; sampling zoom−1 transform; verified_period minimal reduction (−1, period 4→2). **Tick 27:** range add/sub/square-through-zero/scalar-mul; snip 1×1 PPM channel order; `f64_stencil_admits` + HUD PPS/`floor_policy_label` (`mutant_kill_stencil_admit_and_hud_pps`). **Tick 28:** queue fallback priority (scredge vs edge/out/in) + `percent_completed` ×100 scale; naive GPU `pos_from_index` row-major + finish flag bits 2/4 + bulk publish; MOVE_SPEED_IN_SCREENS / SCROLL_SPEED exact pins. **Tick 29:** Stec LIFO/capacity + `struggling_to_clear` 2s PPS gate; coords `parse_mag_token`/`2**`/word-boundary split + readout sci thresholds; `iterate_complex` schoolbook + `points_near` inclusive box. **Tick 30:** FloatExp rug `from_rug`/`to_rug` + IntExp→FE `*2`/`exp-1`; `isqrt_u64` floor/perfect + loop-checkpoint `<<1` doubling gate. **Tick 31:** views stencil `index`/`seat_and_row`/`clamp` + half-open segment overlap/subset + `int_exp_is_integer`; naive GPU `SeatSkip` bitword insert/remove/contains. **Tick 32:** stencil `space`/`corners`/`bottom_right`/`correct_precision` (incl. `From<usize>` exp:1); `f32_collapses_neighbors` + `scan_undelivered` skip gates; `relative_location_from_index` `%`/`/`. **Tick 33:** reference_worker IEEE `f64_to_intexp` bits (sign/biased/implicit/subnormal); classify kernel priority + usable_ref/host_stack TypeId; FloatExp Ord zero/sign/magnitude reverse deepen. **Tick 34:** pert `near_fe`/`near_complex` inclusive ε boxes + `fe_pair`; `square_ring_offset` exact ring-1 walk; `IntExp::from(usize)` exp:1 vs i32. **Tick 35:** IntExp Sub/Mul/Shl/Shr/round/`set_precision`; index/pos/`signed_shift`/f32↔i16; pert `f64_period_check` inclusive ε + `saturating_mul(2)` doubling. Also: rug-doubling oracle craftsmanship pin allows ±1 escape-time at the bailout circle (FloatExp vs rug boundary; answer class still required). **Tick 36:** settings `Normalizing` arms/roundtrip + RecipLn=`ln(1/x)` (aligned `get_normalizer`); IntExp Ord/`zoom_from_pot`/Into; `transform_index` exclusive-zero bounds. **Tick 37:** `Animable::determine` wave/static + **start latch fix** (Copy-bound `mut start` was dead); `manual_gear_override` enabled gate; `ColoringInstruction::id`/id-only Hash. **Charter note (tick 37):** shadergroup colorer/escaper call sites only — `ref mut shading_method` / no-clone `bailout_radius.determine()` so the latch reaches live shade/escape paths; no lighting redesign. **Tick 38:** `get_points` +seat→+real / +row→−imag pitch + zoom_pot `>>`/`<<` polarity + `WORKER_INIT_ZOOM`=0.25. **Tick 39:** HUD `rolling_frame_calc` gates/push/avg/worst + fail-closed on missing `window_start` and empty-queue ÷0 (was unwrap/panic); worst scan is an explicit loop.

## DAT failures (2026-08-01) — product watchlist on the v0.0.9 baseline

Developer acceptance test failed on the tile machine. Most items were tile-era implementation failures (closed by revert). The items below are **product-level**: they describe things v0.0.9 already did right, and they must keep working on the restored baseline. Verify headed before DAT.

- coloring options — v0.0.9 had the full layer menu; keep it complete (the tile era lost most kinds; do not repeat).
- normalization / period animation NaN — v0.0.9 is the golden oracle (RecipLn = ln(1/x)); GPU port must match it.
- animated / configurable bailout — v0.0.9 had it working; keep it working.
- intexp display and magnification in location readout — v0.0.9 displays these; keep the format clean.
- goto Apply accepts the HUD's own readout format and lands the same from any view (B-GOTO-1/2 lessons; v0.0.9-era coords behavior is the baseline to check).
- worker must never die silently and never constipate (B-PIVOT-1's lesson): at v0.0.9 the worker is a local 10ms-shift loop with hard-seat rotation — keep it that way; any GPU/depth extension must preserve interruptibility (virtues §4).
- zoom response: new work starts immediately on pivot at v0.0.9 (replace + remap); the 1–2s play regression was tile-era retarget machinery. Any successor must match the old latency, not "improve toward" it.
- unfinished pixels must never look like finished flat black: v0.0.9 fills from remap / provisional edge answers / Dummy-interior default; keep an honest-incomplete signal in any GPU port.

**Charter note (2026-08-11 cadence pass):** headgroup present vsync default +
Settings cadence knobs (`content_refresh_*`, `head_vsync_enabled`,
`head_max_fps`, `auto_vsync_hz`); Settings fan-out to collector; content-tier
`wait_periodic(resolved_content_period)` on collector/escaper/colorer; no
actor skip-send; GPU paint always refreshes from current inputs
(persistent buffers kept). HUD `pub:/esc:/col:/ctrl:` emission-Instant rates
already landed. Escape gear stays OG default. Workgroup TTFP / Replace coalesce
still parked. **Charter note (2026-08-11 settings viewport):** deferred native
settings viewport kept; settings viewport `request_repaint_after(100ms)`;
auto_vsync fan hysteresis ≥2 Hz with multi-present debounce. **Do not** disable root GL swap Wait to paper
over dual-viewport Wait (that spun ~1500 FPS and unparked the worker). Display
timing only — no lighting redesign. **Charter note (2026-08-11 post-settle
park):** after fill, attention/settings/size flicker can keep window+worker
hot for seconds; park wait excludes attention + periodic timer; stencil/attention/
vsync send gates hardened; `steady_state_home_stays_parked_for_10s_after_fill`.

## True bugs (open)

- **Transition rectangular blockiness / shallow false admit (2026-08-12 design RCA).**
  When a deeper gear exists but the image shows rectangular precision blocks,
  suspect **C-generator false-admit of a shallow type** because admit only checks
  neighbor distinguishability and **omits ~10 bits of render headroom**. Gear =
  compute kernel; type = cheapest admitted representation inside that gear;
  gearbox = max expected PPS among admitting gears. Margin belongs in C-gen on
  both absolute `c` and δc paths. Interview:
  `interviews/2026-08-12-precision-wall-gear-switching.md`; paraphrase:
  `paraphrase-authoritative/c-generator-admit-margin.md`.
  **Code in tree (2026-08-12), product not verified.** Default 10-bit admit
  margin (`space / 2^margin` probe) plus settings `c_generator_margin_bits`
  (0–32) for manual testing. Mechanism pins exist; **do not treat transition
  blockiness or “gear kaput” as fixed** until the developer says so. Third
  shape (interview 2026-08-12): C-gen admits honestly, then a **later layer
  drops precision** (classic: f64 interlayer) → same rectangular look.
  Naive **black wrong-interior** remains a periodicity tangent.
- **Precision wall / gear:F64 at ~pot 43–48 — HUD floor on live path (2026-08-09),
  product blockiness not closed.** Headed #5: HUD stayed `gear:F64` past the
  f64 wall. Code: ScaledF64 `view_gear` floor; pin
  `deep_view_gear_floor_stays_scaled_after_fill`. **Do not treat rectangular
  transitions or post-admit f64 drop as fixed.** See true-bug blockiness item.
- **B-SCH-3 home banding — fixed by f64 restore (2026-08-07).** Rectangular black columns at home (`1.3359375 + 0.125i mag 2^-2`) were caused by the FloatExp live-actor experiment. Restoring f64 production (worker→collector channel, perturb kernel, workshift, controller gate) eliminates the banding; `tmp/capture_at.sh` reports `black_cols_80=0`, matching known-good `ea27b4f`. The smaller `relative_location_from_index` divisor fix (data_res.0) is kept.
- Re-verify headed: resize, settings layers, bailout slider. (2026-08-07: home render, scroll zoom, and drag verified normal headed after the kernel-seam + reference-actor wiring.)
- **Phase-two home render corruption — closed under readiness wait (2026-08-07).** Earlier captures of giant black disk / rectangular discontinuity / flat purple were either pre-ready frames (purple: zero gray holes falsely treated as filled) or superseded by escaped-reference rejection + settled capture. Settled Xvfb home now shows coherent Mandelbrot structure; `e2e_visual.sh` passes with structure+baseline readiness (crop RMSE ~9.5k ≤12k). Keep the readiness gate; do not regress to gray-hole-only completion.
- **Uncovered sticky reference → classic glitch blobs (2026-08-07) — superseded by liberal reuse (2026-08-10).** Earlier fix dropped off-screen sticky refs via `reference_c_covers_frame`. Product policy now is **greedy keep** + **best ref per seat** + **local glitch → pause seat until better ref** (never discard refs for glitch; `docs/assistant/paraphrase-authoritative/reference-reuse.md`): off-screen interiors stay selected/carried; `reference_library` + `best_reference_for_c`; pins `sticky_selection_keeps_interior_outside_new_view`, `from_stencil_keeps_offscreen_sticky_reference`. Coverage helper remains diagnostic only. Discard/byte-budget still deferred. Headed corroboration: `scripts/screenshot_check.sh`. Short incomplete center refs can still disagree with DirectKernel — production must not publish truncated orbits (`r[cz.depth.reference-until-done+1]`).
- **Finished-frame busy-spin — fix landed.** `percent_completed` was derived from the empty-queue break index (`(N-1)/N*100`), so completed frames never idled. Now delivered-fraction. Pin via never-stall / load-proportional follow-up.
- **Harness zombie processes — fixed (2026-08-07).** Repeated Xvfb captures left orphaned `critical_zoomer` / `Xvfb` processes. `tmp/capture_at.sh` now has an `EXIT` trap that stops the session and kills session-scoped app/Xvfb processes.
- **Short published orbit soft-stall / iteration wall — fix landed.** Library: missing iterate = `Unfinished`, not Glitch. Seats soft-continue on zero orbit (`δz ← z`). Reference worker no longer publishes at an artificial `max_iterations`/`MAX_BOUT` length wall — only period or escaped (`r[cz.depth.reference-until-done+1]`).

## Known issues (open)

- **Open (ghost-hunt 2026-08-12): head window ~100% CPU.** Not shelved.
  Screen **worker** parks after fill (`seats_need_work`) — that is the wrong
  actor. Window `update` still does bare `ctx.request_repaint()` (`351afdf`).
  `NativeOptions.vsync` is true. Head vsync / max-FPS **settings UI** was
  removed (fields remain; they do not pace present). Developer: ~100% CPU
  at vsync-looking frame rates. **Not fixed.** See
  `design/pipeline-refresh-rates.md` and `ghost-hunt-2026-08-12.md`.

- **Resolution / ~1.5× default pixels — revealed issues (2026-08-11).**
  Past ~1.5× `DEFAULT_WINDOW_RES` / at 1080p:
  (A) **shadergroup too slow** — colorer problem child (`shadergroup_fitness`).
  **Landed (2026-08-11):** manual OG↔GPU color gear + honest f32 wgpu colorer
  with exact `Color32` parity (`gpu_matches_og_*`). **Colorer default is GPU**
  (`resolved_color_gear`). Escaper stays OG. Headed 1080p feel still open.
  (B) **workgroup unfinished bands** — Stec deleted; growable Vec → channel.
  **2026-08-11 RCA / fix:** undeliver-on-full reopened Dummy (black streaks);
  replaced by `wait-on-channel-full` (worker waits for collector). Collector
  speed remains secondary.
  (C) **Parked (revealed after banding):** time-to-first-work and work-update
  post rate degrade past ~1.5× / 1080p — revisit after GPU colorer makes the
  shade path fast enough to expose them cleanly.
  (D) **Collector / publish / shell O(pixels) — diagnosed 2026-08-12.** Headed
  `pub:`/`ctrl:` ~15 at 854×480 under motion, →0 at 1080p. Controller is
  stencil-only (~80 B); `ctrl:` couples to collector publish stamps, not
  controller loop cost. Release probe: same-res `from_stencil` ~13 ms / ~76 ms;
  `sample_old_values` ~24 ms / ~97 ms; publish clone+`view_from_package`
  ~10 ms / ~151 ms. Options (Arc frozen snapshot, sparse integrator for remap
  lineage, shell reuse) in `design/collector-publish-bottleneck.md`. Not
  implemented. Do not “fix” with shared-mutable Views or delta shade protocols.

**Charter note (2026-08-11 GPU colorer):** bucket-3 honest rewrite of
shadergroup colorer (wgpu f32, feature parity, exact Color32 vs OG) + bucket-2
HUD `color:` + settings color gear (**GPU default** 2026-08-11). Escaper stays OG default.
No workgroup TTFP/update-rate work in this chunk.
- **Out-filament highlighting absent where verification is difficult.** After period correctness fixes, cloudy false positives are gone, but difficult areas can remain period 0 (unknown); unknown periods correctly create no out-filaments, so highlighting is absent there. Do not fix by publishing guessed periods. The resolution is stronger verification/continuation so difficult interior points eventually get verified periods.

**Charter note (2026-08-11 interview):** shadergroup/headgroup HUD — extract
`escape_frame`, shade drop counters + HUD `drop:`, Criterion
`shadergroup_fitness`; document single-path virtue. Bucket 3 display audit +
bucket 2 telemetry.

## Design gaps (open)

- **Series approximation live (2026-08-11).** Performance-minded rewrite landed:
  flat coeffs, `SeriesBuilder` fused into `ReferenceOrbit::extend_inner`,
  O(log N) `safe_skip` (large `|δc|` immediate no-op), always-on
  `apply_series_skip` after `init_delta` in both pert kernels. Prior linear/heap
  sketch replaced. Rule `r[cz.depth.series-approximation+1]` is on the production
  path. Pins: `series_approximation_wired_into_production_kernels`,
  `series_safe_skip_eval_count_is_logarithmic`,
  `series_shallow_probe_stays_nearly_free`,
  `series_deep_skip_is_material_on_long_orbit`,
  `live_series_skip_initializes_delta_prefix`, membership pins with SA on.
- **Deep exterior black/"in" vs stuffed δc (2026-08-08) — that production bug
  closed.** Flip-flop from stuffing generator `delta_c` into the zero-orbit δc slot (false interior) vs iterating collapsed f64 absolute `c` without a reference (blocky). Production bug: `workshift` called `perturbation_reference_active()` *after* `latest_reference.take()`, so the held orbit was always dropped and seats iterated zero-orbit with generator `delta_c` → false `repeats` at iters≈2 (flat black) while HUD still showed `mode:pert`/`ref:complete`. Fix: decide publish-orbit use from the held snapshot. Pins (workshift path): `pin_exterior_not_marked_in_at_zoom_52`, `pin_not_blocky_delta_c_at_zoom_49`. Soft-continue still uses absolute `c` in the δc slot. **Not** v0.1 headed blockiness/depth-trust.
- **Depth integration — finish-line gates (2026-08-07), product trust open.**
  Representability / series / HUD gear+IPS+PPS / home pert parity vs
  DirectKernel landed. **Do not read as v0.1 depth-trust done.** Rectangular
  blockiness still open (admit-none, false-shallow, or **post-admit precision
  drop**). Live host is f64 (`WorkUpdate<f64>` pinned); that interlayer is
  a named failure shape. FloatExp-host banding remains open and is **not**
  moot if f64 host is the drop. Interview:
  `interviews/2026-08-12-v01-product-direction.md`.
- **First reference job length still = `MAX_BOUT` (1000); no mid-view extend.** **Closed (2026-08-07):** publish only on period/escape; no length wall. Intermediate snapshots before done remain an open question (see depth-design).
- **Reference library reuse landing; discard still open.** Greedy keep + per-seat best-ref bind are in; byte-budgeted eviction / unused-ref discard are not.
- **Display-path latency profiling.** Partially superseded 2026-08-12 for the
  workgroup publish path: see Known issues (D) and
  `design/collector-publish-bottleneck.md` (controller audit + collector/shell
  microbench). Remaining open: headed per-stage cost at 1920×1080 for escaper,
  colorer, window sampling, texture upload, and repaint cadence — still do not
  blame view/remap alone for shade-side lag.
- **Collector publish throughput / remap lineage (design gap 2026-08-12).** Dense
  absorb-all + full-package clone every content beat is correct for continuity
  and whole snapshots, but O(pixels)×pivots when behind. Tip-only dense remap
  is not free (lineage carries intermediate seats through smear). Preferred
  directions discussed: frozen Arc/pool snapshots first; sparse integrator if
  lineage must stay without full-frame hops; same-res Point buffer reuse on
  worker. Doc: `design/collector-publish-bottleneck.md`. No code until chosen.
  Context closed 2026-08-12 still **not implemented**.
- **Pipeline refresh rates (ghost-hunt 2026-08-12).** Content-tier cadence
  **did** land (timers, absorb-all, HUD rates). Head present did **not**:
  live `VSYNC=true` + bare `request_repaint` (`351afdf`). Older note
  “`VSYNC=false` + no code until big plan” is stale. Window CPU still open.
  Docs: `pipeline-refresh-rates.md`, `ghost-hunt-2026-08-12.md`.
- **Color gear (OG ↔ GPU) — landed; GPU default 2026-08-11** (manual still
  forces OG/GPU; HUD `color:`). Escape gear remains OG default (`escape:`).
  Auto gearbox must still never pick GPU shade. Interview on cadence:
  `interviews/2026-08-11-actor-layout-frame-pacing.md`.
- **GPU port of the golden design** (`docs/assistant/design/design-target.md`, `docs/assistant/design/naive-gpu-design.md`): views not tiles, full remap of old work, v0.0.9 semantics on GPU. **First Naive GPU pass landed 2026-08-08** (wgpu island, F32+optional F64, wave workshift, sparse harvest into collector). Live wire-up verified: Xvfb HUD `mode:naive-gpu`, F64 on Vulkan when available; init off actor thread + backend fallback. **Iterate-heavy FLOP→IPS probe met 2026-08-08** (sparse fullstack ≥0.80 of compute/header-only after warmup; earlier ~12× was first-submit latency). **2026-08-09 live fixes:** HUD `gear:` now reports real GPU precision (`F32`/`F64`, not hardcoded F64); auto F32→F64 escalate when neighbors collapse; clear finish accumulators after harvest (was re-applying finals every wave → felt ~CPU); adaptive bouts for shallow vs iterate-heavy; **actor `iterations_delta` no longer subtracts prior-shift totals (HUD IPS was near-zero after shift 1)**; finish copy covers full WIP (home shallow floods). Steady-state IPS path tests: `steady_state_*` in craftsmanship_tests (`docs/assistant/testing.md`). **2026-08-09 home blotches:** F32 GPU period-detect could false-mark shallow exterior seats as interior (black speckles); host f64 confirms low-iter repeats before publish. **IPS/PPS tracking:** actor now emits WorkUpdate when `iterations_delta > 0` even with zero completions (iterate-heavy was dropping HUD IPS). HUD PPS = collector `points_delta` → `RateCounter`. **PPS bar (quality grind):** hard floor ≥1× CPU (`steady_state_home_pps_gpu_vs_cpu_ratio`); aspiration still ~160×. Do not “fix” by skipping queues, CPU mop, or soft floors. Pins: `steady_state_naive_gpu_home_neighbor_queues_grow`, `…_fills_without_cpu_mop`, `…_no_dummy_holes`. Continuous + deep-cusp green. Still must not re-break `workgroup-virtues.md`.
- **Certified `Boundary` completion state.** Dyadic pixel centers can only hit algebraically certifiable boundary parameters: exact parabolic points via rational cycle/multiplier checks, and Misiurewicz points via exact preperiodic repetition. Add a third completion state and separate coloring; do not impose an app effort cap. Explicitly deferred from the perturbation-core round, not forgotten.
- **Lookahead/hoard across mags**: v0.0.9 remaps one screen only. The tile era's thin-tower lookahead failed by fragmenting the truth store; any future lookahead must extend the remap discipline, not replace it (virtues §3, §11).
- **PPS-selected kernel (naive vs pert)** (`r[cz.perf.pps-selected-kernel+1]`): **landing** — PPS race among legal kernels (Naive / Naive GPU / Pert); no GPU-first assumption; lock highest measured PPS; **one-workshift trials (~10ms), re-open every ~500ms** so slowing gears (esp. Naive GPU) can lose without continuous / janky cycling (`pps_probe_locks_highest_measured_kernel`, `pps_probe_reevaluates_after_interval`).
- **C-generator ~10-bit admit margin (2026-08-12) — mechanism in tree, bug open.**
  Default margin in `CGenerator::new_with_margin` / `admit_generator_with_margin`;
  settings slider `c_generator_margin_bits`. Rule text under
  `r[cz.depth.c-generator-fails-closed+1]`. Transition blockiness is still an
  open true bug until headed/product confirmation. See
  `c-generator-admit-margin.md`.
- **Headgroup/shadergroup test strategy** (open problem): the workgroup now has property tests bound to its craftsmanship rules; the headgroup does not. The screenshot harness is the only net for visual bugs but needs use on every edit and image-description trust is imperfect; oracles can rot when output legitimately changes; the only known visual property so far is real-axis reflection symmetry. Needs a stronger strategy before the GPU shade port — the shadergroup was cut back last time partly for lack of tests. In-app PPM snip (`snip.rs` / `CZ_SNIPREQ`) is a start for faux-user paths.   Workgroup membership pins: `pin_exterior_not_marked_in_at_zoom_52`, `pin_not_blocky_delta_c_at_zoom_49`. Series package oracles are live with SA (`r[cz.depth.series-approximation+1]`). Paint/headgroup still screenshot-only.
- **HUD truth (2026-08-08):** metrics top-left show stack/mode/ref/gear; location+goto
  panel bottom-right (`coords-parse+2`, `location-readout+2`); mode flips
  `naive`→`pert` when reference installs; ref stays `complete` with reused refs,
  `wip` on cold start or post-glitch until newer generation installs.

## Salvage ports (from `salvage-from-code.md` — detail and evidence there)

Tile-era code worth porting, in suggested order. Delete each entry when ported.

- **PPS counter** (`TpsCounter` in rolling.rs; feed = point counts in `update_sampling_context`; HUD string at window/mod.rs).
- **Coloring-layer add/remove + settings UI fixes** (settings.rs `template` factory; widgetize period-slider and animated-bailout change-detection fixes).
- **Zoom-debt input feel** (inputs.rs scroll/key debt accumulators; skip the mag_velocity helpers).

## Cleanup (from `docs/assistant/design/workgroup-virtues.md` §12 — the honest 10%)

Not bugs; provisional mechanisms that shipped because they beat nothing. None is load-bearing.

- **Delete token accounting** in the screen worker (`workshift.rs` / `screen_worker/mod.rs`): the budget check in the shift loop is commented out, wall-clock is the only law; the token fields and `spent_tokens_today` recomputation are dead code.
- **`Stec` removed (2026-08-11).** Completion staging is a growable per-shift
  `Vec` drained LIFO into the collector channel. Channel-full → worker waits
  (`wait-on-channel-full`); no Dummy reopen. Fixed-cap Stec / double-queue
  staging deleted per interview.
- **Delivered-aware attention sampling**: done as the attention square-ring spiral (`cz.craft.attention-spiral+1`).
- **Incremental WorkContext construction**: channel side done (stencil-only Replace +
  lazy `ensure_started`). Remaining: same-res worker shell still O(pixels)
  `resize_with` placeholders — see Known issues (D) /
  `design/collector-publish-bottleneck.md`.
- **Completion staging vs channel**: staging Vec is only for per-shift batching + LIFO; backpressure is the channel.

## Done (recent)

- Perturbation delta kernel (milestone 2) — **closed for correctness** (gear ladder,
  series, glitch handling). Production dispatch migrating to PPS-selected naive vs pert
  (`r[cz.perf.pps-selected-kernel+1]`); `DirectKernel` is production naive path and
  test oracle.
- In-filament detection now carries the Mandelbrot derivative through remap and extrapolates
  the escape field across the four screen neighbors before applying the existing one-pixel
  peak test. Derivative, convergence, and 2x/4x ridge-survival tests are green. Pending headed
  visual verification that interim zoom frames retain the expected thin filaments. The same
  data-vs-screen defect remains open for small-time node highlighting.
- Stencil-only Replace: controller sends `frame_info` only; worker builds an uninitialized
  shell and materializes seat `c`/`z`/`dc` from `CGenerator` at first start. Reuses points /
  mixmap / completion buffers across pivots. `time_to_first_publish` −42%; full-frame unchanged.
- Attention-first square-ring spiral owns slot 0; `Option` attention (`None` = pointer
  off-screen → center anchor). Full-frame −29% vs prior; first-publish tradeoff noted in
  benchmarks.
- View remap associativity fixed: large-zoom remaps select source pixels from absolute plane
  positions; the saved `(0,513)` regression and generated associativity property are green.
- Period refinement replaced by the atom-domain candidate → Newton attractor → multiplier-test
  pipeline (`verified_period`); full-frame benchmark improved from 12.30 s to 234 ms with the
  same 10,302,563 counted iterations.
- Codebase reverted to v0.0.9 (e6a0560); tile-era code preserved in stash "tile-era code WIP before v0.0.9 revert"; `cargo check` green.
- v0.0.9 workgroup study enshrined: `docs/assistant/design/workgroup-virtues.md`.
- Tile-era design/unit-design/experiments moved to Stale; developer decisions annotated standing vs suspended.
