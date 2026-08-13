# Depth core rules

These rules cover the perturbation core through the final compute-gear phase.
See `../design/depth-design.md` and `../issue-stack.md`.

## Vocabulary (normative)

| Meaning | Name | Forbidden |
|--------|------|-----------|
| Absolute Mandelbrot parameter | `c` | `plane_c`, “plain c” |
| Absolute iterate | `z` | `plane_z`, “plain z” |
| Reference parameter | `reference_c` | bare “c” when context is the reference |
| Reference iterate at step n | `reference_z` (`orbit.get(n)`) | bare “z” when meaning Z_ref |
| Seat−reference sample / perturbation δc | `delta_c` | `little_c` |
| Perturbation δz | `delta_z` | `little_z` |
| Escape derivative ∂z/∂c | `dc` (derivative only) | conflating with `c` / `delta_c` |

**Invariant.** Relative shell + live reference: recurrence uses `delta_c` + `reference_z`.
Zero-orbit / soft-continue: the δc slot holds absolute `c` (same math as naive). Never put
generator `delta_c` in that slot — that marks exterior as interior (flat black / “in”).
Do not iterate deep relative seats with collapsed f64 absolute `c` alone — that is blocky.

r[cz.depth.c-generator-fails-closed+1]

**Rule.** Each host type carries [`HostPrecision`] (significand bit count and
exponent floor). The C-generator admits that type when the bit count covers
`|c|` magnitude down to pixel pitch (plus optional slider margin, default **1**).
No near/far `T` probe — the gate is the count. On success store `origin` and
`space` as `T`; hot `get_c` is pure `T` multiply-add. Grid is v0.0.9 top-left,
no-half-pixel. Relative subtracts the anchor in exact `IntExp` before the same
count. Naive GPU F32 is a second type (24 bits) and must not run when the
count exceeds 24 while f64 (53) still admits.

**Implementation.** `src/assemblies/workgroup/c_generator.rs` — `HostPrecision`,
`Mandelbrotable::PRECISION`, `stencil_bits_needed`, `CGenerator::new_with_margin`.
Settings slider `c_generator_margin_bits` (default 1). OG naive CPU uses the same
`DirectKernel` / `Mandelbrotable` iterate for f32, f64, then `CopyIntExp<1>`
when absolute f64 fails the bit-count gate.

**Verification.** `generator_matches_v009_grid_bit_for_bit`,
`rejects_collapse_at_far_end`, `successful_generator_has_distinct_neighbors`,
`relative_generator_subtracts_before_narrowing`, `from_stencil_carried_ref_anchors_to_ref_c`,
`reference_install_rebuilds_c_generator`,
`home_f64_absolute_wall_moves_earlier_with_default_margin`,
`margin_bits_zero_matches_prior_distinguish_only_admit`,
`og_naive_f32_uses_same_direct_kernel`,
`og_naive_copy_intexp1_uses_same_direct_kernel`,
`home_copy_intexp1_admits_after_f64_wall`,
`og_copy_intexp1_naive_flips_on_after_home_f64_wall`,
`og_copy_intexp1_headed_mag_43_not_all_interior`,

r[cz.depth.relative-coords+1]

**Rule.** Relative/`delta_c` coordinate generation must admit only when the narrowed type
keeps a legal path (distinct neighbors with render headroom) or hard-wall refusal —
home f64 admission must not soft-fail into an illegal relative grid.

**Implementation.** `src/assemblies/workgroup/c_generator.rs` — relative admit / home path.

**Verification.** `home_f64_admission_has_legal_path_or_hard_wall`.

r[cz.depth.floatexp-range+1]

**Rule.** Per-pixel deltas and stored reference iterates use a normalized f64 mantissa plus
i64 exponent. Values far below f64's exponent floor remain nonzero; arithmetic agrees with
high-precision rug arithmetic to stored-mantissa precision. Zero has correct ordering against
arbitrarily small positive/negative values.

**Implementation.** `src/floatexp.rs` — `FloatExp`, `ComplexFloatExp`.

