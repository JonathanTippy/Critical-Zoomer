# Standards Tracey rules (assistant-owned mapping of the standards hard bars)

Not authoritative text. Hard asserts only — no ignore waives.
Normative source: the standards doc, moved to `docs/assistant/Trash/stale/standards.md` in the 2026-08-06
revert cleanup — the *bars* remain the product's standards; the file's new home reflects that
its tile-era measurement classes (TPS etc.) were written for the old machine.

> **2026-08-06 revert note.** Codebase is now v0.0.9 (e6a0560). Classes marked per rule:
> - **STANDS** — a hard bar v0.0.9 already embodies; re-verify against restored symbols.
> - **SUSPENDED** — a bar defined on tile-era machinery (TileSession workshifts, GPU compute,
>   reference orbits, lookahead). It returns with the GPU/depth port and must then be met
>   *without* breaking the v0.0.9 invariants (see `docs/assistant/design/workgroup-virtues.md`).
> All checkboxes earned on the tile machine were cleared.

r[cz.perf.foveation-half-time+1]

**Normative summary.** Half of available working time fills the current view; half goes to lookahead.

**Acceptance criteria.**
- **SUSPENDED.** v0.0.9 has no lookahead — all working time fills the current screen, and the
  "hoard" is the remap-restored previous screen. The 50/50 policy returns only if lookahead
  returns, and then must extend the one-package discipline rather than fragment it.

r[cz.perf.home-100tps+1]

**Normative summary.** Home view at default resolution must fill fast (the tile-era TPS
addendum expressed this as tiles/sec).

**Acceptance criteria.**
- [ ] STANDS as a fill bar, re-expressed for a tileless workgroup: home view reaches oracle
  quality within 5s headed (cross-link the e2e home-fill rules), with no flat-black mid-wait.
  A numeric seats/sec class can be re-derived if wanted; the headed bar is primary.
- **Benchmark.** `time_to_full_frame` in `benches/workgroup_fitness.rs` tracks the wall-clock
  home fill against the committed baseline in `docs/assistant/benchmarks.md`. (As of the
  2026-08-06 baseline the full frame is ~12.4s — over the 5s headed bar, dominated by the §12
  period-refinement apparatus; recorded, not waived.)

r[cz.perf.min-300m-ips-cpu+2]

**Normative summary.** ≥300M iterations/s on single-core CPU in real workgroup conditions,
scheduling overhead included.

**Acceptance criteria.**
- **SUSPENDED as a number.** The 300M figure was derived for the tile-era machine and awaits
  re-derivation on the view pipeline.
- **STANDS as a method.** IPS is measured full-stack by `full_stack_ips` in
  `benches/workgroup_fitness.rs` (real workgroup loop, scheduling included) and tracked against
  the committed baseline in `docs/assistant/benchmarks.md`; microbenches are diagnostics only.

r[cz.perf.min-30b-ips-gpu+1]

**Normative summary.** GPU iterations/s in real workgroup conditions must track the
machine's measured FLOP advantage over a single CPU core. No adapter ⇒ fail.

**Acceptance criteria.**
- **SUSPENDED as an absolute number.** The 30B figure was a tile-era / one-class
  stand-in; do not treat it as the live bar.
- **STANDS as a method (2026-08-08).** Measure CPU single-core FLOPs and GPU total
  FLOPs on the same machine; full-stack naive `IPS_gpu / IPS_cpu` must match that
  ratio within about ±20% on iterate-heavy work (scheduling included). Design:
  `docs/assistant/design/naive-gpu-design.md`, decision D-NGPU-5. Absolute billions
  may be re-derived later per machine class; they do not replace the ratio method.

r[cz.perf.optimal-ipp+1]

**Normative summary.** Iterations per point equal optimal (out → escape time; in → preperiod + period).

**Acceptance criteria.**
- [ ] STANDS — v0.0.9 is the reference: exterior points stop at bailout, interior points stop
  at loop detection, and resumable points never redo finished iterations. Re-verify on sampled
  loci. (Period *refinement* cost is §12 cleanup material, not a violation of this bar.)

