# New unit: Number stack

No authoritative unit file (homothety validity only). Non-authoritative.

## IntExp (UD-NUM-INT-1) — inferred

Rug integer significand + `i32` exponent. Basis for all homothety locations. Tagged properties: add commutative; mul associative (where precision/membership allows).

Homothety validity ties exponents to `mag_pot + PIXELS_PER_UNIT_POT` (auth homothety).

## StackedIntExp (UD-NUM-STACK-1) — inferred from tile_worker gears

Stack gears: 1×…8× i32 limbs + exp (CPU & GPU where listed). Array `[i32;N]+exp` CPU-only. Prefer smaller.

## FloatExp (UD-NUM-FLOAT-1) — inferred

Float-with-exponent path for intermediate / SA / some worker needs. Not a substitute for IntExp homotheties.

## Rug float (UD-NUM-RUG-1) — inferred

Heap adaptive gear + reference orbits (discrimination + 20 bits). Least preferred for tile work; required for deep refs.

## Selection (UD-NUM-SEL-1) — D-GEAR-1

C generator fails closed if type cannot distinguish all stencil points **with
~10 bits of render headroom** (2026-08-12). Same margin on absolute `c` and
perturbation `delta_c`. No mid-iteration “upgrade gear” path: new stencil →
gearbox re-admits. Gear = compute kernel; type = cheapest admitted representation
inside that gear. See `docs/assistant/paraphrase-authoritative/c-generator-admit-margin.md`.
