# Depth design: perturbation with a background reference worker

Status: **partially implemented** — reference worker, perturbation kernel, gear ladder,
HUD telemetry, and **live series approximation** (fused coeff steps + O(log N)
seat-init skip) are in the tree (`r[cz.depth.series-approximation+1]`; developer
interview 2026-08-11). **In progress:** CGenerator admission wiring,
reference-scoped generator rebuild, and PPS-selected naive vs pert dispatch
(`r[cz.perf.pps-selected-kernel+1]`).
The per-pixel path uses the **fastest compute gear whose range admits the
delta**. Research digests live
in the private sister repo `Critical-Zoomer-Math-Library` (not published with
CZ). This design is constrained by `workgroup-virtues.md` and must not
re-break the v0.0.9 invariants (issue-stack standing rule).

## What this buys

Plain f64 pixel coordinates die around zoom 2^-50-ish of pixel spacing and
absolutely near 2^-1022. The executive target is 2^3600000
(`r[cz.deep.min-zoom-pot-capacity+1]`). The only workable way there: iterate
one **reference orbit** at full precision, and iterate every pixel as a
low-precision **delta** against it when naive direct iteration cannot be honest at
depth. Shallow views default to **naive** (`DirectKernel`) when faster
(`r[cz.perf.pps-selected-kernel+1]`). Background reference worker has no user
toggle (`r[cz.seamless.reference-background+1]`).

## Vocabulary

Normative names (see depth-rules vocabulary table): absolute parameter/iterate are `c`/`z`;
reference ones are `reference_c`/`reference_z`; seat−reference samples are `delta_c`/`delta_z`;
escape derivative stays `dc`.

- **Reference orbit**: the orbit of one `reference_c`, computed at depth-appropriate arbitrary precision
  (MPFR/rug), stored low-precision. Singular, current, owned by the reference worker.
- **Delta orbit**: per-pixel, `z = reference_z_n + delta_z_n`; `delta_z` and `delta_c`
  are tiny and carry the depth in their exponents. Zero-orbit/soft-continue puts absolute `c`
  in the `delta_c` slot (never generator `delta_c`).
- **Floatexp**: f64 mantissa + wide integer exponent. The **storage** type for reference
  iterates and mathematical deltas. Range, not mantissa, is what depth demands.
- **Compute gear (kernel):** production seat-worker path — naive `DirectKernel`,
  `PerturbationKernel`, Naive GPU, etc. **Not** a numeric type. Gearbox picks
  among admitting kernels by expected PPS (`r[cz.perf.pps-selected-kernel+1]`).
- **Compute type / delta ladder:** numeric representation used inside a kernel
  for the hot recurrence (f64, scaled-f64, or full FloatExp under pert). Storage
  stays FloatExp; each gear picks the **smallest type** C-generator admits
  (with ~10-bit render margin).
- **Reference worker**: a new actor that computes/extends reference orbits in the background.
  Not "rebasing" — that word means a mid-orbit numerical fold (z ← z+Δz), a different,
  smaller technique; this design contains no world-stopping recomputation.

## The reference worker

A fourth actor beside headgroup / workgroup / shadergroup, obeying the same discipline as the
workgroup's screen worker (virtues §4, §7):

- **Latest-wins input.** Its input is a coalescing channel of reference requests (target
  location + required precision + required orbit length). New pivot → drop in-progress work
  immediately, start the new thing. No queue, no backlog, no slack — same rule as the
  workgroup's replace-and-remap pivot (virtues §2).
- **Interruptible bouts.** Arbitrary-precision iteration is internally sliced into wall-clock
  bouts (~10ms, same law as `r[cz.craft.wall-clock-law+1]`); the worker is always responsive
  to a newer request.
- **One live target.** The worker has exactly one current reference job, matching the
  one-live-target invariant. It never computes two references speculatively in parallel.
- **Publishes whole snapshots.** A reference orbit is published as one immutable buffer
  (plus its IntExp location, precision, and length), replacing the previous publication.
  The workgroup reads the latest published reference; it never waits on the worker.

### Selection (v1)

The reference point is the **deepest-known pixel of the current view** — the pixel with the
highest iteration count that has not escaped — falling back to the view center if nothing
has completed yet. This is nearly free (the workgroup already knows every pixel's iteration
count) and strictly better than an edge/corner pixel: a reference that escapes early is
useless, and the deepest point is by construction the hardest thing in view. The published
best practice (references at component nuclei, found by atom-domain → Newton-on-c) is a
**deferred upgrade** (see Deferrals); v1's deepest-point reference is correct, just sometimes
glitch-prone in difficult views.

