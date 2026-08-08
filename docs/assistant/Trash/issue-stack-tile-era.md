# Bug / todo stack (live)

Two lists: **true bugs** (incorrect behavior) vs **design gaps** (intended design not yet on the live path). Newest notes at the top of each open section. Do not clear an item until verified fixed (tests and/or visual).

PO quotes: `he-said/`. This file tracks status, loci, and follow-through.

## DAT failures (2026-08-01)

Developer acceptance test failed. Items below are verbatim from DAT.
Landing notes (assistant): NORES fallthrough + WIP proximate gate + location mag readout + publish cadence [20,100k]; lookahead progressive WIP + mag-retarget flush (unit tests green; headed confirm still needed).

- worker panics and never recovers — *unit closed: SessionRestore puts session back (3 tests); SteadyState restarts; headed recheck optional*
- sampling bug (?), some tiles are the NORES values for no reason — *see B-HOARD-NORES; unit S1–S3 + CPU lookup skip*
- thin towers of one tile going up / down (lookahead / hoard) are missing, resulting in extreme overuse of the nores value — *see B-LOOK-1; unit lookback export+carry+sampling ingest*
- when zooming, new work takes a good 1-2s to start (play, regression) — *see P-PERF-BALANCE / D-PLAY-TICK (sync ~50ms GPU-scaled iteration ticks)*
- even though TPS is better than before, it is not near the target. — *design closed (D-GPU-1…8, D-PUB-3/4): GPU-resident calibrated after every bout, publisher binds directly (uploader bypass), dense WIP refill, on-device completion counter, no payload readback. Impl still short of ≥3000; remaining wall is host sync / readback drift vs that design. Headed NORES-grey (B-DISP) note-only. No homescreen cheat.*
- intexp values are not displayed properly — *unit closed: `format_intexp_readout` avoids ellipsis; 3+ location readout verifies*
- animated bailout is not running & configuable bailout not working — *unit landing: max-extra slider restored; anim period uses time range; shade uniforms from determine; 3+ bailout verifies*
- regression: coloring options from v0.0.9 are almost all removed — *closed for addability: default stays D-COLOR-1 (3 layers); Add layer menu restores all v0.0.9 kinds; Remove selected keeps ≥1; 3+ unit tests green. Headed smoke still welcome.*
- regression: normalization breaks period animation with NAN (see v0.0.9 for golden. Use it as an oracle, but shader must be on GPU.) — *fixed: RecipLn = ln(1/x); GPU shade tests green*
- previous work is not re-emitted in WIP tiles; incomplete parts of new tiles are the NORES value. — *see B-HOARD-NORES*
- precision wall at mag 20 (depth requirement fail) — *unit landing: pot20 complete + mag26 progress/gear-beyond-f32; verify tags ≥3; rug/headed recheck still open*
- magnificiation missing from current location display — *unit closed: HUD `mag 2^{pot}` via `format_location_readout`; wired in window*
- goto Apply greyed out on pasted/readout location strings — *see B-GOTO-1; unit closed*
- goto Apply lands differently depending on current view — *see B-GOTO-2; unit closed*
- tile worker constipated (99% load, 0 publish) zooming into set neck/butt — *see B-PIVOT-1*

## True bugs (open)

### B-PIVOT-1 — Tile worker constipates on neck/butt zooms (never recovers)
- **Symptom:** Zoom/goto into set neck or butt: tile worker ~99% load, publisher/uploader avg rate 0, intratile ~100%; never recovers. Main pivotability fail.
- **Repro loci (PO 2026-08-06):**
  - `0.313232421875 + 0.015380859375i mag 2^2` (butt/neck)
  - `-0.742538452148 -0.005569458008i mag 2^6` (seahorse neck-ish)
- **Telemetry:** window still sends ~30/s retargets; worker←scheduler/reference rates drop to 0; worker→publisher/uploader 0.
- **Mechanism:** sticky open-batch monopoly while `seats_done==0` + GPU `await_map_async` busy-spin; Agnostic WIP never entered `unsent_origins`.
- **Status:** unit landing — play tick rotates after WIP-only steps; map await yields after soft deadline; screen WIP proximate-publish; 3 pivot verifies green. Headed recheck at PO loci.
- **Locus:** `tile_session` `workshift_play_tick` / WIP publish; `perturbation_gpu_worker` `await_map_async`.
- **Related:** P-PERF-BALANCE play; B-SCH-3 bands.

