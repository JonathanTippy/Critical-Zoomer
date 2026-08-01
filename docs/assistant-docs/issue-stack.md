# Bug / todo stack (live)

Two lists: **true bugs** (incorrect behavior) vs **design gaps** (intended design not yet on the live path). Newest notes at the top of each open section. Do not clear an item until verified fixed (tests and/or visual).

PO quotes: `he-said/`. This file tracks status, loci, and follow-through.

## DAT failures (2026-08-01)

Developer acceptance test failed. Items below are verbatim from DAT.

- worker panics and never recovers
- sampling bug (?), some tiles are the NORES values for no reason
- thin towers of one tile going up / down (lookahead / hoard) are missing, resulting in extreme overuse of the nores value
- when zooming, new work takes a good 1-2s to start (play, regression)
- even though TPS is better than before, it is not near the target.
- intexp values are not displayed properly
- animated bailout is not running & configuable bailout not working
- regression: coloring options from v0.0.9 are almost all removed
- regression: normalization breaks period animation with NAN (see v0.0.9 for golden. Use it as an oracle, but shader must be on GPU.)
- previous work is not re-emitted in WIP tiles; incomplete parts of new tiles are the NORES value.
- precision wall at mag 20 (depth requirement fail)
- magnificiation missing from current location display

## True bugs (open)

### B-DISP-1 — Phase 2 cutover display regressions (grey / ~15fps / no GPU escape)
- **Symptom:** Immediate display grey; ~15fps; no escaper on GPU; settings open greys window; location UI misplaced / no input box.
- **Mechanism (PO):** Headgroup must own a GPU tile collection and run sampler → escape → edge → shade shaders.
- **Status:** landing — shared-device grey fixed; shade/oracle parity; vsync Fifo; CPU Color32 view path removed (GPU sample→shade only). **Still open:** headed fps re-measure; full coloring_script edge cases.
- **Locus:** `headgroup/window/{mod,sampling,gpu_display}.rs`.

### B-PER-2 — Period bands in deeper minibrots (regression / thought-fixed)
- **Status:** landing — regular Inside answers emit `period == 0`; max-iter force-finish removed from perturbation bout. Deeper minibrot visual confirm still needed.
- **Locus:** `perturbation_*_worker`; `tile_session::try_resolve_periods`; `shade.wgsl`.

### B-STE-1 — Small-time edges (interior tree missing / stray lines)
- **Status:** landing — STE scheduled after out-fill; shade/oracle STE coverage. Visual confirm interior tree without zoom still needed.

### B-SCH-3 — Regularly shaped incomplete (black) bands
- **Symptom:** Rectangular / striped unfinished regions.
- **Mechanism (PO):** Structural — worker→collector pipes backing up is not acceptable as a product state. Requeue-on-blocked is a visibility/mitigation only, not the fix.
- **Status:** unparked — Phase 2 tile path is live; publish/upload channels raised to 512 and control channels to 64 to remove artificial backpressure. Re-verify headed rectangular bands.
- **Locus:** `tile_worker/mod.rs`; `main.rs` capacities; `tile_session.rs`.
- **Quotes:** `he-said/scheduler-and-edges.md`

### B-PER-1 — Period banding (residual visual)
- **Done so far:** See `phase-1.5-notes.md`.
- **Status:** **reopened as B-PER-2** — deeper minibrot bands returned.

### B-ZOOM-1 — Apparent stall zooming into bulbs left of cardioid
- **Status:** **parked** — PO could not reproduce.

## Design gaps (open)

### D-GEAR-TYPED — Host batch still f64; stacked/FloatExp bout not fully monomorphized
- **Need:** Typed ActivePoint enum / Mandelbrotable bout for StackedI32 + AdaptiveRug (FloatExp). StackedI32 now dispatches GPU stacked bout pipelines; AdaptiveRug stays CPU (auth).
- **Status:** closed — `ActiveGearWork` holds typed arms (F32/F64/Stacked1..=8/FloatExp); CPU bouts are `iterate_perturbation_bout_typed<T>`; gear refresh drops batches on identity change. Residual: GPU still bridges through host f64 point buffers for upload.
- **Locus:** `active_gear_work.rs`; `perturbation_{cpu,gpu}_worker.rs`; `gears.wgsl` / `stacked_bout_tail.wgsl`

### D-GEAR-ARRAY — `[i32;N]+exp` CPU-only array gear
- **Need:** Auth lists array gear; N unspecified; auth “12 stack” vs enumerated 11.
- **Status:** blocked on developer specifying N / reconciling count. Explicit design hole in `gear.rs`.
- **Locus:** `docs/design/tile_worker.md`; `src/gear.rs`

### D-PUB-GPU — GPU publisher required; CPU publish_seat is interim only
- **Need:** GPU shader that combines hoarded tiles with new calibrated work (NORES / proximate bias). Cadence flat **1000/s** ceiling while incomplete (D-PUB-1).
- **Status:** landing — fake 30 Hz publish floor removed; LivePublisher flat 1000 owned by tile publisher actor; settings (memory) wired to publisher. GPU `publisher_shader` preferred; host bridge until atlas bind is complete.
- **Locus:** `workgroup/tile_publisher.rs`, `workgroup/publisher_shader.rs`, `publisher.wgsl`.

