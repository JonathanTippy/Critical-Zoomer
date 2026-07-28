# Bug / todo stack (live)

Two lists: **true bugs** (incorrect behavior) vs **design gaps** (intended design not yet on the live path). Newest notes at the top of each open section. Do not clear an item until verified fixed (tests and/or visual).

PO quotes: `he-said/`. This file tracks status, loci, and follow-through.

## True bugs (open)

### B-DISP-1 — Phase 2 cutover display regressions (grey / ~15fps / no GPU escape)
- **Symptom:** Immediate display grey; ~15fps; no escaper on GPU; settings open greys window; location UI misplaced / no input box.
- **Mechanism (PO):** Headgroup must own a GPU tile collection and run sampler → escape → edge → shade shaders.
- **Status:** landing — shared-device grey fixed; shade/oracle parity; vsync Fifo; CPU Color32 path purged. **Still open:** headed fps re-measure; full coloring_script edge cases.
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
- **Locus:** `screen_worker/mod.rs`; `main.rs` capacities; `tile_session.rs`.
- **Quotes:** `he-said/scheduler-and-edges.md`

### B-PER-1 — Period banding (residual visual)
- **Done so far:** See `phase-1.5-notes.md`.
- **Status:** **reopened as B-PER-2** — deeper minibrot bands returned.

### B-ZOOM-1 — Apparent stall zooming into bulbs left of cardioid
- **Status:** **parked** — PO could not reproduce.

## Design gaps (open)

### D-GEAR-TYPED — Host batch still f64; stacked/FloatExp bout not fully monomorphized
- **Need:** Typed ActivePoint enum / Mandelbrotable bout for StackedI32 + AdaptiveRug (FloatExp). StackedI32 now dispatches GPU stacked bout pipelines; AdaptiveRug still CPU.
- **Status:** partial — StackedI32 GPU path live; typed host batch for AdaptiveRug still open.
- **Locus:** `perturbation_{cpu,gpu}_worker.rs`; `gears.wgsl` / `stacked_bout_tail.wgsl`


### D-PUB-GPU — GPU publisher required; CPU publish_seat is interim only
- **Need:** GPU shader that combines hoarded tiles with new calibrated work (NORES / proximate bias). Cadence flat **1000/s** ceiling while incomplete (D-PUB-1).
- **Status:** landing — GPU `publisher_shader` restored; `publish_tile` prefers GPU when available; single-seat CPU path remains for tests.
- **Locus:** `workgroup/tile_publisher.rs`, `workgroup/publisher_shader.rs`, `publisher.wgsl`.

### D-PER-1 — Period-determination phase after boundary + out-fill
- **Need:** After boundary tracing and out-fill complete, run a phase that determines periods of the **in-edge**. Regular iterate must stay certain (no false periods); unknown period is allowed until this phase. Out-filament rendering must not show an ugly boundary between period-unknown Inside and period-known Inside.
- **Status:** landing — `OutfillInfillScheduler::{needs,take,apply}_period_resolve*` + `TileSession::try_resolve_periods` after `screen_edge_complete()`. Flood-in period propagation wired (see D-SCH-2). Verify flood-in + out-filament seam visually.
- **Locus:** `tile_session.rs`; `outfill_infill_scheduler.rs`; shade out-filament.
- **Quotes:** `he-said/period-determination-phase.md`

### D-SCH-3 — Boundary-trace all renderable edges (not only in/out)
- **Need:** Scheduler is out-fill + boundary trace. Order: out-fill until a real boundary → trace **in/out**, **in-filament**, **out-filament** edges → finish out-fill → trace **small-time** edges. Also period-change edges; future **derivative magnitude angle** on Answers (in-filament detection outside workgroup). Do not time-slice inefficiently across phases.
- **Status:** landing — `in_filament_edge` / `out_filament_edge` / `small_time_edge` queues + ranks + PhaseJobTracker wire in `outfill_infill_scheduler.rs`. STE gated until out-fill complete. In-filament still needs derivative-magnitude-angle on Answers for full seed quality.
- **Locus:** `outfill_infill_scheduler.rs`; `tile_session.rs`; Answer angle field still future.
- **Quotes:** `he-said/scheduler-boundary-trace.md`

### D-SCH-1 — Full screen-edge seed missing on live tile path
- **Need:** Out-bucket-fill requires walking the **entire screen edge** first (prototype had it; live `TileSession` only seeds tile rims).
- **Status:** closed (gate) — `OutfillInfillSchedulerState::screen_edge_complete()`; `FloodIn` / `PeriodEdge` / `In` and `fill_remaining_in` gated until edge remaining is 0. TileScheduler still seeds tile-perimeter seats into screen scredge (not a pure outer-frame-only walk — residual vs prototype full-screen-edge-first). Tests: `d_sch1_tests::*`.
- **Locus:** `tile_session.rs`; `outfill_infill_scheduler.rs`; `tile_scheduler.rs`.
- **Quotes:** `he-said/scheduler-and-edges.md`

### D-SCH-2 — After screen edge walked: in-fill hints with propagated period
- **Need:** Hint-fill **in** areas as Inside with period from the edge of that area; still work points for min-magnitudes.
- **Status:** closed — `apply_period_resolved` → `queue_flood_in_neighbors` → `FloodIn` / `flood_in_fill` emits `inside_calibrated(period)` hints. Test: `d_sch1_tests::period_resolve_queues_flood_in_with_propagated_period`.
- **Quotes:** `he-said/scheduler-and-edges.md`

## Done (recent)

- Phase 1.5 period detector + shape tests (see `phase-1.5-notes.md`).
- Out-filament: `get_loop_period` skips `loop_period == 0` (Dummy).
- D-SCH-1 FloodIn/PeriodEdge/In gate + unit tests.
- D-SCH-2 flood-in period propagation after D-PER-1 resolve.
- B-TEN-1 NORES pack + shade missing→Outside escape-1 (`b_ten_1_tests`).
- Phases 3–6 landed in-tree (refs collection, perturb CPU off-by-default, naive GPU live, StackedIntExp/FloatExp minimal); Batch B–G recorded under `he-said/`.
- Home settle verified after period-resolve stall fix: `/tmp/cz_ctl_capture/verify_home.png` (~73 fps).
