# Foundation Tracey rules (assistant-owned; not authoritative product spec)

Atomic rules for bottom-up units: IntExp/homothety, Range/calibration bias, NORES, Mandelbrot symmetry, off-screen geometry. Normative product text remains in `docs/requirements.md` / design; these tags exist for Tracey linkage only.

> **2026-08-06 revert note.** Codebase is now v0.0.9 (e6a0560). Checked boxes earned on the tile machine must be re-earned against v0.0.9 symbols; per-rule notes below where the cited symbol changed. The product intent of every rule in this file still stands — these are foundation-level truths the golden design already embodies.

r[cz.math.intexp-add-commutative+1]

**Normative summary.** `IntExp` addition is commutative for finite values used as homothety coordinates.

**Acceptance criteria.**
- [x] For arbitrary finite `a`, `b`, `a + b == b + a`.

r[cz.math.intexp-mul-associative+1]

**Normative summary.** `IntExp` multiplication is associative for finite values in typical coordinate ranges.

**Acceptance criteria.**
- [x] For arbitrary finite `a`, `b`, `c` within test bounds, `(a * b) * c == a * (b * c)`.

r[cz.math.mandelbrot-real-axis-symmetry+1]

**Normative summary.** Mandelbrot membership / escape classification is symmetric across the real axis for conjugate `c`.

**Acceptance criteria.**
- [x] For sampled `c`, conjugating imag preserves inside/outside and escape-time when finished.

r[cz.range.guess-biased-nearest+1]

**Normative summary.** `Range::guess_biased` returns the bias when inside the range, otherwise the nearest endpoint.

**Acceptance criteria.**
- [ ] Bias inside → bias; bias below → lower; bias above → upper.
- Revert note: **suspended** — `src/range.rs` at v0.0.9 has the `Range` struct but no
  `guess_biased`; that method arrived with the tile machine. Recreate the verify if a biased
  guess returns with the GPU port (proximate-bias sampling).

r[cz.display.nores-when-no-proximate+1]

**Normative summary.** Missing proximate work must surface as an outside-flavored
placeholder (escape after ~1 iteration, infinitely far), never as flat set-black.

**Acceptance criteria.**
- [x] Placeholder packs/paints as Outside, not missing/Inside.
- Revert note: the tile-era `NORES_ANSWER` constant is gone. The v0.0.9 symbol is
  `CompletedPoint::Dummy{}` (`workgroup/screen_worker/workshift.rs`), which the escaper
  (`shadergroup/escaper.rs`) paints as outside-looking. Principle unchanged; see
  `docs/assistant-docs/collected-wisdom.md` ("Unknown is NORES, never black").

r[cz.display.window-default-800x480+1]

**Normative summary.** App defaults to 800×480 on startup and does not restore a customized size on launch.

**Acceptance criteria.**
- [ ] `DEFAULT_WINDOW_RES` matches the product default.
- Revert note: **discrepancy flagged** — v0.0.9 `constants.rs` has `(854, 480)`, this rule
  says 800×480. Resolve with developer (code change vs rule text change) before re-checking.

r[cz.display.offscreen-r2-circle+1]

**Normative summary.** Off-screen guidance uses the r=2 circle: fully off, mostly off (within 10% of fully off), too small (≤1px), mostly too small (<10% of screen).

**Acceptance criteria.**
- [ ] Classifier returns the four states from stencil geometry vs unit circle radius 2.
- Revert note: **feature absent at v0.0.9** — the classifier (`headgroup/window/offscreen.rs`)
  was tile-era. The product rule stands; the implementation needs porting to the restored
  headgroup.
