# Mandelbrot knowledge library

Curated, digested research for Critical-Zoomer's design problems. Purpose: depth (perturbation)
and GPU work have failed twice when approached by dead-reckoning; this library exists so design
starts from published, working theory instead.

**Trust tiering.** Sources are external research (cited inline per claim). Sections marked
"Leverage for Critical-Zoomer" are *our* interpretations — tier 2 until checked against the
code and, where behavioral, the developer.

## By design problem

1. **"Is this point inside, and with what period?"** — `period-and-interiority.md`
   Atom-domain candidate periods (free: v0.0.9 already collects them as smallness/small_time),
   Newton attractor solve, and the multiplier test |∂f^p/∂z| ≤ 1 — the non-arbitrary answer
   that replaces `determine_period` and answers the virtues-doc §13 ideal.
2. **"How do we compute past the f64 limit?"** — `perturbation.md`
   Reference orbit + delta iteration; glitches, detection, correction; reference choice.
3. **"How do we skip per-pixel work?"** — `series-approximation.md`
   Series coefficients from the reference only; plateau behavior; biseries with periodic
   references.
4. **"What numeric types where?"** — `numerics-and-precision.md`
   f64 → double-double → floatexp/rescaling → MPFR ladder; precision = reference policy.
5. **"How do we color/analyze answers?"** — `rendering-and-filaments.md`
   Exterior/interior distance estimation, continuous dwell, atom-domain structure display,
   exponential mapping.
6. **"Where do references come from?"** — `reference-orbit-strategy.md`
   Atom domains locate components, Newton finds nuclei, periodic references win; multiple
   references for difficult views.

The dependency spine: period pipeline ⟂-checked by derivatives → enables trustworthy filaments
and interiority → perturbation carries it all past f64 → series approximation multiplies the
win when references are periodic → reference strategy feeds the whole thing.

## Primary sources

- **mathr (Claude Heiland-Allen)** — the deepest open source for everything above:
  - Mandelbook (draft book with C99 reference code for most algorithms):
    https://mathr.co.uk/mandelbrot/book-draft-2017-11-10.pdf
  - Perturbation write-up: https://mathr.co.uk/mandelbrot/perturbation.pdf
  - Blog posts (2010–2021), linked per-topic in the docs above.
- **mrob / muency (Robert Munafo)** — the encyclopedia, for definitions and structure
  (mu-atoms, periods, atom domains, tuning): https://mrob.com/pub/muency.html
- **K. I. Martin**, "SuperFractalThing Maths" (2013), the original perturbation + series
  approximation popularization: http://superfractalthing.co.nf/sft_maths.pdf
- **Sergey Khashin** (2016), independent perturbation derivation:
  http://math.ivanovo.ac.ru/dalgebra/Khashin/man2/Mandelbrot.pdf
- **Kalles Fraktaler** practice (knighty's biseries/NanoMB) as described in mathr's deep-zoom
  writeup: https://mathr.co.uk/blog/2021-05-14_deep_zoom_theory_and_practice.html