**Verification.** `add_and_multiply_agree_with_rug`,
`does_not_underflow_far_beyond_f64`, `zero_is_canonical_and_exact`,
`deep_delta_runs_without_f64_underflow`.

r[cz.depth.reference-low-storage+1]

**Rule.** Reference iterates are computed in depth-appropriate rug precision but stored as
floatexp. Only constant-size high-precision state is retained: the tail and Brent
cycle-detector cursors, so extension and exact cycle detection resume across bouts without
an unbounded full-precision history. Proven periodic/preperiodic references index
indefinitely by wrapping their finite cycle; escaping/nonperiodic references refuse unknown
indices.

**Implementation.** `src/reference.rs` — `ReferenceOrbit::{compute,extend,get}`,
`bits_for_zoom`.

**Verification.** `stored_orbit_matches_full_precision_rounding`,
`extending_matches_one_shot`, `periodic_and_preperiodic_orbits_index_forever`,
`escaping_reference_is_finite_and_honest`.

r[cz.depth.perturb-never-wrong+1]

**Rule.** Delta iteration implements Δz' = 2ZΔz + Δz² + Δc. Missing reference work,
loss-of-significance at the bailout circle, and Pauldelbrot glitches never become
guessed Mandelbrot answers. These are **distinct** honest outcomes (library
`PerturbedOutcome` in `src/perturb.rs`):

- **Missing iterate** (`orbit.get(n) == None`) → unfinished / soft-continue. Not a
  glitch. Switch to the zero-orbit floor with `delta_z ← z` (reconstructed absolute
  iterate) and `delta_c ← c` (absolute parameter), and keep iterating — same
  recurrence, no invented answer.
- **Pauldelbrot glitch** → unfinished; rebind that seat to the zero-orbit floor
  (reset; do not trust a corrupted reconstruct). Never publish a guessed answer.

**Implementation.** `src/perturb.rs` — `iterate_pixel`, `PerturbedOutcome::{Glitch,
Unfinished}`; `perturb_kernel.rs` — `rebind_to_zero_continuing` (exhaustion) vs
`reset_for_glitch` (Pauldelbrot).

**Verification.** `missing_reference_is_unfinished_not_wrong`,
`missing_reference_iterate_stays_unfinished`,
`glitch_sets_direct_only_and_never_publishes_guess`,
`perturbation_matches_precision_doubling_oracle_for_exteriors`.

r[cz.depth.oracle-doubling+1]

**Rule.** The test-only naive oracle starts with enough rug bits to represent the dyadic input
exactly, then doubles precision until two answers agree. Starting at a fixed low precision is
forbidden: two insufficient precisions can agree only because both erased the same deep bit.
A concluded oracle must be matched exactly or the perturbation path must report itself
honestly incomplete.

**Implementation/verification.** `src/perturb.rs` test module —
`doubling_oracle`, `perturbation_matches_precision_doubling_oracle_for_exteriors`,
`deep_delta_runs_without_f64_underflow`.

r[cz.depth.reference-bout-law+1]

**Rule.** Arbitrary-precision reference extension is resumable and checked
against a wall-clock budget between individual iterations. A newer request
therefore gets control after at most the current arithmetic iteration; no
multi-iteration unbounded call is made by the actor.

**Implementation.** `src/reference.rs` — `ReferenceOrbit::extend_for`;
`src/assemblies/workgroup/reference_worker.rs` — `work_for` and the 10ms actor
bout.

**Verification.** `bout_sliced_extension_matches_one_shot`,
`one_step_bouts_preserve_period_and_preperiod_detection`,
`zero_budget_does_no_work`.

r[cz.depth.reference-latest-wins+1]

**Rule.** Reference requests are drained to newest before work starts. Replacing
a request discards the in-progress target; there is exactly one live reference
job and no backlog of stale computation.

**Implementation.** `reference_worker.rs` — `ReferenceWorkerState::replace`
and the input drain loop.

**Verification.** `newer_request_replaces_in_progress_job`.

r[cz.depth.reference-sticky-selection+1]

**Rule.** Reference selection happens exactly once per screen pivot. It chooses
the deepest delivered non-escaped interior seat known from the prior live view,
or the new view center when none exists. Progress within a view never
reselects. Precision is computed once from the new view depth.

