# Salvage from Trash (LOWER TRUST — unvetted)

**Trust tier 2.** Rescued 2026-08-06 from `Trash/` during the docs reorganization. Unlike
`collected-wisdom.md` (tier 1 — checked against the v0.0.9 golden study and often
developer-confirmed), nothing here has been vetted against the restored code or the virtues
doc. These come from pre-revert writings of various eras; some were superseded *by the same
documents' later sections*. Treat every entry as a lead, not a rule. Promote to
collected-wisdom only after checking against the v0.0.9 code and, where behavioral, the
developer. Known internal contradictions are collected at the bottom — read them before
trusting anything else.

Classification per entry: **(a)** plausibly applies to v0.0.9 now · **(b)** forward-looking
GPU/depth port guidance · **(c)** questionable or contradicted.

## Product / UX spec

- **Zoom quanta and bars.** One scroll bump = one POT step = exactly 2×. A fast gesture (~10
  bumps) must sustain all bumps in ~300ms with zero missed; "spinny" bursts far faster than
  baseline must also not miss. (a)
- **Pointer-anchored zoom, half-pixel correction.** The point under the cursor at bump time
  must map to the same pixel after the bump, with an explicit cell-center half-pixel
  correction per step. (a)
- **Retained drag anchor.** On drag start, store both objective and screenspace anchors; the
  user may zoom out, pan, zoom back in and must return to the stored objective anchor. (a)
- **Coordinate conventions.** Internal frame: upper-left origin, x right, y down. User-facing:
  screen-center origin. Convert only at the UI boundary; never navigate via a low-precision
  float frame. (a)
- **Infinite-precision viewport.** Location held at infinite precision, no silent rounding;
  zoom in binary-exponent steps; viewport position math always < 1ms. (a/b)
- **Coordinate bar UI.** Bottom-left themed bar; an earlier upper-left black box covered the
  fps display and was confusing. (a)
- **Seamless ship criteria.** No max-iteration setting, no perturbation toggle, no GPU toggle;
  input never waits on compute; settings recolor from the hoard without recompute; memory
  budget a constant at the requirements floor, no slider UI. (a)
- **Remap is identity, not preview.** "The app is not allowed to show the user a flat empty
  pane when it has the opportunity to remap the previous work." Remapped old work is the
  product's core real-time-viewer value, not a loading screen. (a)
- **HUD is not evidence.** Only screenshots prove display state; the HUD can be a grey panel
  while the assistant claims success. (a — verification discipline)
- **Incomplete-pixel cosmetics (unimplemented spec).** Unfinished pixels as 64×64
  `#FF00FF`/`#000000` checkerboard; solid purple `(128,0,128)` before first full paint. (b —
  v0.0.9 instead defaults unfinished seats to outside-looking `Dummy`; reconcile before any
  port)
- **Ship-gate manual script.** Home settle looks tenacious; cardioid browse shows escape time +
  in-filaments with no whole-screen black flood before the edge walk; one deep seahorse-style
  location stays responsive with low-res interim OK. (a)

## Scheduler / boundary tracing

- **Phase preference order with feeding relations.** fill-out > edge > scredge > period-edge >
  flood-in > in. Fill-out and edge are fed by scredge; period edges by edge + scredge; flood-in
  instantly bucket-fills equal-period in-areas, fed by all edges. (a)
- **Edge trace, not flood.** Correct tracing enqueues only the ~8 contour-continuing seats
  (push_front, stays local); enqueueing all axis neighbors degenerates into a thick
  near-boundary flood that dominates scheduling and looks like an "extremely slow edge phase."
  (a)
- **Never preempt a started edge seat.** Once tracing starts a seat, finish it before picking
  another; churn on started points is an antipattern. (a)
- **Petal fill.** When edges enclose a petal with one shared orbit period, fill the petal with
  that period instead of iterating interior pixels (still compute small-time/min-magnitude per
  point). In-fill under unknown period may spread any *consistent* period value, uniformly, to
  avoid false in-filaments. (b)
- **Antenna lattice artifact.** Home view Im=0 lands exactly on seat y=256 before the half-step
  offset — "broken real axis" reports are this alignment, not math. (a — diagnostic)

## Period determination theory (the newer, derivative-based one)

