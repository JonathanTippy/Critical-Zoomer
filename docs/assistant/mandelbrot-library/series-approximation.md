# Series approximation

The second deep-zoom accelerator: because the perturbed delta is a polynomial in Δc whose
coefficients depend **only on the reference orbit**, you can evaluate a truncated series once
per pixel and *skip* a large block of per-pixel iterations.

## The technique (Martin 2013; Heiland-Allen 2016)

Write the perturbed orbit as a series in the initial delta:

    ⟨⟨z_n⟩⟩ = Σ_k a_{k,n} · ⟨⟨c⟩⟩^k

The coefficient iterations are simple nested loops (Heiland-Allen, "Simpler series
approximation", 2016 — the earlier symbolic-algebra code generation is unnecessary):

    a_{1,n+1} = 2·z_n·a_{1,n} + 1
    a_{k,n+1} = 2·z_n·a_{k,n} + Σ_{j=1..k-1} a_{j,n}·a_{k-j,n}

(and a companion b-series for the derivative, needed if distance estimation rides along).

Practical facts from the same post:

- Coefficients need only **f64 mantissa with an extended exponent** (the values get huge) — a
  floatexp-like type, not full arbitrary precision.
- Skippable iterations **plateau** when zooming toward a fixed reference: doubling the series
  order buys roughly one extra reference-period of skipping. So order should be chosen
  automatically from the reference minibrot's size, not cranked up globally; high orders at low
  zoom cause glitches.
- After the skipped prefix, per-pixel perturbed iteration resumes as usual; an escape-radius
  check on the series partial sums bounds when the truncation stays valid.

## Biseries (knighty; KF NanoMB1/2)

If the reference C is itself periodic (e.g. a minibrot nucleus), the series in **two** variables
(z and c) can skip a *whole period* of iterations per step, repeatedly — a large extra speedup
near minibrots, at the cost of a z-escape-radius derived from the reference's properties.

## Leverage for Critical-Zoomer (interpretation — ours)

- This is exactly the salvaged "series approximation as exact skip" rule: coefficients built
  only from the reference orbit, precomputed once, stored beside it; each pixel searches for
  the last step where the z² term is still absorbed, then runs a short perturbed tail.
- The plateau fact sets expectations: series approximation is a constant-factor-to-log win per
  reference, not unbounded; the leverage multiplies when combined with periodic references
  (biseries) — which again ties back to the period pipeline (`period-and-interiority.md`) and
  reference strategy (`reference-orbit-strategy.md`).

## Sources

- Heiland-Allen, "Simpler series approximation" (2016):
  https://mathr.co.uk/blog/2016-03-06_simpler_series_approximation.html
- Heiland-Allen, "Code generation for series approximation" (2016, superseded but background):
  https://mathr.co.uk/blog/2016-02-26_code_generation_for_series_approximation.html
- Heiland-Allen, "Deep zoom theory and practice" (2021), incl. NanoMB1/2 description:
  https://mathr.co.uk/blog/2021-05-14_deep_zoom_theory_and_practice.html
- Martin, "SuperFractalThing Maths" (2013): http://superfractalthing.co.nf/sft_maths.pdf