**Implementation.** `reference_worker.rs` — `select_reference_request`;
`screen_worker/mod.rs` — the `Replace` arm is the sole caller.

**Verification.**
`selection_uses_deepest_completed_interior_then_center_fallback`,
`precision_is_chosen_once_from_new_view_depth`.

r[cz.depth.reference-whole-snapshot+1]

**Rule.** A reference publication owns one complete `ReferenceOrbit`, its exact
objective c, and a monotonically advancing generation. The screen worker installs
the latest snapshot into the live `WorkContext` for the delta kernel; the worker
never blocks waiting for a reference (the zero-orbit floor always runs).

**Implementation.** `reference_worker.rs` — `PublishedReference` and
`ReferenceWorkerState::work_for`; `screen_worker/mod.rs` — pending/live install;
`WorkContext::latest_reference`.

**Verification.**
`publication_moves_one_complete_snapshot_and_increments_generation`.

r[cz.depth.delta-kernel+1]

**Rule.** Delta iteration lives behind the `SeatKernel` seam as
`PerturbationKernel`. The golden scheduler (queues, attention, backpressure,
publish protocol) is untouched. Per-seat state (`DeltaState`) is resumable
across `BoutCap` bouts.

**Implementation.** `screen_worker/perturb_kernel.rs`; `workshift.rs` —
`DeltaState`, `Point::{delta,direct_only}`, `WorkContext::latest_reference`.

**Verification.** `zero_orbit_floor_matches_direct_kernel_escape_times`,
`published_reference_matches_direct_on_shallow_view`,
`perturbation_kernel_matches_rug_doubling_oracle`,
`perturbation_bout_obeys_cap_and_split_bouts_match`,
`phase_two_perturbation_test_inventory_is_present`,
`pin_exterior_not_marked_in_at_zoom_52`,
`pin_not_blocky_delta_c_at_zoom_49`.
Shallow DirectKernel comparisons are data-flow checks only; deep truth is the
rug precision-doubling oracle.

r[cz.depth.glitch-is-unfinished+1]

**Rule.** A Pauldelbrot glitch permanently rebinds that seat to the zero-orbit
floor (`direct_only`) through the same delta code path for the current
published generation. The seat is reset unfinished; it never publishes a guessed
answer from the glitched delta. A *newer* published generation may clear the
bind and retry (exhaustion/false sticky poison must not outlive a retarget).

**Implementation.** `perturb_kernel.rs` — `reset_for_glitch`, `bound_zero_generation`,
`maybe_clear_zero_bind`.

**Verification.** `glitch_sets_direct_only_and_never_publishes_guess`.

r[cz.depth.reference-until-done+1]

**Rule.** The reference worker publishes only when the orbit has an honest
terminal state (period found or escaped). There is **no artificial length wall**
(`max_iterations` / `MAX_BOUT` as a publish target). Incomplete interiors keep
the zero-orbit floor until then. Extension is wall-clock bout-sliced
(`r[cz.depth.reference-bout-law+1]`), same interruptibility as seats — not a
length cap. Matches `r[cz.tenacious.no-max-iter+1]`.

**Implementation.** `reference_worker.rs` — `work_for` done = period || escaped;
`ReferenceRequest` carries `c` + precision only.

**Verification.** `never_publishes_a_finite_incomplete_orbit`,
`publication_moves_one_complete_snapshot_and_increments_generation`.

r[cz.depth.reference-coverage+1]

**Rule.** A previous published reference may be carried across a pivot only while
its `c` still lies inside the new viewport. Uncovered sticky refs are dropped
(zero-orbit interim). Sticky selection likewise falls back to the new center when
the previous deepest interior is outside the new frame. Prevents classic
glitch-blob clusters when zooming into hard areas / minibrots (dead-reckon goto
to the same place stays clean).

**Implementation.** `reference_c_covers_frame`; `from_stencil` carry filter;
Replace pending install gate; `select_reference_request` coverage filter.

