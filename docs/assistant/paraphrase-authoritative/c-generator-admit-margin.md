# C-generator admit margin (paraphrase)

Source: developer interview 2026-08-11/12
(`docs/assistant/interviews/2026-08-12-precision-wall-gear-switching.md`).
Not authoritative prose — restatement for implementers.

## Gear vs type

- A **gear** is a seat-worker / compute kernel (naive, perturbation, naive GPU, …).
- A **type** is the numeric representation that kernel uses for the hot values.
- HUD labels may still conflate them; design vocabulary does not.

## Gearbox decision

1. On a new stencil, evaluate which **gears admit** via the C-generator.
2. Among admitting gears, pick **fastest expected PPS** (measured race; do not assume GPU).
3. Within the winning gear, use the **smallest type** the C-generator admits
   (smaller type ⇒ better PPS for that gear).

Blockiness from rescale of prior work is unrelated. If a deeper admitting option
exists, rectangular precision-blockiness must not appear — that means admit,
selection, **or a later precision drop** failed.

**Three failure shapes (interview 2026-08-12):** (A) C-gen admits nothing →
worker stops; (B) C-gen false-admits shallow → rectangular low-res; (C) C-gen
admits honestly, then code **after** admit uses insufficient precision (e.g.
an f64 interlayer) → same rectangular look. Naive or pert; do not split the
theory. Live host completions are f64 (`WorkUpdate<f64>`); treat that as a
(C) suspect, not as “depth closed.”

## What C-generator judges

Gate the values **actually used in computation**:

| Path | Values admitted |
|------|-----------------|
| Naive / absolute | absolute seat `c` |
| Perturbation / relative | `delta_c` vs the candidate reference |

It is the admission gate for precision — fail closed when the type cannot
honestly carry those values for this stencil.

## Render margin (~10 bits)

Neighbor distinguishability is necessary but **not sufficient**.

Admission must keep about **10 bits** of headroom beyond “adjacent seats stay
distinct in `T`”. Same margin on absolute and δc paths. Rationale: polar /
Mandelbrot dynamics need a little margin even when Cartesian seat spacing looks
fine; at shallow depth ~10 bits has been enough for correctness; deeper, the
extra may approach zero, but leaving the margin at 10 is fine.

**Suspected live bug:** missing this margin → shallow type false-admits →
gearbox never steps deeper → rectangular blockiness at transitions while
deeper gears “exist.” A default-10-bit probe is in the C-generator (settings
override for testing). **The visual/product bug is still open** until the
developer confirms.

## Perturbation evaluation order

When scoring the perturbation gear for a stencil:

1. Choose candidate ref = **nearest kept reference to the screen**.
2. Build the δc stencil for that ref.
3. Run admit (distinguish + ~10-bit margin) for candidate types.
4. Take the smallest type that passes.

A closer ref can admit a cheaper type for the same view; uncommon, must be correct.

## Precision wall (deprioritized)

A stencil with **no** admitting gear+type is a precision wall. In practice this
is extremely unlikely for “ref too far for δc”: work already has refs near the
current magnification, and types span on the order of **≥20 magnifications**.
Do not prioritize exotic wall UX for that case.

## Glitch (separate)

Glitch is not “discard the reference.”

- References are **liberally / greedily saved**.
- **Never discard a reference because it glitched.**
- Only **glitching seats** pause (hold off wrong answers) until a **better ref**
  is available for those seats.
- Do not reject the whole gear or invalidate the ref globally for local glitch.

Product stuck case worth caring about: seats that need a non-glitching ref and
none is available yet — work those seats when a suitable ref appears.

## Not this

- Black wrong-interior on naive is **not** explained as C-gen false pass
  (false pass looks blocky). Treat as periodicity-detection tangent unless new
  evidence ties it to admit.
- Gear ≠ type; do not “promote type” as a separate story from “gear admits
  with its cheapest type.”