**Selection is sticky: it happens at pivot time, not continuously.** The reference target is
chosen once per view, from the deepest *completed* point known at that moment, and is not
re-evaluated as the workgroup keeps iterating — otherwise the "deepest point" moves every
bout, the coalescing request channel sees a new target constantly, and the worker (which must
always drop to the newest request) starves: forever restarting, never publishing. The rule is
the one-live-target invariant applied to selection: **view changes drive reselection; progress
within a view never does.** Within a view, the only legitimate reasons to replace the selected
reference are that it escaped (proven bad) or that it has been published and is being extended
(same location, longer orbit — a continuation, not a new target).

### Precision policy

Precision is a per-reference decision, computed once when a job starts, never per pixel:
bits ≈ zoom pot + pixels-per-unit + safety margin. The formula lives in the reference
worker's contract. No user-visible setting (`r[cz.hoarding.no-compute-settings+1]`).

### Storage: floatexp; compute: gear ladder

Floatexp is the **storage** type for per-pixel deltas (Δz, Δc) and for the
published reference orbit. Those numbers are tiny by construction and carry
zoom depth in their exponents — f64's ~1e-308 floor would flush them to zero.
The *stored* orbit is floatexp: one storage type, no silent underflow.

The orbit is *computed* at full precision but each iterate is **stored rounded
to floatexp** (~16 bytes per iterate). Depth lives in the delta exponents.

### Compute gear ladder (`r[cz.depth.compute-gear+1]`)

Per-pixel recurrence uses the fastest gear whose range admits the current
delta. Selection is automatic from exact view spacing / delta magnitude — no
user-visible setting (`r[cz.hoarding.no-compute-settings+1]`). Legal promotions
only: **F64 → ScaledF64 → FloatExp**. A value at a gear's underflow floor is
handed off, never flushed to zero.

1. **F64** — hardware complex doubles for δz, δc, δd when magnitudes stay in
   ~[1e-300, 1e300]. Zero-orbit floor skips the `2Z·δz` term at bind time
   (Z=0 known): that *is* the naive/direct shortest path, same recurrence.
2. **ScaledF64** — unevaluated product δz = S·w, δc = S·d with S a FloatExp
   scale and w,d hardware f64. Inner step `w ← 2·Z·w + S·w² + d`. Re-scale
   rarely; dead terms decided at rescale time. The deep workhorse.
3. **FloatExp** — full software recurrence when the reference iterate itself
   cannot narrow safely, or after scaled-f64 cannot continue (deep needle).

Seats may promote mid-orbit independently. The HUD reports the aggregate
active gear (`F64` / `S-F64` / `FE` / `MIXED`) plus rolling IPS and PPS
(`r[cz.depth.gear-hud+2]`). f32 remains a typed extension point, deferred.

**Three HUD layers (stack / mode / ref / gear).** Stack is the view-global host type
(`f64` vs FloatExp) admitted by `CGenerator`. Mode reports which production kernel
runs: **`naive`** = `DirectKernel` (plain f64 iteration); **`pert`** =
`PerturbationKernel` (reference + delta recurrence). Ref is a running snapshot
(`wip` = no usable ref yet or glitch recovery awaiting a newer generation;
`complete` = steady state with reused ref). Gear is the per-seat delta ladder
under **`mode:pert` only**; naive mode reports `F64`. Per-seat `direct_only`
glitch recovery is not a view-global HUD mode.
The normative goal is view-global selection of the legal stack that maximizes
completed points per second (`r[cz.perf.pps-selected-kernel+1]`): default naive
when legal and fast; hard-bump to pert when naive cannot be honest; soft-probe
pert briefly when stuck and a covering reference exists. The transitional
`r[cz.perf.one-kernel-path+1]` milestone (perturbation-only shipping path) is
superseded by dual-kernel dispatch.

The worker retains a **constant-size high-precision state**: the current
iterate plus Brent cycle-detector cursors. That buys resumability — including
exact period/preperiod detection across 10ms bout boundaries — without keeping
an unbounded full-precision orbit history. The orbit can be extended later
(tenacious: deeper iteration on demand, `r[cz.tenacious.no-max-iter+1]`) without
recomputing from scratch. Discarding the high-precision tail would make every
extension a full recompute.

