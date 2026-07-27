# Foundation Tracey rules (assistant-owned; not authoritative product spec)

Atomic rules for bottom-up units: IntExp/homothety, Range/calibration bias, NORES, Mandelbrot symmetry, off-screen geometry. Normative product text remains in `docs/requirements.md` / design; these tags exist for Tracey linkage only.

r[cz.math.intexp-add-commutative+1]

**Normative summary.** `IntExp` addition is commutative for finite values used as homothety coordinates.

**Acceptance criteria.**
- [ ] For arbitrary finite `a`, `b`, `a + b == b + a`.

r[cz.math.intexp-mul-associative+1]

**Normative summary.** `IntExp` multiplication is associative for finite values in typical coordinate ranges.

**Acceptance criteria.**
- [ ] For arbitrary finite `a`, `b`, `c` within test bounds, `(a * b) * c == a * (b * c)`.

r[cz.math.homothety-zoom-fill-associative+1]

**Normative summary.** Dyadic fill under successive equal zoom-ins is associative (4x equals two 2x fills).

**Acceptance criteria.**
- [ ] Covered by `View::fill_from` associativity property.

r[cz.math.mandelbrot-real-axis-symmetry+1]

**Normative summary.** Mandelbrot membership / escape classification is symmetric across the real axis for conjugate `c`.

**Acceptance criteria.**
- [ ] For sampled `c`, conjugating imag preserves inside/outside and escape-time when finished.

r[cz.range.guess-biased-nearest+1]

**Normative summary.** `Range::guess_biased` returns the bias when inside the range, otherwise the nearest endpoint.

**Acceptance criteria.**
- [ ] Bias inside → bias; bias below → lower; bias above → upper.

r[cz.display.nores-when-no-proximate+1]

**Normative summary.** Missing proximate work must surface as `NORES_ANSWER` (outside, escape after 1, infinite escape_z / min_magnitude), never as flat set-black.

**Acceptance criteria.**
- [ ] `NORES_ANSWER` packs/paints as Outside, not missing/Inside.

r[cz.display.window-default-800x480+1]

**Normative summary.** App defaults to 800×480 on startup and does not restore a customized size on launch.

**Acceptance criteria.**
- [ ] `DEFAULT_WINDOW_RES` is `(800, 480)`.

r[cz.display.offscreen-r2-circle+1]

**Normative summary.** Off-screen guidance uses the r=2 circle: fully off, mostly off (within 10% of fully off), too small (≤1px), mostly too small (<10% of screen).

**Acceptance criteria.**
- [ ] Classifier returns the four states from stencil geometry vs unit circle radius 2.
