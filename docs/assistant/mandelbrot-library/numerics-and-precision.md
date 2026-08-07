# Numerics and precision

The numeric-type question underlies every deep-zoom design decision. What the field uses, and
when each runs out.

## The ladder

- **f64**: fine to roughly 2^-50-ish of pixel spacing; dies absolutely at the subnormal floor
  (~5e-324 absolute; practical relative-precision death much earlier, when pixel spacing
  underflows the mantissa at the coordinate magnitude — around zoom 2^-1022 worst case).
- **double-double** (qd-style): ~106 bits, no new exponent range — buys mantissa only. mathr
  notes Newton-basin work needing it is painfully slow in software.
- **floatexp** (f64/f32 mantissa + wide integer exponent): the workhorse for deep zooms. The
  perturbed deltas and series coefficients need *range*, not mantissa (see
  `series-approximation.md`: 53 mantissa bits suffice when the exponent is extended).
  Rescaling tricks (renormalize when |Δ| underflows) recover most floatexp performance with
  plain floats, except in the "deep needle" where exponents genuinely vary at full rate.
- **arbitrary precision (MPFR/rug)**: reference orbits only, and Newton solves — never per-pixel.

## The architectural consequence (mathr, "Deep zoom theory and practice", 2021)

The reference orbit is the *only* thing that needs full precision; everything per-pixel is
low-precision deltas against it. So precision policy = reference policy: pick the reference,
compute it at the precision the depth demands, and keep everything else floatexp-or-lower.
GPU note: GPUs are 32-bit territory — f32 deltas with rescaling, or f32+i32 floatexp pairs;
the high-precision path stays on CPU (matches our salvaged numeric-type roadmap).

## Leverage for Critical-Zoomer (interpretation — ours)

- Our IntExp (rug Integer + exponent) is already the right *coordinate* skeleton — the salvaged
  roadmap's "permanent skeleton, additive-only StackedIntExp, GPU is 32-bit only" is consistent
  with the above; the f64-depth bridge (screenspace factor past 2^-1024) is the rescaling trick.
- Precision is a per-reference decision: depth → bits. A view at zoom pot Z needs roughly Z+PPU
  bits in the reference; that formula should live in the reference worker's contract, not in
  per-point logic.

## Sources

- Heiland-Allen, "Deep zoom theory and practice" (2021):
  https://mathr.co.uk/blog/2021-05-14_deep_zoom_theory_and_practice.html
- Heiland-Allen, "Perturbation techniques applied to the Mandelbrot set" (precision discussion):
  https://mathr.co.uk/mandelbrot/perturbation.pdf
- Heiland-Allen, "Perturbation glitches" (2014; double-double Newton basin timings):
  https://mathr.co.uk/blog/2014-03-31_perturbation_glitches.html