A 1M-iterate reference is ~16 MB stored. Reference *count* is therefore not the scarce
resource; orbit *length* (worker compute time) is.

### Cache

Published references are kept in a small byte-budgeted cache keyed by IntExp location,
inside the existing 1GB memory policy (`r[cz.system.memory-default-1gb+1]`):

- The reference serving the current view is pinned (the protect-current rule,
  `r[cz.system.tile-manager-protect-current-lookahead+1]`, applied to references).
- Eviction, when it ever happens, is farthest-from-current-target first.
- Reuse across magnifications is the common case when browsing locally: the previous
  reference often still covers the new view.

## The fallback chain

A view must never be left without a reference, and never stall waiting for one:

1. **Current view's published reference** — the normal path.
2. **Previous valid reference** — while the worker retargets after a pivot, the last
   published reference still serves any pixel it covers. This is the interim answer during
   the pivot window, replacing v0.0.9's remap-only continuity with remap + still-valid
   deltas.
3. **Zero orbit at full precision** — the floor. Always valid, always available, no
   reference needed at all; it just costs full-precision iteration for the affected pixels.
   Rare by construction; if benchmarks show it common, that is the signal to implement
   reference replacement (Deferrals), not to add machinery preemptively.

Each rung is rarer and slower than the one above; none is a user-visible stall
(`r[cz.seamless.reference-background+1]` — no progress bar, no blocked activity).

## Glitch handling (v1)

**Pauldelbrot glitch** (`src/perturb.rs`): when |Z_n + Δz_n| becomes small relative
to |Z_n|, the approximation has failed for that pixel. That seat is *unfinished*:
rebind to fallback rung 3 (zero orbit) and reset — do not publish a guessed answer
(`r[cz.depth.glitch-is-unfinished+1]`).

**Missing reference iterate is not a glitch.** If the published orbit has no
`Z_n` yet (`orbit.get(n) == None`), that is unfinished / short coverage — the
library core returns `Unfinished`, never `Glitch`. The seat soft-continues on
the zero-orbit floor with `δz ← z` (reconstructed objective state) and
`δc ← c` (`r[cz.depth.perturb-never-wrong+1]`). An artificial reference length
wall (publishing incomplete orbits at `MAX_BOUT`) was a design bug: it made
exhaustion look like a shared "iteration wall" / glitch-blob epidemic.

**Reference completion** is period-found or escaped only — no length target
(`r[cz.depth.reference-until-done+1]`, `r[cz.tenacious.no-max-iter+1]`). Until
then, seats use the zero-orbit floor.

### CGenerator admission (`r[cz.depth.c-generator-fails-closed+1]`)

Per frame, once, in O(1): compute exact `IntExp` origin and pixel pitch; probe only
the near and far ends of each axis for distinguishability in the target type `T`
(`Mandelbrotable`, `From<IntExp>`). On success store `(origin, space)` as `T`;
`get_c(seat, row)` is then pure `T` multiply-add with no per-seat IntExp work.
Stack order: f64 absolute → f64 relative → FloatExp absolute → FloatExp relative.
Relative admission subtracts in exact `IntExp` before narrowing; anchor is
`published.c` when a reference exists, else view center. On reference generation
change, rebuild the generator relative to the new reference so seats initialize
from the matching grid.

**Values gated:** absolute seat `c` on naive paths; `delta_c` on perturbation
paths (the numbers actually iterated). **Render margin (2026-08-12):** admission
must keep ~**10 bits** of headroom beyond neighbor distinguishability on both
paths — distinguish-only false-admits shallow types and yields rectangular
transition blockiness. Interview:
`docs/assistant/interviews/2026-08-12-precision-wall-gear-switching.md`;
paraphrase: `docs/assistant/paraphrase-authoritative/c-generator-admit-margin.md`.

**Perturbation admit order:** nearest kept ref to the screen → δc stencil →
margin admit → smallest type. Closer ref may unlock a cheaper type; must be
correct even if uncommon. A pure “no close ref → wall” case is deprioritized as
extremely unlikely; glitch-without-safe-ref is the stuck case that matters
(local seat pause; never discard refs for glitch — see reference-reuse).

## What the workgroup changes look like