### B-GOTO-2 — Apply lands differently depending on starting view
- **Symptom:** Pasting/applying the same location string from different magnifications or pans reaches different centers.
- **Mechanism:** `SetPos` converts center→UL using the *current* zoom; goto emitted `SetPos` then `SetZoom`, so target mag was applied after the wrong half-extent.
- **Status:** unit closed — goto emits `SetZoom` before `SetPos`; 3 absolute-center verifies green.
- **Locus:** `coords.rs` (`commands_from_goto_line`); `transforms.rs` (`SetPos`).

### B-GOTO-1 — Apply rejects location readout / copy-paste format
- **Symptom:** Goto field showing values the HUD itself produced (e.g. `0.30… -0.01…i mag 2^2`) leaves Apply greyed out.
- **Mechanism:** `commands_from_goto_line` only accepted `re im [pot]` decimals; readout uses `a±bi mag 2^N`.
- **Status:** unit closed — goto accepts HUD readout round-trip (3 verifies); Apply enables on paste of produced strings.
- **Locus:** `headgroup/window/coords.rs` (`parse_location_or_pair`, `commands_from_goto_line`).

### B-LOOK-1 — Lookahead and lookback columns missing
- **Symptom:** Thin towers of one tile going up (lookahead / deeper mag) and down (lookback / lesser mag) are absent; viewport overuses NORES instead of column remaps.
- **Mechanism (PO):** Lookahead and lookback are both mag columns under attention, not only flat lesser fallthrough.
- **Root cause (2026-08-06):** mag retarget cleared `unsent_tiles` and wiped `answer_tiles` without exporting the prior screen column, so lookback never reached the headgroup.
- **Status:** unit landing — `take_screen_column_for_lookback` + worker carry merge; tests H2/H2b/H2c/H3/H3b. Headed tower confirm still needed.
- **Locus:** `tile_session` lookahead claim/WIP; `tile_worker` retarget carry; shade tile-entry ranks.
- **DAT:** thin towers / extreme NORES overuse.

### B-HOARD-NORES — NORES instead of remapped / column prior work
- **Symptom:** Parts of the viewport stay NORES when lesser/same-mag Answers exist in the hoard, and WIP unfinished seats publish NORES instead of proximate prior work.
- **Mechanism:** (1) Live GPU `load_raw` accepts packed NORES (Outside) and blocks lesser/finer; CPU `lookup_*` already skips `answer_is_nores`. (2) WIP seats without proximate → `publish_seat` → NORES.
- **Status:** unit landing — sampling.wgsl full-sentinel skip; proximate re-emit; Agnostic-without-prior leaves hole; S1–S3 + H1/H1b. Headed recheck still open.
- **Locus:** `gpu_display/sampling.wgsl`; `sampling.rs`; `tile_session` WIP publish; `tile_publisher::publish_seat`.
- **DAT:** sampling NORES-for-no-reason; previous work not re-emitted in WIP.

### B-DISP-1 — Phase 2 cutover display regressions (grey / ~15fps / no GPU escape)
- **Symptom:** Immediate display grey; ~15fps; **tps:0** at home; no escaper on GPU; settings open greys window; location UI misplaced / no input box.
- **Mechanism (PO):** Headgroup must own a GPU tile collection and run sampler → escape → edge → shade shaders.
- **Status:** **fix landing** — GPU-resident whole-tile completions bridged to publisher bypass → `pixels_in` → `ingest_gpu_handle` (D-PUB-4). Shared-device grey / shade/oracle / vsync Fifo landed. **2026-08-04:** headed NORES-grey / blocky-nav closed for default path — GPU-resident atlas handoff is **opt-in** (`CZ_GPU_RESIDENT=1`); default uses Answer/`write_slot` publish. Synthetic Outside-escape-4 fallback removed from GPU publish. Opt-in path materializes via point harvest → `cpu_fallback` pack. **Still open:** headed fps re-measure; true textureStore→copy handoff; full coloring_script edge cases.
- **Locus:** `tile_session.rs` (`emit_gpu_whole_tile`, `pending_gpu_bypass`); `tile_worker/mod.rs` (`flush_unsent_tiles`); `headgroup/window/{mod,sampling,gpu_display}.rs`.
- **Guard:** `scripts/e2e_visual.sh` (gray-hole + structure oracles); unit `whole_tile_completion_queues_bypass_once`.

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

