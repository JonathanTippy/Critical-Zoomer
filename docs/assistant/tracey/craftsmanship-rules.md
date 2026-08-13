# Craftsmanship Tracey rules (assistant-owned; v0.0.9 workgroup inventory)

One rule per item in the craftsmanship inventory (`docs/assistant/design/workgroup-virtues.md`
§10), plus two protocol-level rules. These bind the *golden mechanisms* to their code sites so
that future edits (GPU port, cleanup of the §12 experiments) cannot silently lose them.
Code sites carry `r[impl ...]` markers. Normative prose: the virtues doc; these tags exist for
Tracey linkage.

> These are invariants of the *restored v0.0.9 code*, verified against the live tree on
> 2026-08-06. They are not aspirations. If a rule's verify fails, either the code regressed or
> a deliberate, developer-approved redesign happened — update the rule and the virtues doc
> together in that case.

## Protecting the lineage

Prevention beats detection. Two disciplines keep these rules from being undone by
accident:

- **Prefer a type over a comment.** Where an invariant can be made unrepresentable
  in the type system, make it unrepresentable. `r[cz.craft.bout-cap+1]` is the
  pattern: the unbounded-call rule held only as prose until `BoutCap` made an
  over-limit cap impossible to express. When touching any rule below, ask first
  whether it is still prose-only and could be a type instead.
- **The change protocol.** A deliberate redesign updates the rule here, the prose
  in `workgroup-virtues.md`, and the pinned test *together*, in one change.
- **The pre-edit hook.** `.cursor/hooks.json` runs `.cursor/hooks/workgroup-rules.sh` on
  every `Write`/`StrReplace`/`EditNotebook`. When the target file is under
  `src/assemblies/workgroup/screen_worker/` or `src/assemblies/shadergroup/colorer/`, it
  injects that file's rule summaries (below) as agent context at the moment of the edit —
  the forcing function the docs alone cannot provide. It fails open (never blocks an edit).
- **Test leftover reaper.** `.cursor/hooks/kill-test-zombies.sh` (before/after
  `cargo test|cargo bench|screenshot_check`, and on agent `stop`) reaps repo-scoped
  app/bench orphans and `/tmp/cz_*` Xvfb sessions so cleanup does not depend on ad-hoc
  `pkill` approvals. Fails open; log `/tmp/cz_zombie_kill.log`.
  `.cursor/hooks/guard-raw-kill.sh` **denies** raw `kill`/`pkill` aimed at those
  leftovers (after running the reaper). Always-on:
  `.cursor/rules/test-zombie-reaper.mdc`.

The always-on summary for editing sessions is `.cursor/rules/critical-zoomer-invariants.mdc`;
the agent-facing entry point is `AGENTS.md`. Detection (the periodic tracey-link audit and
the full test suite) catches what prevention misses.

r[cz.craft.epsilon-pixel-pitch+1]

**Normative summary.** The provisional loop-detection trigger derives its epsilon from the
actual pixel pitch in complex space (neighbor distance / 256). It only decides when to attempt
completion; final period/interiority truth comes from `cz.craft.period-derivative-test+1`.

**Caveat.** Better than a constant, but the `/256` factor is still arbitrary — a tolerance
that falls out of the correct numbers with no free parameter is a design ideal
(`docs/assistant/design/workgroup-virtues.md` §13). Acceptance criteria below describe the
current form and stand until such a form exists.

**Code site.** `src/assemblies/workgroup/screen_worker/workshift.rs` — epsilon computed from
`points[0].c − points[1].c` (search `episilon`).

**Acceptance criteria.**
- [ ] Epsilon scales with the frame's pixel pitch: two frames at different zoom produce
  proportionally different epsilons; no absolute constant is used.

**Test.** `epsilon_scales_with_pixel_pitch` (`src/assemblies/workgroup/screen_worker/craftsmanship_tests.rs`) — doubling the pitch doubles epsilon.

r[cz.craft.period-derivative-test+1]

**Normative summary.** A repeating point's period candidates are the atom-domain partials
(record-minimum steps of the critical orbit) tried in ascending order — the FIRST that verifies
is the true period. Verification is Newton on `F^p(w,c)=w`, reduced to the converged root's
minimal period, accepted exactly when the cycle multiplier satisfies `|∂F^p/∂z| <= 1`. Failure
on every candidate yields period 0 ("repeats, period unknown"), and unknown periods must not
light period edges. The pixel-pitch epsilon is not the final oracle.