- The proven scheduler calls a narrow `SeatKernel` interface. **`DirectKernel`**
  is the production naive path; **`PerturbationKernel`** is production when depth
  or honesty requires perturbation (`r[cz.perf.pps-selected-kernel+1]`). Both reuse
  the same slots, queues, attention, delivery, and wall-clock policy.
- `Point` gains delta-orbit state (Δz as floatexp pair, reference generation id); c remains
  the IntExp-derived truth. Points iterate as deltas against the currently published
  reference; the per-pixel path stays f64-speed or faster.
- Escape and loop checks run on z = Z_n + Δz_n. Interior detection and the
  atom-domain → Newton → multiplier period pipeline are unaffected in *structure*; their
  inputs come from the reconstructed z. Period verification on the reference's own c still
  runs at full precision in the worker.
- Bouts, queues, publish cadence, remap, and the four guarantees are unchanged. Delta
  iteration is *cheaper* than the current direct f64 path; nothing about scheduling changes.

## Series approximation (the actual skip)

From a reference orbit, build a polynomial in `delta_c` that skips the first N
iterations of every pixel, with an error bound deciding safe N. Simple series
only; biseries / nucleus seek / multi-ref remain named deferrals
(`r[cz.depth.series-approximation+1]`).

### Product intent (developer 2026-08-11)

- **Why:** deep zoom. Large safe skips are the win. Shallow/home must barely
  notice SA when the skip is useless.
- **Always on** when a covering reference is published — no “enable SA”
  heuristic branch. If the probe is done well, a no-op skip costs almost
  nothing; branching for worth-it is pointless.
- **Seat role:** SA is **point initialization**, not iterate-bout work. Skip
  discovery must be so cheap it is free: binary search through the orbit
  (O(log N) evaluations), not a linear scan of every index. Easy cases ≈ a
  single access per point. It must not steal budget from workshift iterate
  bouts — that framing does not apply.
- **Reference role:** advance **one series step per reference iterate**, rolled
  into the same reference loop (same worker, same cadence). Not a separate
  coeff-building process. Extra math is allowed only to the degree it is
  necessary; no extra big-O beyond the per-iterate series recurrence; airtight
  performance mindfulness from the start (hand-coded v0.0.9 habits: tight
  layout, no slow ops in the hot path, no “too much in the loop”).
- **Gear:** mid-orbit gear promotion does not restart seats or re-run series
  init. Restart stays on reference generation / glitch / unbound paths.

### Prior live attempt (2026-08-07/08) — not the target

Wired briefly (`PublishedReference.series` + `apply_series_skip`), then deferred
for membership pins (`pin_exterior_not_marked_in_at_zoom_52`,
`pin_not_blocky_delta_c_at_zoom_49`). Audit: even before that yank, the sketch
failed the performance bar — linear `safe_skip` with full re-evaluate per n,
heap `Vec<Vec<_>>` coeffs, FloatExp-heavy probe from the f64 path, skip cost
unbounded relative to seat init. That sketch was replaced 2026-08-11 by the
fused / O(log N) production path in `src/series.rs`.

### Acceptance for re-enable

**Landed 2026-08-11:** membership pins green with SA on; logarithmic probe +
shallow no-op + deep material skip pins; live `apply_series_skip` on seat init;
full suite / benches / visual as the gate for this chunk.

## Acceptance (this push — all gates together)

Nothing is complete until every gate passes on one final tree:

- Home-view perturbation parity-or-better vs DirectKernel f64 baseline.
- Scaled-f64 materially faster than all-FloatExp on representative deep workloads.
- Series materially reduces prefix iterations.
- Live adjacent-pixel distinction at capacity ≥2^3600000.
- Full tests, tracey links, visual captures (no banding/blobs), HUD gear+IPS+PPS.
- Zero regressions vs accepted benchmarks and craftsmanship invariants.

## Deferrals (named, not smuggled)