### P-PERF-BALANCE — 1×-GPU continuous play (sync tick)
- **Need:** App must work as well with ~1× CPU-FLOPS GPU as with ~200×: low play, continuously visible outputs. Same tick period; only iteration budget scales with GPU speed.
- **Machine note (2026-08-06):** this box is a **GTX 1080 Ti** — green here is not acceptance. T550-class currently produces nothing.
- **Status:** landing — worker live path uses `workshift_play_tick` (50 ms + GPU-scaled N). Legacy `workshift_budget_ms_for_session` now returns the same 50 ms period (no dual 1/8/16 pacing). `await_map_async` increments `payload_map_harvests` (debt); GPU-resident scatter remains `CZ_GPU_RESIDENT=1` opt-in. Fitness P1–P3 + P3b (force_cpu clean).
- **Fitness:** P1 ingest within 2 ticks; P2 exact N; P3 zero payload harvest on bypass/CPU path + unique ingest.
- **Locus:** `tile_worker/mod.rs`; `play_tick.rs`; session workshift; GPU completion counter (D-GPU-3).

### D-GEAR-TYPED — Host batch still f64; stacked/FloatExp bout not fully monomorphized
- **Need:** Typed ActivePoint enum / Mandelbrotable bout for StackedI32 + AdaptiveRug (FloatExp). StackedI32 now dispatches GPU stacked bout pipelines; AdaptiveRug stays CPU (auth).
- **Status:** closed — `ActiveGearWork` holds typed arms (F32/F64/Stacked1..=8/FloatExp); CPU bouts are `iterate_perturbation_bout_typed<T>`; gear refresh drops batches on identity change. Residual: GPU still bridges through host f64 point buffers for upload.
- **Locus:** `active_gear_work.rs`; `perturbation_{cpu,gpu}_worker.rs`; `gears.wgsl` / `stacked_bout_tail.wgsl`

### D-GEAR-ARRAY — `[i32;N]+exp` CPU-only array gear
- **Need:** Auth lists array gear; N unspecified; auth “12 stack” vs enumerated 11.
- **Status:** blocked on developer specifying N / reconciling count. Explicit design hole in `gear.rs`.
- **Locus:** `docs/design/tile_worker.md`; `src/gear.rs`

### D-PUB-GPU — GPU publisher; uploader only for CPU work
- **Need:** GPU shader combines hoarded tiles with new calibrated work (NORES / proximate bias). Cadence **[20, 100000] Hz** while incomplete; idle when complete (D-PUB-1). GPU-native worker path binds calibrated in VRAM directly (D-PUB-4 / D-GPU-7); uploader bypassed.
- **Status:** design closed. Impl landing — LivePublisher owned by tile publisher; GPU publisher shader preferred; host bridge until calibrated-buffer bind is complete. Cadence constants aligned to auth [20, 100000]; headed GPU home TPS ≥3000 still open.
- **Locus:** `workgroup/tile_publisher.rs`, `workgroup/publisher_shader.rs`, `publisher.wgsl`.

### D-ACT-1 — Workgroup SteadyState actor layout vs auth
- **Need:** Live telemetry graph matches auth workgroup sub-actors (not work controller / screen worker).
- **Status:** closed — registered actors: tile scheduler, tile worker, intratile scheduler, reference worker, gpu uploader, tile publisher. Reference receives whole-screen stencils from the headgroup (not via scheduler). Glitch = zero-orbit fallback only (no rebase).
- **Locus:** `main.rs`; `tile_scheduler_actor.rs`; `tile_worker/`; `intratile_actor.rs`; `reference_actor.rs`; `headgroup/window/mod.rs`.

### D-PER-1 — Period certainty vs scheduling (tension with auth period_detector)
- **Need (historical):** After boundary/out-fill, resolve periods on the in-edge; unknown period allowed until sure; avoid ugly out-filament seams between period-unknown and period-known Inside.
- **Auth tension:** `docs/design/period_detector.md` forbids a separate stalling period-detection phase and requires the detector on iterated points. Locked non-auth default is **integrated** period on the main path (D-PER-4); two-pass / deferred resolve is a **design fallback only** (D-PER-5, approval required).
- **Status:** impl still has `period_resolve*` gating in places — treat as debt toward D-PER-4 / auth, not as license for a separate phase. Flood-in may still use auth’s “same placeholder period” under unknown (intratile). Visual out-filament seam remains a verify item.
- **Locus:** `tile_session.rs`; `outfill_infill_scheduler.rs`; shade out-filament; `docs/design/period_detector.md`.
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