Two corrections to the naive pipeline, both caught by the shape oracles below:
- Trying only the last record minimum (`small_time`) verifies a multiple of the true period,
  because interior orbits set records until convergence.
- Newton started from `F^p(0,c)` is unreliable near necks (parabolic = linear convergence) and
  can land on a divisor root; starting from the orbit's tail iterate and reducing to the
  minimal period fixes both.

**Code site.** `workshift.rs` — `period_partials`, `verified_period` / `verified_period_from`;
completion in `workshift` tries partials ascending with the tail iterate as Newton start, else 0;
`point_is_edge` refuses to match period 0.

**Acceptance criteria.**
- [ ] Published period-1, period-2, period-3 and period-4 attractors verify with their periods.
- [ ] Exterior points and incorrect candidate periods are rejected.
- [ ] Generated points strictly inside the main cardioid detect as period 1; generated points
  strictly inside the disk |c+1| < 1/4 detect as period 2.
- [ ] At the cardioid/bulb neck c = −0.75, points at −0.75 ± 2^-k (k = 2..40) detect as
  period 1 (right) and period 2 (left).
- [ ] Small neighborhoods inside known period-3 and conjugate period-4 child components remain
  period-constant; no cloudy or speckled period noise appears in component interiors.
- [ ] Newton that lands on a divisor of the candidate period is reduced to the minimal period
  before the multiplier test (a period-2 bulb point must never report 4/8).
- [ ] An unfinished scheduling-queue publication uses period 0 (unknown), never its loop-checkpoint
  gap; only the completed verification path may publish a nonzero period.
- [ ] The renderer treats period 0 as missing data, not a numeric period: unknown periods create no
  out-filaments, while differing verified periods still do.
- [ ] No timewarp or tighter-epsilon period-refinement pass remains.

**Test.** `known_attractors_have_their_published_periods`,
`exterior_or_wrong_period_is_not_accepted`, and the proptests
`main_cardioid_points_detect_as_period_one`, `period_two_bulb_detects_as_period_two`, and
`child_bulb_interiors_are_period_constant`,
`neck_zoom_classifies_correctly_at_arbitrary_depth`, plus the real scheduling-path test
`provisional_answer_never_marks_delivered` (`craftsmanship_tests.rs`) and renderer tests
`unknown_period_never_creates_out_filament` /
`differing_verified_periods_still_create_out_filament` (`shadergroup/colorer/color.rs`).

r[cz.craft.cached-products+1]

**Normative summary.** `Point` caches squared/cross products so each iteration is three
multiplies plus checks; smallness and small-time are collected as free side effects.

**Code site.** `workshift.rs` — `update_point_results` (running min of |z|² + small_time).

**Acceptance criteria.**
- [ ] The iterate path performs no redundant squaring; smallness/small_time update inside the
  same per-iteration function, not a separate pass.

**Test.** `cached_products_match_z` (proptest, craftsmanship_tests.rs) — cached products equal
z-products for arbitrary z; smallness/small_time collected in the same call.

r[cz.craft.lifo-drain+1]

**Normative summary.** The per-shift completion Vec drains newest-first (pop from end), so during a
pivot the freshest work publishes first.

**Code site.** `src/assemblies/workgroup/screen_worker/mod.rs` — `work_update` pops the
growable `completed_points` Vec.

**Acceptance criteria.**
- [ ] Property: drain order is exactly reverse push order.

**Test.** `completion_drain_is_lifo` / `mutant_kill_completion_lifo_and_struggling_to_clear`
(craftsmanship_tests.rs).

r[cz.craft.edge-push-front+1]

**Normative summary.** Neighbors of a detected boundary pair are pushed to the *front* of the
edge queue — edges jump their own line.

**Code site.** `workshift.rs` — `queue_incomplete_neighbors_of_edge` uses `push_front`.

**Acceptance criteria.**
- [ ] Newly discovered edge work is scheduled before older queued edge work (queue head
  observation or equivalent unit).

**Test.** `edge_neighbors_jump_queue_front` (craftsmanship_tests.rs) — new entries precede a
pre-existing entry; delivered neighbors excluded.

r[cz.craft.cost-metadata+1]

**Normative summary.** Queue entries carry a cost estimate (spawning neighbor's iteration
count, or period for interior floods), captured free at the source.

**Code site.** `workshift.rs` — `queue_incomplete_neighbors` / `queue_incomplete_neighbors_in`
tag entries with the completer's iterations/period.