- **Twin test.** Candidate period from single-candidate loop detection (power-of-two
  checkpoint, checked every iteration). Confirmation is off-to-side: iterate current z and
  saved z N=20 times each, requiring spatial equality within epsilon AND derivative similarity
  throughout ("true twins behave similarly"). Cheap because the first comparison almost always
  fails. Derivative = orbit multiplier along the iterate. (a/b — this is the theory meant to
  replace v0.0.9's `determine_period`; not yet fully working anywhere)
- **Pitfalls.** A first epsilon-hit against a fixed checkpoint can be a multiple of the true
  period (approach lag is not monotonic) — reduce by ascending |f^d(z)−z|. Do NOT gate that
  reduce on |λ|<1 at the off-cycle z (false-rejects near-parabolic points). Interior-only test
  grids miss boundary failures — keep near-edge samples. (b)
- **Period seam paint rule.** Out-filament rendering must tolerate period-unknown next to
  period-known Inside without a visible seam; bands inside 'in' areas are the signature of
  wrong periods in out-filament highlighting. (a)
- **Small_time zero filter scope.** The small_time==0 filter applies to small-time-edge
  detection only (map 0 → none), never to paint; stray lines were zero-vs-nonzero neighbor
  edges. (a)

## Perturbation / numerics (GPU port)

- **Anti-cheating rules.** No naive fallback (except the zero orbit, which is legitimate); no
  max iteration count; reference orbits must be inside the set; never-finishing seats are
  banned (pause a slow point, never give up); perturbation correctness rests on period
  detection. Must demonstrate real precision gain past z≈17. (b)
- **Zero orbit is not a special case.** Write code so the zero case works naturally — no
  if-forests; early points fall back to the trivial 0-orbit; keep the last orbit. (b)
- **Per-seat orbit binding.** Each seat references its own orbit; session-wide reference
  management was explicitly rejected. (b)
- **No perturbation pause.** References are IntExp-defined precisely so they can be reused
  across magnifications; there must never be a visible stall while a reference computes. Biggest
  speed win: keep everything small and on stack. (b)
- **Cursor-zoom lookahead.** Continuously compute references near the mouse cursor; keep many
  references in glitchy areas within a memory budget; idle time does thorough nucleus seeks
  (short interactive cap + background thorough seek). (b)
- **Numeric type roadmap.** IntExp is the permanent skeleton (never migrate away);
  StackedIntExp ([i64;N]+i32, i128 carry) is additive-only and forbidden in screenspace without
  explicit approval; FloatExp variants: f64+i32 and rug::Float+i32 for reference builds; GPU is
  32-bit only (f32, f32+i32, i32+i32); Pauldelbrot glitch threshold fixed at 1e-4, glitch →
  rebind seat to zero orbit; series approximation ships in the same phase as orbit-only
  perturbation. (b)
- **Series approximation as exact skip.** Coefficients built only from the reference orbit,
  precomputed once, stored beside it; each pixel becomes a search for the last step where the
  z² term is still absorbed, then a short full-perturbation tail. Not a fuzzy-bounds skip. (b)
- **Derivative direction on Answers.** Edge detection moves out of the worker via
  derivative-direction storage; escape-time change direction via derivative; a GPU Answer
  variant must fit in 32 bits. (b)
- **f64-depth bridge.** Perturbation must use a screenspace factor to keep zooming past the
  f64 limit (~2^-1024). (b)

## Display / pipeline rules

- **Highlighter marks are set invariants.** Marks must not depend on the animatable bailout
  radius; highlighter runs before escaper; colorer only paints marks, never detects them. (b)
- **Filament definitions.** In-filament: interior pixels from neighbor escape-time slope *sign
  changes (peaks)*, biased thin. Out-filament: outside pixels from *period changes* among
  interior neighbors, biased outward (toward higher period). (b)
- **Fool's period.** Points show temporary periods that settle arbitrarily slowly; a black
  region shares one period but edge-near points take longer to reach the node. Node
  highlighting marks minibrots too small to see and the most-stable settled points of bulbs.
  (b)
- **GPU pipeline order.** Sampler → escape (small max-iter bailout) → edge → shade, all at
  framerate; CPU computes only IntExp seat deltas in O(1). GPU sampling is "practically free" —
  low fps is inexcusable. Settings window must not grey the main window. (b)
- **Escape z retained.** Work includes the escape z-value so other bailout radii can be
  recomputed cheaply (known bug accepted at −2+0i). (a/b)
- **WIP escape-location ring bound.** An escaped point's escape location is known within the
  ring between r=2 and r=6 (2²=4+2). (b — calibrated partial knowledge)
- **Main antenna exception.** Near (−2,0), points escape at radius "as close to 2 as screen
  pixels are," breaking the escape-is-fast assumption; an escaper iteration limit with
  imperfect metrics is sanctioned there. (b — see tensions: effort-limit ban)

## Architecture / policy survivals

- **Three-group shape.** Headgroup (framerate-locked, owns settings and all app IO, owns the
  GPU answer hoard), workgroup (completes work, owns CPU hoard for continuity/dedup),
  shadergroup (sampling → shading, sampling first, same path every frame). v0.0.9 matches this
  shape. (a)
- **Policy taxonomy.** Seamless (no engine-internal toggles), Deep (representable ≥ 2^3.6M,
  snappy at depth), Tenacious (never abandon started in-viewport work, no max-iter knob),
  Hoarding (one answer per point, recolor-from-hoard, never recompute membership for settings),
  Calibrated, Fast. (a)
- **Backpressure is a bug.** "Channel backpressures are a sign of incorrect code, not
  transient stress." (a — see tensions: small channels)
- **Answer-only hoards.** No color hoard anywhere; colors exist only at shade time. (b)
- **Static data, dynamic sampling.** Work is never transformed for display; sampling maps
  static work to the dynamic viewport. (b)
- **No global clear-all.** There is literally no case in which all work should be cleared. (a)
- **Homothety sharing.** Equal-zoom views share one homothety; coordinate convention survives
  the tile era. (b)
- **Thin window for testability.** Egui isn't testable; the window must be thin/dumb, emitting
  commands to a sampler that outputs RGB frames plus metadata. (b)

## Process lessons

- **No surprises.** Refuse to write behaviorally-ambiguous code; if interpretations are
  behaviorally distinct, ask; if not, don't ask. (a)
- **Test bar.** ≥3 meaningfully different tests per requirement; prefer small properties over
  input/output tests ("if it looks multi-part, it's not a good property"); proptest weighting
  to keep tests fast. (a)
- **Harness first.** The screenshot/input harness is the regression net — port it before any
  refactor. (a)
- **One-step cutover.** Flip the pipeline graph in one step when ready; never run dual
  publishers; any bridge adapter must be explicitly temporary. (a)
- **Voice rule.** Name things as the developer would (authoritative spec, v0.0.9 code, README
  are the voice references). (a)
- **Answers add detail.** Developer answers added binding detail beyond assistant defaults in
  ~2/3 of scored cases — record them verbatim. (a)
- **Old spec-citation rule.** "Rigorous e2e or an explicit doc complaint — you may not invent
  holes": if the spec is wrong, say so; otherwise fix the code. (a)

## Tensions and open contradictions (unresolved — developer decides)

- **Timeslicing vs sequential phases.** The virtues doc praises the five-queue timeslicing
  rotation; a developer quote says "Originally it would split its time but I found that
  inefficient," with a strict sequential phase preference documented. Possibly era-dependent
  (the quote may postdate or target a different scheduler), possibly a real refinement.
- **Small channels vs banding.** The virtues doc treats small channels as a promise; the tile
  era recorded regularly-shaped black bands caused by worker→collector channel backpressure,
  declared "not acceptable as a product state" — requeue-on-blocked was mitigation only.
  Reading that respects both: small channels are right *with* drain-to-newest on
  command/attention paths; completion paths must not visibly backpressure.
- **Window default.** Docs said 800×480; v0.0.9 code has 854×480 (`constants.rs`). Also
  flagged in tracey foundations rules.
- **Effort-limit ban vs antenna exception.** One spec section bans effort limits outright;
  another sanctions an escaper iteration limit on the main antenna. Resolve explicitly before
  the GPU port.
- **Guess-biased sampling vs one-answer honesty.** An old workgroup design had the publisher
  guess "biased towards the previous value of that location," which sits uneasily with the
  one-answer-per-view / honest-incomplete law.
- **GPU IPS figure.** The old standards text says 6B IPS in prose and 30B in the rule title.
- **Remap associativity.** Remap should ideally satisfy 4×-zoom == two 2×-zooms; flagged as a
  layout-shift time sink. Moot at v0.0.9 (one remap per retarget); returns if remap chains ever
  appear.
- **Period-phase placement.** One doc says "no separate period detection phase"; the later
  locked quote requires a dedicated period phase after boundary+out-fill. The later quote wins.
