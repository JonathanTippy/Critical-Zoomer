# Bug / todo stack (live)

Two lists: **true bugs** (incorrect behavior) vs **design gaps** (intended design not yet on the live path). Newest notes at the top of each open section. Do not clear an item until verified fixed (tests and/or visual).

PO quotes: `he-said/`. This file tracks status, loci, and follow-through.

## True bugs (open)

### B-DISP-1 — Phase 2 cutover display regressions (grey / ~15fps / no GPU escape)
- **Symptom:** Immediate display grey; ~15fps; no escaper on GPU; settings open greys window; location UI misplaced / no input box.
- **Mechanism (PO):** Headgroup must own a GPU tile collection and run sampler → escape → edge → shade shaders. Tiles are independent, share a homothety; CPU only computes seat deltas (IntExp O(1)). Live path CPU-shades every frame and cleared the tile hoard on location change.
- **Status:** landing — shared-device grey fixed; shade/oracle parity tests cover default ops including out-filament + STE. Vsync Fifo forced (`HEADGROUP_PRESENT_MODE`). **Still open:** headed fps re-measure evidence; full coloring_script edge cases beyond default.
- **Locus:** `headgroup/window/{mod,sampling,shade,gpu_display}.rs`; quotes in `he-said/phase-2-display-regressions.md`.

### B-TEN-1 — Unknown painted as set-black (tenacity)
- **Symptom:** Unfinished / unknown seats render as black (looks like Inside). Made the home antenna look gapped / off the real axis when seats were merely unknown.
- **Mechanism (PO):** Marking unknown as black breaches tenacity. Unknown must use `NORES_ANSWER` (Outside, escape after 1 iteration, `escape_z` at infinity) — not Dummy/black-as-set.
- **Status:** fixed — `pack_tile_upload` no longer drops infinite `min_magnitude` (NORES); shade treats missing as Outside escape-1. Test: `b_ten_1_tests::nores_answer_packs_as_outside_not_missing`.
- **Locus:** `gpu_display/{mod.rs,shade.wgsl}`; `constants::NORES_ANSWER`.
- **Quotes:** `he-said/unknown-nores.md`

### B-PER-2 — Period bands in deeper minibrots (regression / thought-fixed)
- **Symptom:** Concentric gray bands inside deeper minibrots; ugly seam between solid black (period known) and banded (period wrong/unknown) regions. Reappearance of period-banding class (was B-PER-1).
- **Mechanism (PO):** Correct period detection is demanding. Emitting **false periodicity** violates tenacity. Regular iterate must use the simplest **certain** check only; full period resolve belongs in a later phase (D-PER-1). `period == 0` means unknown (never a real period).
- **Status:** landing — regular Inside answers emit `period == 0`; shade `opt_period` ignores 0; D-PER-1 resolve after tile edge complete. Test: `b_per_2_tests::regular_inside_answer_emits_unknown_period_zero`. Verify deeper minibrot visually.
- **Locus:** `naive_{cpu,gpu}_worker` answers; `tile_session::try_resolve_periods`; `shade.wgsl`.
- **Quotes:** `he-said/period-determination-phase.md`
- **Evidence:** PO screenshot (deeper minibrot right half banded vs left solid).

### B-STE-1 — Small-time edges (interior tree missing / stray lines)
- **Symptom:** Interior small-time tree missing until first zoom-in; also stray lines from exterior `small_time == 0` vs nonzero neighbors.
- **Mechanism (PO):** Exterior `small_time == 0` is valid — do **not** filter all zeros for **paint**. Small_time updates matter of course each iterate; period resolve is a separate phase (see D-PER-1), not a bolted-on mid-loop `determine_period`.
- **Status:** landing — paint/stray-line fixes remain; D-SCH-3 now schedules `small_time_edge` after out-fill so STE work is not zoom-gated by scheduler starvation. Visual confirm interior tree without zoom still needed.
- **Locus:** `tile_session.rs`; `color.rs` `is_node_tree`; `naive_cpu_worker.rs` `iterate_point_bout`; `shade.wgsl`.
- **Quotes:** `he-said/scheduler-and-edges.md`, `he-said/period-small-time.md`, `he-said/period-determination-phase.md`, `he-said/scheduler-boundary-trace.md`

### B-SCH-3 — Regularly shaped incomplete (black) bands
- **Symptom:** Rectangular / striped unfinished regions.
- **Mechanism (PO):** Structural — worker→collector pipes backing up is not acceptable as a product state. Requeue-on-blocked is a visibility/mitigation only, not the fix. Proper redress when Phase 2 tile path lands (tiles should shrink batch pressure / clarify ownership).
- **Status:** **parked** — readdress at Phase 2 execution. Do not add further send-path tooling/tests for this.
- **Locus:** `screen_worker/mod.rs`; `main.rs` capacities; `tile_session.rs`.
- **Quotes:** `he-said/scheduler-and-edges.md`

### B-PER-1 — Period banding (residual visual)
- **Done so far:** See `phase-1.5-notes.md`.
- **Status:** **reopened as B-PER-2** — deeper minibrot bands returned.

### B-ZOOM-1 — Apparent stall zooming into bulbs left of cardioid
- **Status:** **parked** — PO could not reproduce.

## Design gaps (open)

### D-GEAR-TYPED — Host batch still f64; stacked/FloatExp bout not fully monomorphized
- **Need:** Typed ActivePoint enum / Mandelbrotable bout for StackedI32 + AdaptiveRug (FloatExp). Today gear is read: F32 has f32 bout; GPU gated on `runs_on_gpu()` (no silent F64→f32). Stacked GPU bout still deferred to CPU.
- **Status:** open (partial P3)
- **Locus:** `perturbation_{cpu,gpu}_worker.rs`; `gears.wgsl`


### D-PUB-GPU — GPU publisher deleted; CPU publish_seat is live path
- **Status:** closed (purge) — `publisher_shader.rs` / `publisher.wgsl` removed; tile publish uses CPU `publish_seat` in `tile_publisher.rs`.
- **Locus:** `workgroup_new/tile_publisher.rs`.

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