**Acceptance criteria.**
- [ ] Every queue entry's metadata equals the spawning point's measured cost (no recompute, no
  placeholder constants).

**Test.** `queue_entries_carry_source_cost` (craftsmanship_tests.rs) — out entries carry the
source's iterations, in entries its period.

r[cz.craft.mixmap-shuffle+1]

**Normative summary.** Seat traversal order is a shuffled permutation, regenerated exactly when
resolution changes — anti-banding by construction.

**Code site.** `src/assemblies/workgroup/screen_worker/workshift.rs` — `get_random_mixmap`,
called from `from_stencil` on resolution change; `random_map` field of the context.

**Acceptance criteria.**
- [ ] The mixmap is a true permutation (no duplicates, full coverage); identical res keeps the
  same map; changed res rebuilds.

**Test.** `mixmap_is_permutation` (proptest, craftsmanship_tests.rs) covers the permutation
half; the rebuild-on-resolution-change half is acceptance by code review (`from_stencil`) plus
`replace_reuses_points_capacity_and_resets_initialized`.

r[cz.craft.scredge-first-shift0+1]

**Normative summary.** When the attention spiral yields nothing on the first shift of a new
context, the screen-perimeter queue leads the fallthrough; after that first shift, scredge
lives in slot 4 of the rotation.

**Code site.** `workshift.rs` — slot 0 fallthrough with `prefer_scredge: workshifts == 0`.

**Acceptance criteria.**
- [ ] First-shift fallthrough draws scredge seats before edge; later fallthrough / slot 1 prefer
  edge over scredge.

**Test.** `scredge_first_only_on_shift_zero` (craftsmanship_tests.rs) — first buffered
completion belongs to the scredge seat on shift 0 (spiral exhausted), to the edge seat on
shift 1.

r[cz.craft.attention-spiral+1]

**Normative summary.** Slot 0 of the five-slot rotation is the attention phase when motion is
`Zoomed` or `Neither` (see `pan-zoom-slot0`). It walks a square-ring spiral from the live
attention seat (`Some`) or screen center (`None`, pointer off-screen), skipping delivered /
off-screen seats. Tenacity is *state*, not call depth: the seat under work is held in
`attention_current`, and every bout is bounded by `BoutCap` (see `bout-cap`). When the held
seat completes (or is found delivered), the hold is released and the next bout advances the
spiral. Exhaustion falls through to the queue priorities.

**Code site.** `workshift.rs` — `next_attention_spiral_pos`, `set_attention`,
`attention_current` hold/release, slot 0 of `workshifts % 5`; `screen_worker/mod.rs`
attention drain; `inputs.rs` sends `Option`.

**Acceptance criteria.**
- [ ] Fresh shells default the spiral anchor to screen center with `attention: None`.
- [ ] Spiral offsets are non-decreasing in Chebyshev distance from the origin.
- [ ] On Zoomed / Neither, slot 0 selects the spiral seat before queued edge work.
- [ ] A held seat is reworked on the next attention bout until it completes, then released.
- [ ] A held seat found delivered is released so the bout cannot spin on it.
- [ ] `set_attention(None)` restores the center anchor, restarts the index, drops the hold.

**Test.** `from_stencil_defaults_attention_anchor_to_center`,
`square_ring_spiral_is_nondecreasing_chebyshev`,
`attention_slot_picks_spiral_before_queues`,
`attention_bout_works_seat_to_completion`,
`attention_holds_seat_across_bouts_until_complete`,
`attention_releases_held_seat_delivered_elsewhere`,
`spiral_skips_delivered_and_falls_through_when_exhausted`,
`spiral_skips_offscreen_seats`,
`set_attention_none_restores_center_anchor`.

r[cz.craft.bout-cap+1]

**Normative summary.** The worker may never make an unbounded call. The 10 ms wall-clock check
at the top of the bout loop is only valid if no call inside the loop can run away. Every
iteration bout therefore takes a `BoutCap`, whose only constructor clamps to `MAX_BOUT`
(1000). Passing a raw `u32` (including `u32::MAX`) is a type error.

**Code site.** `workshift.rs` — `BoutCap`, `MAX_BOUT`, `iterate_max_n_times`.

**Acceptance criteria.**
- [ ] `BoutCap::new(n)` never returns a value greater than `MAX_BOUT`.
- [ ] `BoutCap::new(u32::MAX)` equals `BoutCap::STANDARD`.
- [ ] The sole production caller of `iterate_max_n_times` passes `BoutCap::STANDARD`.

