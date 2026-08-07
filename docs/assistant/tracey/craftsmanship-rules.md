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
  every `Write`/`Edit`/`StrReplace`. When the target file is under
  `src/assemblies/workgroup/screen_worker/` or `src/assemblies/shadergroup/colorer/`, it
  injects that file's rule summaries (below) as agent context at the moment of the edit —
  the forcing function the docs alone cannot provide. It fails open (never blocks an edit).

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

**Normative summary.** The completion buffer drains newest-first (pop from end), so during a
pivot the freshest work publishes first.

**Code site.** `src/assemblies/workgroup/screen_worker/mod.rs` — `work_update` drains via
`try_pop` on the `Stec`.

**Acceptance criteria.**
- [ ] Property: drain order is exactly reverse push order.

**Test.** `completion_drain_is_lifo` (craftsmanship_tests.rs).

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

r[cz.craft.undeliver-on-full+1]

**Normative summary.** When the completion buffer is full, the seat is marked undelivered and
the shift breaks: backpressure degrades to re-queue, never to a dropped answer.

**Code site.** `workshift.rs` — `WorkContext::push_delivery` (the only method that touches
`delivered`), returning `#[must_use] PushOutcome`; the `BufferFull` arm breaks the shift.

**Acceptance criteria.**
- [ ] Full-buffer simulation: no completed point is lost; affected seats complete on a later
  shift.
- [ ] The buffer slot and `delivered` flag are updated atomically — now type-enforced:
  `push_delivery` owns both, and `#[must_use]` on `PushOutcome` makes dropped backpressure a
  compile error.

**Test.** `full_buffer_undelivers_and_stops` (craftsmanship_tests.rs) — buffer pinned at
100000, completing seat flips back to undelivered, nothing lost.

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
with empty queues and under non-completing load. The 10ms constant itself stays code-reviewed
(timing is not meaningfully unit-testable).

r[cz.craft.emergent-cadence+1]

**Normative summary.** Publish cadence is emergent: every non-empty shift sends; there is no
publish timer or gate.

**Code site.** `screen_worker/mod.rs` — post-shift drain-and-send, every shift.

**Acceptance criteria.**
- [ ] While incomplete, publishes track workrate with no fixed interval; when complete,
  publishing goes idle (no empty publishes).

**Test.** None — actor send loop; acceptance by code review + e2e.

r[cz.craft.load-proportional-ignorance+1]

**Normative summary.** The worker is busy exactly while the screen is unfinished: incomplete →
chain shifts with no sleep; complete → sleep on the 50ms/command wait.

**Code site.** `screen_worker/mod.rs` — `max_sleep` 50ms and the idle/chain branch on
`percent_completed`.

**Acceptance criteria.**
- [ ] CPU usage measured on a completed frame is ~idle; on an incomplete frame the worker does
  not sleep between shifts.

**Test.** None — actor sleep branch; acceptance by code review + e2e (CPU measurement).

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

**Code site.** `screen_worker/workshift.rs` — `SeatKernel`, `DirectKernel`,
the compatibility `workshift` wrapper, and generic `workshift_with_kernel`.

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