r[cz.perf.headgroup-shaders-2ms+1]

**Normative summary.** Headgroup shaders together ≤2ms frametime at 1080p. No adapter ⇒ fail.

**Acceptance criteria.**
- [ ] Re-verify on the restored shadergroup (escaper + colorer) at 1080p.

r[cz.perf.headgroup-vsync+1]

**Normative summary.** Vsync / PresentMode::Fifo enabled; no janky forced FPS cap.

**Acceptance criteria.**
- [ ] Re-verify on the restored window path (no explicit present-mode code was found in the
  v0.0.9 tree — confirm what the windowing framework defaults to and pin it).

r[cz.ctrl.zoom-in-homothety+1]

**Normative summary.** Zoom-in: magnification pot += 1; location ← (L − P)/2 + P (pointer-fixed).

**Acceptance criteria.**
- [ ] STANDS. `inputs.rs` shifts by zoom_pot around the pointer. Re-verify the
  complex-under-pointer invariant per bump, zoom-out inverse.

r[cz.ctrl.scroll-up-zooms-in+1]

**Normative summary.** Scroll up ⇒ zoom in ⇒ magnification pot +1.

**Acceptance criteria.**
- [ ] STANDS. Re-verify scroll polarity on restored `inputs.rs`.

r[cz.perf.play-minimize+1]

**Normative summary.** Aggressively minimize play: no or very small initialization
phases; continuous delivery of work so far.

**Acceptance criteria.**
- [x] Production home first publish within 20% of DirectKernel
  (`home_workshift_first_publish_within_20pct_of_direct_kernel`).
- [x] Criterion `time_to_first_publish` measures fill-only via `iter_custom`
  (not context construction).
- **Benchmark.** `time_to_first_publish` in `benches/workgroup_fitness.rs` tracks
  first-work latency against the baseline (`docs/assistant/benchmarks.md`).

**Verification.** `home_workshift_first_publish_within_20pct_of_direct_kernel`,
`home_workshift_stays_on_direct_kernel_without_ref`.

r[cz.perf.play-8bump-100ms+1]

**Normative summary.** After the user zooms in 8 bumps at a time, some new work
must be visible within 100ms of the last bump of the gesture.

**Acceptance criteria.**
- [ ] STANDS as a bar. v0.0.9's mechanism: drain-to-newest makes the 8 bumps collapse to one
  target, remap shows old work immediately, fresh work lands within shifts. Re-verify headed.
  First-publish latency is tracked by `time_to_first_publish` vs baseline
  (`docs/assistant/benchmarks.md`).

r[cz.play.actor-poll+1]

**Normative summary.** Each actor checks its input channel at a quick pace at the
start of its loop.

**Acceptance criteria.**
- [ ] STANDS — v0.0.9 embodies it: the worker returns to its loop head every ≤10ms shift;
  controller/collector wake on messages or a 50ms pulse. Re-verify per actor.

r[cz.play.actor-drain+1]

**Normative summary.** Each actor fully drains its channel when anything is there.

**Acceptance criteria.**
- [ ] STANDS — drain-to-newest on every workgroup input is the load-bearing pattern
  (virtues §2). Re-verify per channel.

r[cz.play.latest-wins+1]

**Normative summary.** Actors immediately prioritize the most recent work over
previous work. The display side must still ingest every unique published snapshot —
neither dropping work nor getting behind are acceptable.

**Acceptance criteria.**
- [ ] STANDS — v0.0.9 embodies both halves: latest-wins on inputs (coalescing), and the
  collector applies every update in order with the pivot handshake guaranteeing none cross a
  remap (virtues §6). Re-verify.
- Revert note: the old headgroup exception clause ("ingest all unique tiles") is reworded for
  a tileless pipeline; the intent — display never drops published work — is unchanged.

r[cz.perf.home-10000tps-gpu+1]

**Normative summary.** Home view at default resolution on GPU must meet a high fill-rate class.

**Acceptance criteria.**
- **SUSPENDED** until the GPU compute port exists; then re-derive the number for the
  view-based pipeline.

r[cz.math.perturbation-naive-oracle+1]

