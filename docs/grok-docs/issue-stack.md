# Bug / todo stack (live)

Two lists: **true bugs** (incorrect behavior) vs **design gaps** (intended design not yet on the live path). Newest notes at the top of each open section. Do not clear an item until verified fixed (tests and/or visual).

PO quotes: `he-said/`. This file tracks status, loci, and follow-through.

## True bugs (open)

### B-DISP-1 — Phase 2 cutover display regressions (grey / ~15fps / no GPU escape)
- **Symptom:** Immediate display grey; ~15fps; no escaper on GPU; settings open greys window; location UI misplaced / no input box.
- **Mechanism (PO):** Headgroup must own a GPU tile collection and run sampler → escape → edge → shade shaders. Tiles are independent, share a homothety; CPU only computes seat deltas (IntExp O(1)). Live path CPU-shades every frame and cleared the tile hoard on location change.
- **Status:** in progress — wgpu PaintCallback sampler+neighbor-escape+shade landed; stop clearing tile hoard on pan; coord bar+goto box; settings try_lock no longer greys. Still thin vs full coloring_script / full filament escape on GPU.
- **Locus:** `headgroup/window/{mod,sampling,shade,gpu_display}.rs`; quotes in `he-said/phase-2-display-regressions.md`.

### B-TEN-1 — Unknown painted as set-black (tenacity)
- **Symptom:** Unfinished / unknown seats render as black (looks like Inside). Made the home antenna look gapped / off the real axis when seats were merely unknown.
- **Mechanism (PO):** Marking unknown as black breaches tenacity. Unknown must use `NORES_ANSWER` (Outside, escape after 1 iteration, `escape_z` at infinity) — not Dummy/black-as-set.
- **Status:** open.
- **Locus:** collector / escaper / `CompletedPoint::Dummy` vs `constants::NORES_ANSWER`; color path for unfinished.
- **Quotes:** `he-said/unknown-nores.md`

### B-PER-2 — Period bands in deeper minibrots (regression / thought-fixed)
- **Symptom:** Concentric gray bands inside deeper minibrots; ugly seam between solid black (period known) and banded (period wrong/unknown) regions. Reappearance of period-banding class (was B-PER-1).
- **Mechanism (PO):** Correct period detection is demanding. Emitting **false periodicity** violates tenacity. Regular iterate must use the simplest **certain** check only; full period resolve belongs in a later phase (D-PER-1). `period == 0` means unknown (never a real period).
- **Status:** open — `get_loop_period` already ignores 0; fix `point_to_answer` `unwrap_or(1).max(1)` false period-1; then certain-iterate; then D-PER-1 phase.
- **Locus:** `periodicity_detector.rs`; `naive_cpu_worker.rs` `point_to_answer`; out-filament / `get_loop_period`; `tile_session` phases.
- **Quotes:** `he-said/period-determination-phase.md`
- **Evidence:** PO screenshot (deeper minibrot right half banded vs left solid).

### B-STE-1 — Small-time edges (interior tree missing / stray lines)
- **Symptom:** Interior small-time tree missing until first zoom-in; also stray lines from exterior `small_time == 0` vs nonzero neighbors.
- **Mechanism (PO):** Exterior `small_time == 0` is valid — do **not** filter all zeros. Small_time updates matter of course each iterate; period resolve is a separate phase (see D-PER-1), not a bolted-on mid-loop `determine_period`.
- **Status:** open — scheduler/edge-trace work ongoing; STE-until-zoom still unverified.
- **Locus:** `tile_session.rs`; `color.rs` `is_node_tree`; `naive_cpu_worker.rs` `iterate_point_bout`.
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

### D-PER-1 — Period-determination phase after boundary + out-fill
- **Need:** After boundary tracing and out-fill complete, run a phase that determines periods of the **in-edge**. Regular iterate must stay certain (no false periods); unknown period is allowed until this phase. Out-filament rendering must not show an ugly boundary between period-unknown Inside and period-known Inside.
- **Status:** open — necessary division; not optional polish.
- **Locus:** `tile_session.rs` (new phase); period resolve worker path; shadergroup out-filament / period consumers.
- **Quotes:** `he-said/period-determination-phase.md`

### D-SCH-3 — Boundary-trace all renderable edges (not only in/out)
- **Need:** Scheduler is out-fill + boundary trace. Order: out-fill until a real boundary → trace **in/out**, **in-filament**, **out-filament** edges → finish out-fill → trace **small-time** edges. Also period-change edges; future **derivative magnitude angle** on Answers (in-filament detection outside workgroup). Do not time-slice inefficiently across phases.
- **Status:** open — live `TileSession` only has scredge / in/out / edge queues (in/out style).
- **Locus:** `tile_session.rs` (and later Answer field for derivative magnitude angle).
- **Quotes:** `he-said/scheduler-boundary-trace.md`

### D-SCH-1 — Full screen-edge seed missing on live tile path
- **Need:** Out-bucket-fill requires walking the **entire screen edge** first (prototype had it; live `TileSession` only seeds tile rims).
- **Status:** landing — `TileSession` seeds full screen edge into scredge; `FloodIn`/`PeriodEdge`/`In` gated until `screen_edge_complete()`. Verify visually (no whole-screen black flood before edge done).
- **Locus:** `tile_session.rs`; `work_controller.rs` edge walk; `screen_worker/mod.rs` Replace.
- **Quotes:** `he-said/scheduler-and-edges.md`

### D-SCH-2 — After screen edge walked: in-fill hints with propagated period
- **Need:** Hint-fill **in** areas as Inside with period from the edge of that area; still work points for min-magnitudes.
- **Status:** open — blocked on D-SCH-1; `in_queue` `u32` slot available for period hint.
- **Quotes:** `he-said/scheduler-and-edges.md`

## Done (recent)

- Phase 1.5 period detector + shape tests (see `phase-1.5-notes.md`).
- Out-filament: `get_loop_period` skips `loop_period == 0` (Dummy).
