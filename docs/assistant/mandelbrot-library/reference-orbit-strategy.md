# Reference orbit strategy

Perturbation makes one question dominate all others: *where is the reference?* The published
answer has three parts: find components, find their nuclei, place references there.

## Finding components: atom domains

Atom domains (`period-and-interiority.md`) enclose hyperbolic components and are much larger,
so they are the practical way to *locate* a component from a rough view: the dominant period of
the view's center region tells you which component dominates; the domain size estimate
(Heiland-Allen 2013f: r = |F^q(0,c)| / |∂/∂c F^p(0,c)| at the nucleus, q the minimizing
earlier iterate) tells you how big it is.

## Finding nuclei: Newton on the critical orbit

The nucleus of the period-p component solves F^p(0, c) = 0. Newton in one complex variable:

    c ← c − F^p(0,c) / (∂/∂c F^p(0,c))

carrying `dc ← 2·z·dc + 1` alongside the iteration. Any point in the atom domain is a
reasonable starting guess (Mandelbook "Nucleus"; worked example: from −1.8 to the period-3
nucleus in 4 iterations). Basins are documented in "Mandelbrot set Newton basins" (2012b).

## Placing references

Kalles Fraktaler / mightymandel practice (from the glitch study and deep-zoom writeups):

- A **periodic** reference (nucleus of the dominating island) is strictly better than an
  arbitrary interior point: it enables the Newton-perturbation precondition (reference period
  dividing target period) and biseries period-skipping (`series-approximation.md`).
- The most-obvious reference (central minibrot) is often *not* the best: glitchy regions sit
  near embedded Julia sets between influencing islands, and references in nearby higher-period
  non-central minibrots — in the limit, pre-periodic (Misiurewicz) points at the spiral tips —
  do better there. Difficult views need **multiple references**.
- Component size estimates (size estimate: 1/(b·l²) from the critical-orbit slopes; child-size
  rules s/(q²sin(πp/q)) for cardioids, s/q² for circles) tell you when a view needs a deeper
  reference at all.

## Leverage for Critical-Zoomer (interpretation — ours)

- The salvaged rules land exactly here: "compute references near the mouse cursor
  continuously", "thorough nucleus seeks in idle time", "memory budget over many references in
  glitchy areas". The library version: cursor-nearby atom-domain → period → Newton nucleus →
  reference orbit at depth-appropriate precision, cached by IntExp location for reuse across
  magnifications ("no perturbation pause").
- The two-level seek the salvage doc describes (short interactive cap + background thorough
  seek) matches practice: a quick Newton from the view-center candidate is usually enough; the
  thorough seek (non-central minibrots near glitch clusters) is the idle-time upgrade.
- **v1 shipping constraints (restored design):** sticky selection prefers the deepest
  delivered interior that still covers the new viewport, else the new center; uncovered
  sticky refs are never carried (`reference_c_covers_frame`). Publish only when
  period-found or escaped — no artificial length wall. Prefer interiors / periodic nuclei
  when upgrading selection later; missing iterates are unfinished soft-continue, not
  Pauldelbrot glitch (`perturbation.md`).

## Sources

- Heiland-Allen, "Mandelbook" (draft 2017), chapters Nucleus / Size Estimate / Domain Size /
  Child Sizes / Shape Estimate: https://mathr.co.uk/mandelbrot/book-draft-2017-11-10.pdf
- Heiland-Allen, "Mandelbrot set Newton basins" (2012):
  https://mathr.co.uk/blog/2012-12-25_mandelbrot_set_newton_basins.html
- Heiland-Allen, "Atom domain size estimation" (2013):
  https://mathr.co.uk/blog/2013-12-10_atom_domain_size_estimation.html
- Heiland-Allen, "Perturbation glitches" (2014; reference placement case study):
  https://mathr.co.uk/blog/2014-03-31_perturbation_glitches.html
- Heiland-Allen, "Newton's method for Misiurewicz points" (2015; pre-periodic references):
  https://mathr.co.uk/blog/2015-01-26_newtons_method_for_misiurewicz_points.html