**Test.** `bout_cap_clamps_above_max`, `attention_bout_on_hard_seat_never_exceeds_max_bout`.

r[cz.craft.pan-zoom-slot0+1]

**Normative summary.** Slot 0's leading phase is the only motion-dependent choice.
`Zoomed` or `Neither` → attention first. `Panned` → scredge first, but only on the
*first* shift of that shell (`workshifts == 0`). Once the pan frame has had its first
shift — including when the user stops panning and no new Replace arrives — slot 0
returns to attention. No other slot changes. Fresh shells are `Neither`. Zoom takes
precedence when both change.

**Code site.** `workshift.rs` — `Motion`, `from_stencil` classification, slot 0 of
`workshifts % 5`; `screen_worker/mod.rs` passes the previous objective into `from_stencil`.

**Acceptance criteria.**
- [ ] No previous → `Motion::Neither`.
- [ ] Zoom pot changed → `Motion::Zoomed`; same zoom, position changed → `Motion::Panned`.
- [ ] On the first shift of a pan shell, slot 0 starts a scredge seat before attention.
- [ ] On later shifts of a pan shell (user stopped), slot 0 leads with attention.
- [ ] On zoom, slot 0 starts the attention spiral before a scredge seat.
- [ ] Slots 1–4 are identical regardless of motion.

**Test.** `from_stencil_classifies_zoom_pan_neither`,
`pan_slot0_prefers_scredge_over_attention`,
`pan_scredge_lead_only_on_first_shift`,
`zoom_slot0_prefers_attention_over_scredge`,
`slots_one_to_four_ignore_motion`.

r[cz.craft.out-rotates-in-stays+1]

**Normative summary.** An unfinished escape seat rotates to the back of its queue (hard pixels
yield the floor); an unfinished interior seat is deliberately *not* rotated (period detection
wants depth). The asymmetry is intentional.

**Code site.** `workshift.rs` — the `Step::Out` rotate-to-back branch and the commented-out
`Step::In` rotation beside it.

**Acceptance criteria.**
- [ ] An out seat that fails to finish within its bout is re-queued behind all pending out
  seats; an in seat is retried without rotation.

**Test.** `out_rotates_without_loss` (craftsmanship_tests.rs) — Out seats survive a shift
undropped and undelivered; In likewise. Note: the In rotation is currently commented out, so
the asymmetry is latent; the test pins "no loss" for both and the rotate branch for Out.

r[cz.craft.provisional-not-delivered+1]

**Normative summary.** An unfinished screen-edge seat publishes a *provisional* repeat answer
(checkpoint delta as period, real smallness data) and remains undelivered so later shifts still
finish it. Guesses never block truth.

**Code site.** `workshift.rs` — `Delivery::Provisional` (cannot set `delivered`) pushed via
`WorkContext::push_delivery`; the `Step::Scredge` unfinished branch stages a provisional
`CompletedPoint::Repeats`.

**Acceptance criteria.**
- [ ] Provisional pushes never set `delivered` — now type-enforced: only `Delivery::Final`
  may flip the flag, and only inside `push_delivery`.

**Test.** `provisional_answer_never_marks_delivered` (craftsmanship_tests.rs) — after a shift
of scredge work on a slow seat: provisional answers exist, `delivered` is still false.

r[cz.craft.wait-on-channel-full+1]

**Normative summary.** Completions stage into a growable per-shift Vec. When the
worker→collector channel is full, the screen worker **calms down and waits**
(`wait_vacant`) until the collector drains — it does **not** clear `delivered`
or reopen Dummy holes. On shutdown interrupt only, unsent answers are restaged
(`restage_unsent_batch`) with `delivered` left true. Throughput yields to the
collector bottleneck; speeding the collector is secondary.

**Code site.** `screen_worker/mod.rs` — `send_update_waiting`, pre-workshift
`is_full`/`wait_vacant`, `restage_unsent_batch`.

**Acceptance criteria.**
- [ ] Channel-full: worker waits; Finals stay delivered; no Dummy reopen from
  undeliver-on-full.
- [ ] Shutdown interrupt restages answers without clearing `delivered`.
- [ ] `push_delivery` still owns buffer slot + `delivered` atomically for stage.

**Test.** `channel_full_restages_without_clearing_delivered`,
`mutant_kill_push_delivery_provisional_not_final` (craftsmanship_tests.rs).

r[cz.craft.clamped-remap-smear+1]

**Normative summary.** Remap sampling clamps at package edges, so seats whose source falls
outside the old frame inherit the nearest edge value — the smear *is* the clamp, not a separate
path.