**Verification.** `sticky_selection_drops_interior_outside_new_view`,
`coverage_accepts_center_of_same_view`,
`faux_user_zoom_to_hard_minibrot_matches_direct`.

r[cz.depth.reference-generation-restart+1]

**Rule.** A seat whose `delta.generation` differs from the installed reference's
generation restarts its delta at zero. Stale deltas never survive a retarget.

**Implementation.** `perturb_kernel.rs` — `start_seat` generation guard.

**Verification.** `generation_mismatch_restarts_delta`.

r[cz.depth.floatexp-host-coords+1]

**Rule.** Seat samples relative to the generator anchor (`coord_anchor`) are `delta_c`.
Absolute `c` for naive/zero-orbit is `reference_c`/`anchor + delta_c` when the generator
is relative. Anchor is `reference_c` when a reference scoped the generator, else view
center. Live shallow/mid actors use `f64` host seats when the generator admits them; deep
admission may use relative f64 or FloatExp host in tests / deep path.
`FloatExp.mantissa` remains f64 by design. Render/`Answer` may narrow at the
collector. Mathematical deltas and stored reference iterates remain FloatExp
storage regardless of host type.

**Implementation.** `from_stencil` relative generators; `c_from_delta_c_*` /
`c_for_seat_*`; screen worker monomorphized to f64 for live; FloatExp kernel
module for depth tests.

**Verification.** `deep_frame_admitted_past_f64_collapse`,
`production_plane_coords_are_not_plain_f64`,
`objective_c_matches_relative_generator_plus_anchor`,
`home_reference_request_matches_c_generator`,
`pin_exterior_not_marked_in_at_zoom_52`,
`pin_not_blocky_delta_c_at_zoom_49`.

r[cz.depth.series-approximation+1]

**Rule.** Series approximation is **on the production path**: always-on (no
enable/disable branch for “worth it”); a published reference includes series
coefficients fused one step per reference iterate; seat **initialization** may
skip a safe prefix by evaluating the series in `delta_c`, then resume ordinary
delta iteration. Skip never invents a final answer. Skip discovery is part of
point init — it must be so cheap it is effectively free (binary search over the
orbit / O(log N) evals; large `|δc|` is an immediate no-op). It must **not**
steal bout budget from iterate workshifts. Deep zoom is the payoff;
shallow/home must not pay meaningful overhead when the skip is useless.
Membership pins `pin_exterior_not_marked_in_at_zoom_52` and
`pin_not_blocky_delta_c_at_zoom_49` must stay green with SA on — never
`#[ignore]` (`docs/assistant/quality-doctrine.md`).

**Normative performance contract (developer 2026-08-11).**
- Win target: deep zoom (long orbits, large safe skips).
- Always run; do not gate SA behind a heuristic that costs more than the probe.
- Probe cost: O(log orbit_len) series evaluations — not a linear walk of every n.
- Easy cases: overhead in the noise next to seat start (tiny bit unavoidable;
  not a meaningful fraction of seat time; does not displace iterate bouts).
- Coeff build: one series step **per reference iterate**, rolled into the same
  reference loop (not a separate process/pipeline). Allowed to add a little
  math; must not change the big-O of reference work beyond the necessary
  per-iterate series recurrence. Airtight performance mindfulness from the
  start — no “correct then optimize.”
- Gear promotion mid-orbit does **not** force seat restart or series re-init;
  restart remains reference-generation / glitch / unbound paths (existing
  delta.generation contract).

**Rejected prior shape.** Linear `safe_skip` scanning n=1..=max with full
`evaluate` each step; heap `Vec<Vec<_>>` coeff rows; FloatExp-only skip from the
f64 kernel path; unbounded-at-init cost. That attempt was also yanked for
membership correctness before it could be measured cleanly.

**Implementation.** `src/series.rs` (`SeriesBuilder` + flat `SeriesApproximation`);
fused in `ReferenceOrbit::extend_inner`; published on `PublishedReference.series`;
`apply_series_skip` after `init_delta` in `perturb_kernel.rs` /
`perturb_floatexp.rs`. Design intent: `docs/assistant/design/depth-design.md`,
decisions `D-SERIES-*` in `docs/assistant/unit-design/decisions.md`.