### D-ACT-1 — Workgroup SteadyState actor layout vs auth
- **Need:** Live telemetry graph matches auth workgroup sub-actors (not work controller / screen worker).
- **Status:** closed — registered actors: tile scheduler, tile worker, intratile scheduler, reference worker, gpu uploader, tile publisher. Reference receives whole-screen stencils from the headgroup (not via scheduler). Glitch = zero-orbit fallback only (no rebase).
- **Locus:** `main.rs`; `tile_scheduler_actor.rs`; `tile_worker/`; `intratile_actor.rs`; `reference_actor.rs`; `headgroup/window/mod.rs`.

### D-PER-1 — Period-determination phase after boundary + out-fill
- **Need:** After boundary tracing and out-fill complete, run a phase that determines periods of the **in-edge**. Regular iterate must stay certain (no false periods); unknown period is allowed until this phase. Out-filament rendering must not show an ugly boundary between period-unknown Inside and period-known Inside.
- **Status:** landing — `OutfillInfillScheduler::{needs,take,apply}_period_resolve*` + `TileSession::try_resolve_periods` after `screen_edge_complete()`. Flood-in period propagation wired (see D-SCH-2). Verify flood-in + out-filament seam visually.
- **Locus:** `tile_session.rs`; `outfill_infill_scheduler.rs`; shade out-filament.
- **Quotes:** `he-said/period-determination-phase.md`

### D-SCH-3 — Boundary-trace all renderable edges (not only in/out)
- **Need:** Scheduler is out-fill + boundary trace. Order: out-fill until a real boundary → trace **in/out**, **in-filament**, **out-filament** edges → finish out-fill → trace **small-time** edges. Also period-change edges; derivative magnitude angle on Answers (in-filament detection). Do not time-slice inefficiently across phases.
- **Status:** landing — queues + ranks live; Answers carry `escape_time_angle` / `min_magnitude_angle`; sharp discontinuities seed `in_filament_edge_queue`. Shade may still use neighbor heuristics until angles are dense everywhere.
- **Locus:** `outfill_infill_scheduler.rs`; `tile_session.rs`; `Answer` angle fields; CPU workers.
- **Quotes:** `he-said/scheduler-boundary-trace.md`

### D-SCH-1 — Full screen-edge seed missing on live tile path
- **Need:** Out-bucket-fill requires walking the **entire screen edge** first (prototype had it; live `TileSession` only seeds tile rims).
- **Status:** closed (gate) — `screen_edge_complete()` gates FloodIn / PeriodEdge / In until screen-border remaining is 0. Residual: TileScheduler may still seed tile-perimeter seats into scredge (not a pure outer-frame-only walk). Tests: `d_sch1_tests::*`.
- **Locus:** `tile_session.rs`; `outfill_infill_scheduler.rs`; `tile_scheduler.rs`.
- **Quotes:** `he-said/scheduler-and-edges.md`

### D-SCH-2 — After screen edge walked: in-fill hints with propagated period
- **Need:** Hint-fill **in** areas as Inside with period from the edge of that area; still work points for min-magnitudes.
- **Status:** closed — `apply_period_resolved` → `queue_flood_in_neighbors` → `FloodIn` / `flood_in_fill` emits `inside_calibrated(period)` hints. Test: `d_sch1_tests::period_resolve_queues_flood_in_with_propagated_period`.
- **Quotes:** `he-said/scheduler-and-edges.md`

## Done (recent)

- B-ZOOM-CTRL: scroll polarity, mag-velocity EWMA, same-mag pan keep, no clear_tiles on Home/goto. Unit/control path verified; headed `e2e_controls.sh` needs `xvfb-run` (not in this environment).
- Work-speed: resident GPU bouts + multi-dispatch/single harvest; home GPU fill probe ≤8s; full-stack IPS bars; shade ≤2ms via nores clear+scissor; progressive Agnostic WIP; AdaptiveRug FloatExp CPU bout. `cz_ctl` no longer auto-forces CPU bouts on Xvfb.
- Phase 1.5 period detector + shape tests (see `phase-1.5-notes.md`).
- Out-filament: `get_loop_period` skips `loop_period == 0` (Dummy).
- D-SCH-1 FloodIn/PeriodEdge/In gate + unit tests.
- D-SCH-2 flood-in period propagation after D-PER-1 resolve.
- B-TEN-1 NORES pack + shade missing→Outside escape-1 (`b_ten_1_tests`).
- Phases 3–6 landed in-tree (refs collection, perturb CPU off-by-default, naive GPU live, StackedIntExp/FloatExp minimal); Batch B–G recorded under `he-said/`.
- Home settle verified after period-resolve stall fix: `/tmp/cz_ctl_capture/verify_home.png` (~73 fps).
