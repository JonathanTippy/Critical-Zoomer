# Supplement: homothety.md

Pairs with authoritative `docs/design/homothety.md`. Non-authoritative.

## Validity (UD-HOM-1) — inferred

Homothety = `(IntExp real, IntExp imag, i32 mag_pot)`.

Valid iff `mag_pot + PIXELS_PER_UNIT_POT` matches the inverse of the location exponents (auth). Invalid homotheties must not be published as stencils or tile keys.

## Operations (UD-HOM-2) — inferred from headgroup/arch

- Zoom bumps change `mag_pot` by ±1 and adjust location so the zoom origin (mouse or center) stays fixed in complex space.
- Pan adds IntExp deltas at current exponent.
- IntExp add commutative / mul associative tagged in auth headgroup — property tests live there.

Number representation details: `new/number_stack.md`.