**Verification.** `series_approximation_wired_into_production_kernels`,
`series_safe_skip_eval_count_is_logarithmic`,
`series_shallow_probe_stays_nearly_free`,
`series_deep_skip_is_material_on_long_orbit`,
`series_skip_matches_delta_tail`,
`series_never_publishes_guessed_completion`,
`live_series_skip_initializes_delta_prefix`,
`published_reference_with_series_matches_direct_outside_r2`,
plus membership pins above (must stay green with SA on — no ignore).

r[cz.depth.oracle-gear+1]

**Rule.** The FloatExp absolute (“slidy”) **Oracle** gear exists only for tests
and benches. It iterates `z ← z² + c` without perturbation, reference, or
series. Production `workshift` dispatch must never select it. Deep membership
parity uses Oracle (or rug doubling), not f64 `DirectKernel`.

**Implementation.** `src/gearbox/oracle.rs` — `OracleKernel`, `iterate_oracle_bout`.

**Verification.** `oracle_escapes_far_exterior`,
`oracle_matches_direct_escape_time_on_shallow_sample`,
`oracle_marks_cardioid_center_repeat`,
`deep_relative_exterior_not_instant_black_at_reported_location`,
`production_workshift_never_dispatches_oracle_gear`.

r[cz.depth.compute-gear+1]

**Rule.** Per-pixel delta recurrence uses the compute gear ladder F64 →
ScaledF64 → FloatExp. A delta at a gear's underflow/overflow floor promotes;
it is never silently flushed to zero or rounded into a guessed completion.
Zero-orbit F64 skips the `2Z·δz` term (Z=0). Legal mid-orbit promotions only;
no reverse transition unless a separately proven reconstruction exists.
Aggregate HUD gear may be MIXED when seats disagree.

**Implementation.** `src/delta_gear.rs`; `DeltaState.gear` / `scale`;
`perturb_kernel` gear branches; `refresh_active_gear`.

**Verification.** `gear_promotes_at_f64_underflow_floor`,
`scaled_f64_matches_floatexp_on_moderate_delta`,
`zero_orbit_f64_skips_two_z_term`,
`aggregate_seat_gears_reports_mixed`,
`f64_gear_zero_orbit_center_reports_period_one`,
`f64_gear_home_fills_without_per_seat_gear_scan`.

r[cz.depth.gear-hud+2]

**Rule.** The HUD displays host stack, kernel mode (`naive`|`pert`), reference status
(`wip`|`complete`), effective active compute gear, rolling IPS and PPS, and view **IPP**
(mean iterations per seat). IPP is a property of the live view: it is known only as seats
iterate, and is final when every seat is delivered. It is not a rate (that is IPS).
Mode names which production kernel runs: **`naive`** = `DirectKernel`; **`pert`** =
`PerturbationKernel`. Gear applies under **`mode:pert`** only (delta ladder); naive
absolute stamps `F64` even when the host stack is `i64` (`CopyIntExp<1>`).
Read `stack:` for the tape, not `gear:`. Ref is a running snapshot:
`wip` when no usable published reference exists yet or any seat is in `direct_only` glitch
recovery awaiting a newer reference generation; `complete` when a usable reference is
installed and no seats are glitched. Mixed-seat views surface MIXED rather than a false
single gear. No user setting selects the gear. Metrics overlay stays top-left;
location/goto panel bottom-right (`r[cz.ui.coords-parse+2]`, `r[cz.ui.location-readout+2]`).

**Implementation.** `WorkUpdate` telemetry → collector → window HUD overlay;
`PpsCounter` / iteration accounting in `rolling.rs`.

**Verification.** `hud_telemetry_carries_gear_and_rates`,
`pps_counter_counts_completions_not_wip`, `telemetry_mode_naive_then_pert`,
`reference_complete_with_reused_ref`, `reference_wip_while_started_seats_await_ref`,
`reference_wip_after_glitch_until_new_generation`,
`naive_f64_north_tip_mag_38_still_escapes`,
`og_copy_intexp1_headed_mag_43_not_all_interior`,