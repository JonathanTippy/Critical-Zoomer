# Perturbation

The depth technique: past the f64 limit, iterating every pixel at full precision is
unaffordable. Perturbation computes **one** high-precision reference orbit and then iterates
only the *difference* for each nearby pixel, at low precision.

## Core recurrence (Martin 2013; Khashin 2016; Heiland-Allen)

Reference: iterate Z at high precision, Z' = Z² + C. Pixel: c = C + Δc, z = Z + Δz. Algebra
gives the delta iteration, in low precision:

    Δz ← 2·Z·Δz + Δz² + Δc

Escape test on Z+Δz (with the reference's Z as the big part). The key numeric fact: subtracting
two nearby high-precision numbers leaves a difference with little meaningful precision, so Δ
lives comfortably in f64/float-exp while Z needs the full precision of the depth.

Derivatives (for distance estimation, interiority, and the period pipeline in
`period-and-interiority.md`) perturb the same way — Heiland-Allen's perturbation.pdf spells out
the delta recurrences for ∂/∂z, ∂/∂c, and second derivatives, and shows the technique applied
to Newton's method (so attractor solves can also run perturbed, provided the reference period
divides the target period — "sensible to choose a periodic point as a reference").

## Glitches and how the field handles them

Perturbation is wrong in "sometimes subtle ways" when the reference orbit stops being
representative (Heiland-Allen, "Perturbation glitches", 2014):

- **Detection**: Pauldelbrot's heuristic — the pixel is suspect when |Z+Δz|² became small
  relative to |Δz|² (threshold fixed at 1e-4 in our salvaged rules) i.e. the delta dominated
  the sum at some iteration. mathr's mightymodel instead tracks an *error estimate* per pixel.
- **Correction**: re-reference the glitched pixels against a new reference and recompute them
  (our salvaged rule: glitch → rebind seat to the zero orbit is the no-reference special case).
- **Reference choice matters more than threshold choice**: mathr's glitch study shows the
  central minibrot is often a *bad* reference; a non-central minibrot near the glitch cluster
  is better, and the limit of "nearby higher-period non-central minibrots" is a pre-periodic
  (Misiurewicz) point at the tips/spirals of the embedded Julia sets that glitch most. No
  single reference fixes a whole difficult view — KF-class renderers use multiple references.

## Leverage for Critical-Zoomer (interpretation — ours)

- Maps onto the salvaged anti-cheating rules: references inside the set, per-seat orbit
  binding, zero orbit as the legitimate trivial reference, never abandon a seat (pause it).
- The natural architecture fit: a reference is IntExp-defined and computed once per view (or
  per dominating island); seats bind to it; glitch → rebind. This is the depth design gap in
  `issue-stack.md` and the suspended D-REF decisions in `unit-design/decisions.md`.
- The period pipeline and perturbation are mutually dependent, as the developer suspected:
  interiority at depth needs the derivative test (period doc), and the derivative test at depth
  needs perturbed evaluation (this doc). Design them as one unit.

## Sources

- Heiland-Allen, "Perturbation techniques applied to the Mandelbrot set" (write-up of Martin's
  technique, extended to derivatives + Newton + interior distance):
  https://mathr.co.uk/mandelbrot/perturbation.pdf and https://mathr.co.uk/web/m-perturbation.html
- Heiland-Allen, "Perturbation glitches" (2014):
  https://mathr.co.uk/blog/2014-03-31_perturbation_glitches.html
- Heiland-Allen, "Deep zoom theory and practice" (2021):
  https://mathr.co.uk/blog/2021-05-14_deep_zoom_theory_and_practice.html
- Martin, "SuperFractalThing Maths" (2013): http://superfractalthing.co.nf/sft_maths.pdf
- Khashin, "Fast calculation of the Mandelbrot set with infinite resolution" (2016):
  http://math.ivanovo.ac.ru/dalgebra/Khashin/man2/Mandelbrot.pdf