**Normative summary.** Perturbation answers must exactly match a trusted naive
oracle (doubling precision until stable) at home view and several well-known
sites. Compare the entire Answer, not merely the result class.

**Acceptance criteria.**
- **SUSPENDED** until perturbation returns. Note: at v0.0.9 the roles invert — the restored
  f64 iterator *is* the trusted oracle for any future perturbation path. Also cross-link the
  normalization NaN watchlist item (RecipLn = ln(1/x) is the golden behavior to match).

r[cz.ref.zero-orbit-same-path+1]

**Normative summary.** Reference data handles looping points; a const zero big-Z
orbit exists; when no better reference is available that const orbit is used on
the same code path (no alternate algorithm branch).

**Acceptance criteria.**
- [x] `ReferenceOrbit::zero_orbit` is the floor reference (Z_n = 0, period 1).
- [x] `PerturbationKernel` uses it when no published reference exists and for
  `direct_only` (post-glitch) seats — same delta recurrence, never `DirectKernel`.
- [x] `zero_orbit_center_reports_period_one`,
  `zero_orbit_floor_matches_direct_kernel_escape_times`.

r[cz.pub.gpu-native-work+1]

**Normative summary.** Completed work remains GPU-native through the publisher
path so easy full-screen cases keep throughput.

**Acceptance criteria.**
- **SUSPENDED** until GPU compute exists. When it returns, applies to the **view**
  pipeline (`docs/assistant/design/naive-gpu-design.md`): device-resident progress,
  no full payload readback on the hot path, host reads only tiny done signals
  (D-NGPU-6).

r[cz.perf.headgroup-stable-path+1]

**Normative summary.** Sample+shade take one path every frame; no frametime
change when panning vs stationary; sampling shader always the same path.

**Acceptance criteria.**
- [ ] STANDS as a bar; re-verify on the restored headgroup. v0.0.9's single publish path and
  shared remap transform are the mechanism that makes this achievable.

r[cz.perf.one-kernel-path+1]

**Normative summary.** *Transitional milestone — superseded by
`r[cz.perf.pps-selected-kernel+1]`.* During the perturbation correctness push, production
shipped `PerturbationKernel` only (including zero-orbit floor). Destination: dual production
dispatch — `DirectKernel` for naive when fast/legal, `PerturbationKernel` for pert.

**Acceptance criteria (historical).**
- [x] Perturbation correctness path landed with zero-orbit floor and gear ladder.
- [ ] Superseded by `pps-selected-kernel+1` dual dispatch acceptance.

r[cz.perf.pps-selected-kernel+1]

**Normative summary.** Per view, run the legal stack (host type + kernel mode)
that maximizes completed points per second for outstanding work. Stack is
view-global and dead-reckoned from `CGenerator` admission. Kernel mode is
view-global: **measure** legal candidates (Naive CPU, Naive GPU when present,
Perturbation) and lock the highest PPS — do not assume GPU is fastest;
hard-bump to perturbed when naive cannot be honest; stick with the winner
until Replace / type change / ref generation change / cooldown re-probe /
manual gear override.

**Acceptance criteria.**
- [x] Dual production kernel dispatch: `DirectKernel` / `PerturbationKernel` /
  naive GPU (`run_workshift_kernel`).
- [x] Shallow home without a reference stays on DirectKernel (no silent pert trial).
- [x] Production `workshift` home wall time within 20% of DirectKernel
  (`home_workshift_full_frame_within_20pct_of_direct_kernel`).
- [x] PPS race picks highest measured sample (`pps_probe_locks_highest_measured_kernel`);
  GPU is not preferred by policy order alone.
- [x] Re-evaluate locked winner every ~100ms (`pps_probe_reevaluates_after_interval`).

**Verification.** `home_workshift_stays_on_direct_kernel_without_ref`,
`home_workshift_full_frame_within_20pct_of_direct_kernel`,
`telemetry_mode_naive_then_pert`,
`steady_state_home_pps_gpu_vs_cpu_ratio`,
`pps_probe_locks_highest_measured_kernel`,
`pps_probe_reevaluates_after_interval`.