**Code site.** `src/assemblies/workgroup/work_collector.rs` — `sample_old_values`;
`src/assemblies/headgroup/window/sampling.rs` — `index_from_relative_location` (clamping).

**Acceptance criteria.**
- [ ] A pan that exposes a strip fills it with edge values (not black/Dummy) before new work
  lands.

**Test.** `remap_index_clamps_to_border` (proptest, craftsmanship_tests.rs) — out-of-range
relative locations resolve to exactly the clamped border seat.

r[cz.craft.shared-remap-transform+1]

**Normative summary.** The collector's work remap and the headgroup's RGB sampler use the *same*
transform functions, so work and color can never disagree about where a pixel went.

**Code site.** `work_collector.rs` (`sample_value` calling `transform_relative_location_i32` /
`index_from_relative_location`) and `sampling.rs` (same calls) — shared definitions in
`sampling.rs`.

**Acceptance criteria.**
- [ ] Both call sites resolve to the same functions (no duplicated transform logic).

**Test.** `remap_onto_same_view_is_fixed_point` (craftsmanship_tests.rs) — remapping a package
onto its own view reproduces it exactly through the shared transform.

r[cz.craft.screen-space-derivative-edges+1]

**Normative summary.** Visual edges are detected between screen pixels from derivative fields
carried with the remapped answers. In-filaments extrapolate each local escape field to the
center pixel, then keep the four-neighbor peak test. A flat raw-escape-time neighborhood on
that axis must stay dark — that is the old integer look, and it is what keeps conjugation-axis
exterior tendrils (cusp / bulb rays through smooth bands) from lighting. Near-flat bands
where escape time only differs by one are also dark (boundary speckles). Remapped duplicate
blocks must not thicken into multi-pixel bands.

**Code site.** `workshift.rs` — the `dc = 2*z*dc + 1` recurrence; `color.rs` —
`is_in_filament`.

**Acceptance criteria.**
- [ ] Caught-up views with zero angles match the old raw `slope_sign_changed` oracle cell-for-cell.
- [ ] Conjugation-symmetric flat bands with opposing flank angles light no axis tendril.
- [ ] Near-flat (±1) escape-time bumps stay dark.
- [ ] Monotone exterior fields produce no in-filaments.
- [ ] A true elevated ridge stays exactly one screen pixel wide.
- [ ] A 2-wide remapped duplicate of a ridge lights at most one column (never a thick band).
- [ ] The derivative agrees with finite differences and complex conjugation.

**Test.** `caught_up_view_matches_old_raw_peak_oracle`,
`conjugation_axis_tendril_stays_dark`, `near_flat_escape_delta_one_stays_dark`,
`monotone_exterior_field_never_lights`,
`true_ridge_stays_one_pixel_with_raw_contrast`,
`remapped_duplicate_block_does_not_become_a_thick_band`,
`mandelbrot_dc_matches_ulp_finite_difference`, and `mandelbrot_dc_obeys_conjugation`.

r[cz.craft.stencil-only-replace+2]

**Normative summary.** A pivot `Replace` carries only `frame_info` (loc + zoom + res). The
worker builds an uninitialized context shell from that stencil and materializes each seat's
`c`/`z`/`dc` from a fail-closed `CGenerator` at first start. No seeded point buffer crosses the
channel; construction cost is amortized across the frame's natural start pattern.

**Code site.** `work_controller.rs` — fail-closed stencil pass-through;
`screen_worker/mod.rs` — shell install on Replace (context + `frame_info` paired in the
`LiveTarget` struct, so a second live target cannot exist by construction); `workshift.rs` —
`from_stencil` / `ensure_started`.

**Acceptance criteria.**
- [ ] `WorkerCommand::Replace` contains no `WorkContext`.
- [ ] A fresh shell leaves every seat `initialized == false`.
- [ ] `ensure_started` produces bit-identical `c` to the generator grid.
- [ ] Steady-zoom Replace reuses the previous points vec capacity.
- [ ] The one live target is structural: `WorkContext` and its `frame_info` move together in
  `LiveTarget`, never a bare tuple.

**Test.** `fresh_shell_leaves_seats_uninitialized`,
`ensure_started_matches_generator_bit_for_bit`,
`replace_reuses_points_capacity_and_resets_initialized`.

r[cz.craft.small-channels+1]

**Normative summary.** Inter-actor channels are small (10–50): a promise that the machine
consumes toward the tip, not a resource limit.

