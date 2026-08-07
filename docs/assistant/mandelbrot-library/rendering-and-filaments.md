# Rendering, distance estimation, and filaments

What to do with iteration data once you have it — the published coloring/analysis layer. This
informs the shade-layer design (the GPU-shade port is a design gap; the shadergroup was cut back
last time partly for lack of tests, so candidate layers here should be chosen for testability).

## Exterior distance estimation

For escaped points, the distance from c to M is approximated by (Fisher bounds in Peitgen &
Saupe 1988; Heiland-Allen 2010):

    d = 2·|z|·log|z| / |∂z/∂c|     (complex-valued variant carries direction: 2·z·log|z|/ (∂z/∂c))

with the c-derivative iterated alongside: `dc ← 2·z·dc + 1`. There is a point of M within d;
no point of M within d/4. Compare d to pixel spacing to know whether the set might intersect a
pixel — a principled "is this pixel resolved" test, and the basis of adaptive supersampling
(Heiland-Allen 2014e).

## Continuous dwell

Vepstas' renormalized escape time μ = n + 1 − log2(log|z|) is almost independent of both escape
radius and iteration cap — a smooth, band-free exterior coordinate from data v0.0.9 already
collects (escape time + escape location).

## Interior distance estimation

Needs the period pipeline (`period-and-interiority.md`): with the Newton-converged attractor z0
of period p, iterate one p-cycle carrying four derivatives
(dz, dzdz, dc, dcdz — recurrences in Mandelbook) and:

    d = (1 − |dz|²) / |dcdz + dzdz·dc/(1 − dz)|

Non-negative exactly when interior — the formula *is* the interiority test with distance as a
byproduct.

## Atom domains as structure display

Coloring by the argmin-|z_n| index (Munafo/Peitgen-Richter) paints regions enclosing each
hyperbolic component — the published relative of our out-filament idea (period changes among
neighbors). Heiland-Allen's "modified atom domains" (2012c) variant makes small domains more
visible.

## Exponential mapping

For deep-zoom video/stills, map screen y through an exponential so vertical position is
proportional to log-distance — keeps detail density uniform down a deep zoom instead of a
featureless center (Heiland-Allen 2010f, 2014g with Kalles Fraktaler).

## Leverage for Critical-Zoomer (interpretation — ours)

- **Filaments**: in-filaments from escape-time slope sign changes and out-filaments from period
  changes (collected-wisdom) are heuristic; the published upgrades are exterior/interior
  *distance* fields, which are smooth, testable (numeric oracles), and composable with
  anti-aliasing. A distance-based filament layer would also give the "pixel resolved?" test
  that honest-incomplete rendering wants.
- All of the above are pure functions of per-point answers + derivatives — shade-time work,
  consistent with the answer-only hoard rule (no color hoard; colors exist only at shade time).
- Derivative storage per Answer has a size cost; the salvaged "derivative direction on Answers /
  GPU Answer in 32 bits" note is the constraint to design against.

## Sources

- Heiland-Allen, "Mandelbook" (draft 2017), Graphical Algorithms chapters (membership, escape
  time, continuous dwell, exterior coordinates/distance, atom domains, interior
  coordinates/distance — all with C99 reference code):
  https://mathr.co.uk/mandelbrot/book-draft-2017-11-10.pdf
- Heiland-Allen, "Interior distance": https://mathr.co.uk/web/m-interior-distance.html
- Heiland-Allen, "Adaptive super-sampling using distance estimate" (2014):
  https://mathr.co.uk/blog/2014-11-22_adaptive_supersampling_using_distance_estimate.html
- Heiland-Allen, "Exponential mapping with Kalles Fraktaler" (2014):
  https://mathr.co.uk/blog/2014-12-17_exponential_mapping_with_kalles_fraktaler.html
- Vepstas, "Renormalizing the Mandelbrot escape" (1997):
  http://linas.org/art-gallery/escape/escape.html
