# Period determination and interiority

The design problem: v0.0.9 decides "inside" by loop detection (a doubling checkpoint plus an
epsilon box), which is correct for membership but yields only a *candidate* period; the
`determine_period` refinement stage (timewarp + tighter-epsilon re-search) was never that good
and is §12 cleanup material. The published literature has a complete, non-arbitrary answer.
This is the centerpiece of the library: it is what the salvaged "twin test" theory was reaching
for, and it is what the §13 design ideal (tolerances that fall out of correct numbers) points at.

## The pipeline (Heiland-Allen, "Practical interior distance rendering", 2014)

Three stages, each with a *correctness condition*, not a tuning constant:

### 1. Candidate periods from atom domains (partials)

Iterate z from 0 as usual. Whenever |z_n| reaches a **new running minimum**, record n as a
candidate period. These n are the "partials"; they correspond to atom domains (Munafo 1996;
Peitgen & Richter 1986), regions that completely enclose the hyperbolic component of the same
period and are usually much larger, so the candidate is robust.

- This replaces the doubling checkpoint as the *candidate generator*. The doubling checkpoint
  asks "did it loop?" at exponentially spaced snapshots; the partials ask "when did the orbit
  get closest to the critical point?" — which is exactly when a nearby attractor of that period
  dominates.
- Cost: free. The running minimum is what v0.0.9 already collects as `smallness_squared` /
  `small_time` (`update_point_results` — see `cz.craft.cached-products+1`). The candidate list
  is every record-minimum step, tried ascending — not just the last one (see the corrections
  under Leverage below).

### 2. Solve for the attractor with Newton's method

Given candidate period p, don't wait for the orbit to converge to within epsilon — *solve*
F^p(w, c) = w directly, iterating w ← w − (F^p(w,c) − w)/(∂/∂z F^p(w,c) − 1), starting from
w₀ = F^p(0, c). Converges quadratically in a handful of steps (Mandelbook "Attractor" gives
worked examples: c = −1.1 + 0.1i, p = 2 pins 17 digits in 4 iterations).

Per Newton step, iterate p steps carrying the z-derivative alongside:
`dz ← 2·z·dz` (starting dz=1), `z ← z²+c`.

### 3. Confirm interiority by the derivative test

Compute b = ∂/∂z F^p(w, c) (the product of 2z around the cycle — the orbit multiplier). The
point is interior to the component of period p **iff |b| ≤ 1**. No epsilon anywhere: the
multiplier is a property of the actual cycle, and the unit circle is the mathematically exact
boundary between attracting and repelling.

If |b| > 1, the candidate was wrong (or the point is exterior): continue to the next partial.

Reference C99 (Mandelbook, "Interior Coordinates"): iterate; on each new |z| minimum at step n,
run `m_attractor(z, n, c)`; carry dw around the n-cycle; `if (cabs(dw) <= 1) return dw;`.

## Why it gets harder with zoom (and why that's OK)

- Near island boundaries the true period grows and the orbit takes long to approach the cycle —
  which is exactly where the epsilon approach fails (approach lag is not monotonic; a fixed
  epsilon either false-fires on a multiple of the period or never fires). The atom-domain +
  Newton approach doesn't care about the approach rate: candidates come from minima, and Newton
  jumps to the cycle.
- The cost shifts to precision: F^p evaluation and Newton need enough bits to represent the
  cycle, which at depth means floatexp or perturbed evaluation (see `numerics-and-precision.md`
  and `perturbation.md` — mathr's perturbation.pdf extends the delta technique to Newton's
  method itself, so the whole pipeline can run perturbed against a periodic reference).
- Known pitfalls recorded in our salvage docs remain valid: don't gate the interiority test on
  |λ|<1 evaluated at an *off-cycle* z (false-rejects near-parabolic points — the derivative must
  be evaluated at the Newton-converged attractor), and keep near-edge test samples.

## Leverage for Critical-Zoomer (interpretation — ours, not the sources')