**Code site.** `src/main.rs` — channel builder capacities.

**Acceptance criteria.**
- [ ] No workgroup channel capacity exceeds the established range; senders never block on
  stale-consumer buildup (coalescing handles overflow, see `cz.craft.drain-to-newest+1`).
- Note: visible banding from completion-channel backpressure was a product bug in a later era
  (see collected-wisdom tensions) — the promise is kept by *draining*, not by growing buffers.

**Test.** None — channel wiring in `main.rs`; acceptance by code review + e2e.

r[cz.craft.wall-clock-law+1]

**Normative summary.** The shift loop condition is wall time (~10ms); the token accounting is
commented-out vestige scheduled for deletion.

**Code site.** `workshift.rs` — `while … elapsed().as_millis() < 10` (token condition
commented out beside it).

**Acceptance criteria.**
- [ ] Shifts terminate within ~10ms + one bout under all load classes (timed verify).

**Test.** `workshift_always_terminates` (craftsmanship_tests.rs) — structural: shifts return
with empty queues and under non-completing load. Never-stall: `unfinished_synthetic_workshift_never_stalls`,
`unfinished_home_workshift_never_stalls`, `reference_install_mid_fill_keeps_shift_progress`.
The 10ms constant itself stays code-reviewed (timing is not meaningfully unit-testable).

r[cz.craft.emergent-cadence+1]

**Normative summary.** Worker → collector cadence is emergent: every non-empty
shift (or iteration-delta update) sends a `WorkUpdate`; there is no publish
timer inside the worker. Collector → shade publish is the **content beat**
(`resolved_content_period()`): resident work-so-far is emitted on that period
even when shifts are sparse. Smoothness means **continuous outputs** on the
content tier while unfinished; iterate-heavy interior may burn iterations
without finals — that is not a cadence failure.

**Code site.** `screen_worker/mod.rs` — post-shift drain-and-send, every shift;
`work_collector.rs` — content-period publish of resident package;
`naive_gpu/mod.rs` — harvest-every-bout until the shift has published points, then optional
multi-bout amortize only when finals are sparse.

**Acceptance criteria.**
- [ ] While incomplete, worker `WorkUpdate`s track workrate with no fixed interval; when complete,
  worker publishing goes idle (no empty updates).
- [ ] Collector publishes resident packages on the content beat while a package exists.
- Worker-layer never-stall: unfinished frames must show progress every workshift
  (`total_iterations_today` / seat advance / completions).
- Home/shallow GPU fill: after the first completion, ≤5 consecutive shifts without a
  completion while fill is still progressing (≤50 ms at 10 ms/shift).

**Test.** Never-stall suite in craftsmanship_tests.rs (same three tests as wall-clock-law);
`steady_state_naive_gpu_home_continuous_outputs`;
`steady_state_naive_gpu_deep_cusp_never_stalls` (missed resume/empty-queue feed, not tenacity);
`steady_state_naive_gpu_f64_gear_via_faux_user_zoom` (generator-plane F32→F64 escalate).
Actor send loop idle/complete still by
code review + e2e.

r[cz.craft.load-proportional-ignorance+1]

**Normative summary.** The worker is busy exactly while the screen is unfinished:
incomplete (any undelivered seat) → chain shifts with no sleep; complete → park
on the warm wait set (commands/attention/settings/references + slow periodic)
and **do not run a workshift**. The incomplete check must be O(1) after fill
(`seats_need_work`) — full-seat scans on every wake are forbidden.

**Code site.** `screen_worker/mod.rs` — `seats_need_work` + park `await_for_any`;
workshift gated on the same flag.

**Acceptance criteria.**
- [ ] CPU usage measured on a completed frame is ~idle for *workshifts*; on an incomplete frame the worker does
  not sleep between shifts.
- [ ] After every seat is `delivered`, the actor loop skips `workshift` until a Replace
  creates undelivered seats again.
- [ ] Delivered home stays delivered (0 iters) across a 10s post-fill window.
- [ ] Post-settle `CZ_PROFILE_CPU`: `worker_shift_ms≈0` and `worker_loop_ms` stays small under warm wakes.

**Test.** `mutant_kill_complete_frame_has_no_undelivered_seats`;
`steady_state_home_stays_parked_for_10s_after_fill`; headed `CZ_PROFILE_CPU` after settle.

r[cz.craft.drain-to-newest+1]

**Normative summary.** Every workgroup input (stencils, Replace commands, attention) is drained
to newest-only, so backlogs of stale obligations are unrepresentable.