- **Nucleus-seeking references.** Atom-domain → Newton on the critical orbit places the
  reference at a component nucleus: fewer glitches, enables period-skipping biseries. We now
  own most of the machinery (the period pipeline's Newton is the z-variable cousin).
  Trigger: measured glitch rate on difficult deep views.
- **Multiple references for glitchy views.** Published practice for embedded-Julia-set
  regions. Deliberately deferred — multi-reference is exactly the tile-era fragmentation
  temptation; it must extend the one-truth-package discipline, not replace it (virtues §3).
- **Reference replacement on glitch** (reseed inside the glitch region, redo only those
  pixels). The standard KF loop; v1's fallback chain is simpler and measurable first.
- **GPU deltas.** Deltas are f32/floatexp-shaped and embarrassingly parallel — the eventual
  GPU port (`design-target.md`) moves the per-pixel delta loop, not the reference worker.
  The high-precision path stays on CPU forever (`numerics-and-precision.md`).

## Oracles and testing

The governing rule (learned from the period work): the pipeline under test receives only
plain inputs — every oracle lives in test code, and production never branches on oracle
knowledge. Three layers:

- **Differential against a precision-doubling naive oracle.** The source of truth at depth
  is naive iteration in `rug::Float`. **First use enough bits to represent the exact dyadic
  input**, then double precision until the answer is bit-identical across a doubling. The
  ordering matters: implementation testing found that two fixed low precisions can agree on
  the same wrong answer because both erased the same deep coordinate bit. After the input is
  exact, survival across a doubling is a sound practical heuristic. Compared quantities, in
  increasing strictness: escape/in/out decision, escape time exactly, and (via the derivative
  pipeline at the oracle's precision) period. Slow is acceptable — this is an oracle, run at
  proptest scale (tens of cases), not a benchmark.
- **Inconclusive is a *test-harness* outcome — the app itself has no effort cap.** The app
  never gives up on a point: never-completing points ride the queues at bounded bout cost
  (v0.0.9 rotation already does this), and certifiable boundary points (parabolic,
  Misiurewicz — exact rational/zero-epsilon checks, see the taxonomy in
  `../mandelbrot-library/period-and-interiority.md`) get a third `Boundary` completion state,
  colored separately. Only the *test* caps precision doublings and iterations, because a test
  physically must terminate; a capped oracle returns `Inconclusive` and the case is skipped
  (`prop_assume!`). That is not the app conceding anything: the perturbed side is never
  *allowed* to disagree with a concluded oracle, and in the app the uncapped answer is always
  eventually one of escapes / repeats / certified boundary.
- **The invariant the tests actually guard:** the perturbed path may answer *unfinished*
  (glitch, unknown period 0) but may never answer *wrong*. Every differential assertion is
  "equal to the oracle, or honestly incomplete" — which also guards the glitch detector
  itself (a glitch reported where the oracle says the delta math was fine is a false alarm;
  a wrong answer with no glitch flag is a silent-corruption bug).

The same doubling oracle validates the reference worker directly (stored floatexp orbit vs.
full-precision truth at each iterate) and extends the existing shape oracles (cardioid /
bulb / child-bulb / neck, which only reach f64 depth) into arbitrary-depth regions where no
closed form exists. Committed regression seeds capture every interesting case the generator
finds, as with the views and craftsmanship suites.

## Open questions

- Exact precision-margin constant in bits = zoom pot + PPU + margin (size from benchmarks).
- Glitch-criterion constant tuning on hard views (published defaults exist; verify on our
  fitness workloads).
- Whether reference *extension* (longer orbit, same location, still incomplete interior)
  should publish intermediate whole snapshots before period/escape — leaning no for v1
  (zero-orbit floor until done); revisit if zero-orbit is too common at depth.
- Selection upgrades: prefer proven periodic nuclei; when glitches cluster, seek toward
  non-central / edge / Misiurewicz candidates. v1 sticky deepest-completed + coverage
  gate stands until measured.

## Traceability

Satisfies: `r[cz.seamless.perturbation-always-on+1]`, `r[cz.seamless.reference-background+1]`,
`r[cz.deep.min-zoom-pot-capacity+1]`, `r[cz.deep.snappy-at-depth+1]`,
`r[cz.tenacious.no-max-iter+1]`, `r[cz.hoarding.no-compute-settings+1]`,
`r[cz.system.memory-default-1gb+1]`, `r[cz.system.tile-manager-protect-current-lookahead+1]`,
`r[cz.depth.compute-gear+1]`, `r[cz.depth.gear-hud+2]`, `r[cz.depth.series-approximation+1]`.
Constrains and is constrained by: `design-target.md`, `workgroup-virtues.md` §2–§5.
Math research: private sister repo `Critical-Zoomer-Math-Library` (not in this tree).