- **Implemented in `workshift.rs`** as `period_partials` + `verified_period` /
  `verified_period_from`: on completion, replay the critical orbit collecting record-minimum
  steps, try candidates ascending with the tail iterate as Newton start, reduce a converged
  root to its minimal period, accept exactly when |b| ≤ 1; if nothing verifies, publish period
  0 ("unknown"), which never lights a period edge. The old `determine_period`
  timewarp/tighter-epsilon stage is gone.
- **Two corrections learned by oracle testing (not in the sources as we read them):**
  1. *Ascending partials, not the last record.* An interior orbit sets new |z| minima until it
     converges, so `small_time` is the convergence time; Newton then happily verifies a
     multiple of the true period (an attracting p-cycle is also an attracting kp-cycle). Every
     record minimum must be tried in increasing order, first verification wins.
  2. *Newton start and divisor roots.* The published `F^p(0,c)` start is far from the attractor
     exactly at necks, where the multiplier sits on the unit circle and Newton converges only
     linearly (budget ~128 steps); it can also land on a divisor of p (fixed points satisfy
     F^p(w)=w too), so the converged root must be reduced to its minimal period before the
     multiplier test. Starting from the orbit's tail iterate fixes the convergence pathology.
- **Oracles that catch both failure classes:** main cardioid → period 1 (multiplier
  parameterization), period-2 bulb |c+1| < 1/4 → period 2, and the cardioid/bulb neck at
  c = −0.75: −0.75 ± 2^-k must classify as 1/2 for k up to 40. Neck points are where
  epsilon-based and single-candidate pipelines break first, and they are testable to f64 depth.
- **Publication is part of correctness.** The v0.0.9 scredge path publishes an unfinished
  repeating answer to keep the newest frame visible. Its loop-checkpoint gap is not a period:
  publishing that value created cloudy period noise inside the period-2 bulb and increasingly
  speckled child bulbs even though the final verifier was correct. Provisional publications must
  use period 0 (unknown); only completion may publish a verified nonzero period. A scheduling-path
  regression test enforces this separately from the mathematical verifier tests.
- **Child-component noise oracle.** Sample small two-dimensional neighborhoods around known
  superattracting period-3 and conjugate period-4 centers and require period constancy. The centers
  and expected periods are test-only independent oracles; production contains no cardioid/bulb
  membership branches.
- **Out-filament detection** (period changes among interior neighbors, per collected-wisdom)
  needs trustworthy periods — this pipeline is the prerequisite.
- **Node/minibrot highlighting** ("most-stable settled points") = |b| near 0; the multiplier
  is computed anyway.
- **Interior distance estimation** (see `rendering-and-filaments.md`) falls out of the same
  derivative pass for free-ish — potential future shade layer.
- Note for the workgroup: the Newton solve is a bounded extra cost per *repeating* point
  (a few p-length passes), fitting the small-interruptible-bouts discipline; it replaces, not
  adds to, the 100k-step timewarp.

## Completion taxonomy: does every point finish?

The app's founding assumption — every point either escapes or repeats, so every point
eventually completes — needs precise statement. The true taxonomy of the critical orbit:

- **Exterior (c ∉ M):** escapes. Finite escape time; precision-doubling always resolves it.
- **Hyperbolic interior:** converges to an attracting cycle → repeats. Every convergent orbit
  is Cauchy, so an epsilon loop check fires eventually even when it never repeats exactly
  (this also covers parabolic and Feigenbaum-type convergence).
- **Boundary (c ∈ ∂M):** the honest exception list. Parabolic parameters converge like 1/n —
  never exactly repeat, and epsilon detection takes ~1/ε iterations (astronomical at depth).
  Note that parabolic parameters include dyadic values (c = 0.25, −0.75) which *can be exact
  pixel centers*. Siegel/Cremer parameters (irrational indifferent multipliers) are worse:
  quasiperiodic, never repeat, never converge to a cycle; proven to exist in positive-measure
  sets on component boundaries (Avila–Lyubich). They are irrational, so pixel centers land
  arbitrarily close but never exactly on them.