**Code site.** `work_controller.rs` — stencil drain loop; `screen_worker/mod.rs` — command and
attention drains at loop head.

**Acceptance criteria.**
- [ ] Burst test: N rapid stencils result in exactly one context construction (for the newest);
  intermediate poses never become work obligations.

**Test.** None — channel drain loops in two actors; acceptance by code review + e2e burst.

r[cz.craft.pivot-two-message-order+1]

**Normative summary.** On Replace: flush old-context completions (`frame_info: None`) *before*
announcing the new frame (`frame_info: Some`); writes never cross a remap.

**Code site.** `screen_worker/mod.rs` — the `WorkerCommand::Replace` arm, and the collector's
corresponding None-write / Some-remap handling in `work_collector.rs`.

**Acceptance criteria.**
- [ ] Sequence verify: a completion computed under frame A is never written into a package
  remapped to frame B; the collector observes no other message interleaving.

**Test.** None — cross-actor message ordering; acceptance by code review + e2e.

r[cz.craft.kernel-seam+1]

**Normative summary.** The proven screen scheduler is independent of the
numerical implementation it runs. Slot rotation, queues, attention,
neighbor-discovery policy, `Delivery` backpressure, and the wall-clock loop
remain scheduler-owned. A `SeatKernel` may only materialize one seat, run one
`BoutCap`-bounded bout, and map a finished seat to a `CompletedPoint`.
The Naive GPU path uses a **wave API** beside `SeatKernel` (`workshift_naive_gpu`):
same scheduler ownership and `BoutCap` per seat, many seats per dispatch.

**Code site.** `screen_worker/workshift.rs` — `SeatKernel`, `DirectKernel`,
the compatibility `workshift` wrapper, and generic `workshift_with_kernel`;
`screen_worker/naive_gpu/` — wave arm/dispatch/harvest.

**Acceptance criteria.**
- [ ] The restored direct arithmetic lives behind `DirectKernel` without a
  numerical or scheduling change.
- [ ] The scheduler contains no Mandelbrot iteration recurrence or
  period-verification implementation.
- [ ] A second kernel can reuse the same slot/queue/attention/backpressure
  machinery by implementing `SeatKernel`.
- [ ] Kernel dispatch does not regress first-publish or full-frame fitness.

**Test.** `direct_kernel_preserves_scheduler_results`
(`craftsmanship_tests.rs`) compares the compatibility path with explicit
`DirectKernel` dispatch; the full craftsmanship suite pins all scheduler
policies.

r[cz.craft.gpu-host-queue-discovery+1]

**Normative summary.** Naive GPU finals are still host-scheduler events. Every
published Final from `publish_gpu_finishes` must run the same neighbor / edge
queue discovery as CPU (`queue_incomplete_neighbors`,
`queue_incomplete_neighbors_in`, edge push-front). Work announces itself; scan
fill is only a cold-start / empty-queue fallback. **Forbidden:** skipping queue
updates for PPS (including “bulk flood” shortcuts); treating linear undelivered
scan as the steady fill authority; any ≥N% CPU mop / residual phase / seeded
fake `out_queue` to hide Dummy holes. Channel-full wait
(`wait-on-channel-full`) remains honesty, not a mop.
Precision escalate to CPU when shader F64 is unavailable for a collapsed F32
view is allowed for that shift only.

**Code site.** `screen_worker/naive_gpu/mod.rs` — `publish_gpu_finishes` always
queues neighbors; `workshift_naive_gpu` has no percent-based DirectKernel mop.

**Acceptance criteria.**
- [ ] After the first home GPU Final, host out/in/edge queues grow (or are
  already non-empty from discovery).
- [ ] Home fills to 100% delivered on the naive GPU path without a CPU mop gate.
- [ ] Collector grid has no Dummy holes after GPU home fill.

**Test.** `steady_state_naive_gpu_home_neighbor_queues_grow`,
`steady_state_naive_gpu_home_fills_without_cpu_mop`,
`steady_state_naive_gpu_home_no_dummy_holes`,
`steady_state_screen_worker_home_ips_naive_gpu_path`
(craftsmanship_tests.rs).

r[cz.craft.shade-single-path+1]

**Normative summary.** Escaper and colorer each have one frame body. Animated
bailout / animated color params use that same body; only the numbers change.
Do not fork a second “static vs animated” shade path
(`docs/assistant/design/shadergroup-virtues.md`).

**Code site.** `shadergroup/escaper.rs` — `escape_frame`; `shadergroup/colorer`
— `color`.

**Acceptance criteria.**
- [ ] Actor loops call the shared frame functions; no parallel animated-only
  implementation.
- [ ] Criterion `shadergroup_fitness` benches those functions directly.

**Test.** Structural — `escape_frame` / `color` are the measured hot paths;
actor loops delegate to them.

r[cz.craft.shade-coalesce-drop-count+1]

**Normative summary.** When more than one full-frame package is queued into
escaper or colorer, drain-to-newest drops the older ones and increments
`packages_dropped` (HUD `drop:`). Persistent growth means the shade path is too
slow for the pixel count — fix cost, do not enlarge channels.

**Code site.** `coalesce_drop_count` + `take_newest_plan` + `packages_dropped`
on escaper/colorer state; `ViewHud.packages_dropped`.

**Acceptance criteria.**
- [ ] `coalesce_drop_count(n) == n.saturating_sub(1)`.
- [ ] `take_newest_plan(n)` drops `n−1` and takes the tip.
- [ ] HUD exposes cumulative drops for headed audit.

**Test.** `coalesce_drop_count_keeps_newest_only`, `take_newest_plan_drops_all_but_tip` (escaper.rs).

r[cz.craft.content-beat-publish+1]

**Normative summary.** Work collector publishes the resident package on every
content beat (`resolved_content_period`), whether or not new WorkUpdates
arrived since the last publish. Shade stays on the continuum; only the screen
worker parks.

**Code site.** `content_beat_due` + publish branch in `work_collector.rs`.

**Acceptance criteria.**
- [ ] `content_beat_due` is true after the period with no intervening work.
- [ ] Publish path has no “only when work arrived” gate.

**Test.** `content_beat_due_without_new_work` (work_collector.rs).

r[cz.craft.collector-absorbs-all+1]

**Normative summary.** Collector folds every WorkUpdate into the resident
package. Never drain-to-newest on worker→collector (that channel is lossy only
if the actor loop itself skips takes — forbidden).

**Code site.** `absorb_work_update` in `work_collector.rs`.

**Acceptance criteria.**
- [ ] K distinct seat fills across K updates all land in the package.

**Test.** `collector_absorbs_all_seat_updates` (work_collector.rs).

r[cz.craft.shade-always-emit+1]

**Normative summary.** Escaper and colorer always run the resident body and
attempt `try_send` each content wake when values are resident. GPU upload flags
(`answers_need_gpu_upload`) must not gate actor send.

**Code site.** Escaper / colorer actor loops; `r[impl]` markers at emit sites.

**Acceptance criteria.**
- [ ] No skip-send based on “unchanged inputs.”
- [ ] Upload gate is private to GPU buffers only.

**Test.** `shade_always_emits_when_resident_even_without_upload` (escaper.rs).

r[cz.craft.gpu-color-parity+1]

**Normative summary.** Color gear selects OG (CPU `color`) or GPU (f32 wgpu
port). Default is **GPU**; manual settings can force OG or GPU. Same inputs →
exact `Color32` equality. GPU gear falls back to OG only when no usable device
exists (`CZ_FORCE_CPU_COLOR=1` or init failure) — never for missing shader f64.
PPS/kernel gearbox must not auto-pick shade GPU.

**Code site.** `colorer/gpu/` + `color_with_gear` in the colorer actor;
settings `resolved_color_gear` / `manual_color_gear_*`.

**Acceptance criteria.**
- [x] GPU output equals OG on synthetic + home escape_frame fixtures.
- [x] Default path is GPU; unavailable GPU → `GPU→OG` HUD without panic.

**Test.** `gpu_matches_og_color32_default_script`,
`gpu_matches_og_per_layer_scripts`, `gpu_matches_og_home_escape_frame`,
`default_gear_is_gpu` (colorer/gpu).

r[cz.craft.completion-cap-fits-screen+1]

**Normative summary.** Per-shift completion staging is a growable `Vec` (Stec
removed). On Replace enlarge, capacity is at least the new pixel count so a
full-screen flood never needs a fixed ceiling. Channel backpressure is the only
publish throttle (`wait-on-channel-full` at send).

**Code site.** `workshift.rs` — `from_stencil` (fresh and reuse arms).

**Acceptance criteria.**
- [ ] Enlarge via `from_stencil(..., Some(previous))`: capacity ≥ new pixel count;
  one Final per seat can stage.

**Test.** `enlarge_replace_completion_vec_accepts_full_screen`
(craftsmanship_tests).