And the load-bearing caveat: **"every interior point is in a hyperbolic component" is the
density of hyperbolicity conjecture — open as of 2026** (implied by MLC; proved for the real
axis by Lyubich and independently Graczyk–Świątek in the 1990s). Hypothetical non-hyperbolic
"queer" interior components have never been observed, and the conjecture is universally
believed; building the app on it is sound. But docs and tests must cite it as a conjecture,
and the practical completion boundary is *cost*, not chaos: near-∂M iteration counts grow
without bound, so "unfinished while visible, keep working" (tenacity) is a mathematical
necessity, not just good UX. The Mandelbrot dynamics relevant here are never chaotic in the
sense of unbounded disorder — bounded critical orbits are structured (attracting, parabolic,
or quasiperiodic) — which is exactly why completion is achievable at all.

## Can boundary points be detected explicitly?

Three-way split, and the split is sharp:

- **Never, in general.** Certifying an arbitrary boundary point as boundary is undecidable:
  M is not computable (BSS), its complement is only computably enumerable, and Hertling showed
  computability of M is equivalent to the density-of-hyperbolicity conjecture. A generic
  boundary point is forever indistinguishable from a very slow interior point.
- **Exactly, for the algebraic cases — which are the only ones pixel centers can hit.** Pixel
  centers are dyadic rationals, and dyadic boundary parameters are low-degree algebraic:
  - *Parabolic* (necks/cusps: 0.25, −0.75, …): certify with exact rational arithmetic —
    multiplier = root of unity and F^p(w,c)=w checked with no epsilon (0.25: w=0.5,
    multiplier exactly 1). Tolerance-free, per the §13 design ideal.
  - *Misiurewicz* (preperiodic: −2, i, …): the orbit repeats exactly after the preperiod; for
    small dyadic c this repetition is exact in machine arithmetic — a zero-epsilon loop check
    certifies it.
  These warrant a third completion state (`Boundary`, colored separately) alongside
  escapes/repeats.
- **Approximately, as regions.** "The boundary passes through this pixel's footprint" is
  achievable via interior/exterior distance estimation — different and weaker than "this
  point is boundary", but it is the renderable version, and it needs no undecidable step.

Everything else on the boundary (Siegel/Cremer/Feigenbaum/generic) is irrational: pixel
centers approach but never land. Those pixels remain honestly unfinished — ridden by the
queues at bounded bout cost, exactly as v0.0.9's rotation already handles slow points.

(Developer-confirmed design intent: this certifiability result is the reason Critical-Zoomer
uses dyadic IntExp coordinates from the start — pixel centers stay exact dyadic rationals at
every zoom, so the only boundary points that can ever be requested are the algebraically
certifiable ones.)

## Sources

- Heiland-Allen, "Practical interior distance rendering" (2014):
  https://mathr.co.uk/blog/2014-11-02_practical_interior_distance_rendering.html
- Heiland-Allen, "Mandelbook" (draft 2017), chapters Atom Domains / Interior Coordinates /
  Interior Distance / Nucleus / Attractor:
  https://mathr.co.uk/mandelbrot/book-draft-2017-11-10.pdf
- Heiland-Allen, "Interior coordinates in the Mandelbrot set" (2013):
  https://mathr.co.uk/blog/2013-04-01_interior_coordinates_in_the_mandelbrot_set.html
- Munafo, "Atom Domain", muency: https://www.mrob.com/pub/muency/atomdomain.html
- Munafo, "Period", muency (incl. Jordan-curve method alternative):
  https://www.mrob.com/pub/muency/period.html
- Density of hyperbolicity / MLC status (open conjectures; real-axis proofs by Lyubich and
  Graczyk–Świątek): https://en.wikipedia.org/wiki/Mandelbrot_set
- Avila–Lyubich, "The positive-measure Julia sets / Siegel parameters" work (parameters with
  Siegel disks have positive measure on component boundaries):
  https://arxiv.org/abs/0805.1658 (survey context for bifurcation loci and hyperbolicity)
