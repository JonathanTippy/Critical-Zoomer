# Supplement: shaders.md

Pairs with authoritative `docs/design/shaders.md`. Non-authoritative.

## Pipeline (UD-SHADE-PIPE-1) — inferred

1. **Sampling shader** — full frame of answers (dyadic supplement).  
2. **Shading shader** — edge annotation → escape → layered coloring.

## Edge annotation

### In-filament (UD-SHADE-INFIL-1) — D-SHADE-1, A-SHADE-INFIL

Detect hard inversion of escape-time slope angle between neighbors.

**Assumed constant:** angle delta **> π/2** ⇒ in-filament candidate pixel(s). Paint via the in-filament **layer** in the script (D-COLOR-4), not a hidden post-pass.

### Out-filament (UD-SHADE-OUTFIL-1) — D-SHADE-3

Period step between neighbors: **any** period change. Paint **only the higher-period side** of the edge (not both sides, not the lower-period side alone).

Unknown (`period == 0`) vs known: a change that involves 0 still counts as a period change under “any change”; prefer not fabricating filaments from 0↔0. **Inferred:** 0 is not a real period; treat 0↔nonzero as a step only if the nonzero side is the higher-period side being painted (nonzero > 0).

### Nodes (UD-SHADE-NODE-1) — D-SHADE-2, A-SHADE-NODE

Auth: smallness approaches zero; also hard angle inversion of smallness.

**Assumed:** seed when `min_magnitude` is below one tile point-spacing at that magnification; refine with smallness slope-angle inversion as secondary confirmation.

### Small-time edge (UD-SHADE-STE-1) — inferred from auth tag + issue-stack

Exterior `small_time == 0` is valid for **paint**. STE ridge detection may filter zeros for edge highlighting only — do not treat paint zeros as missing.

## Escape (UD-SHADE-ESC-1) — inferred + D-BAIL-1

Continue escape coloring to the configured bailout using stored `escape_z`. Bailout changes recolor only; membership unchanged; no rework.

## Layered coloring (UD-SHADE-LAY-1) — D-COLOR-2, D-COLOR-3, D-COLOR-4

For each layer in script order:

1. Read source field from answer (or highlight flag from annotation).
2. Apply normalization (log / reciprocal / none — see settings).
3. Apply colorizer (sin / modulo).
4. Tint with base color.
5. Alpha = inside_opacity or outside_opacity by membership.
6. Composite with **alpha-over** onto the accumulator.

Default script contents: D-COLOR-1 (see `new/settings_and_coloring.md`).

## Nores (UD-SHADE-NORES-1) — inferred

Nores is indistinguishable from a finished outside answer at infinity. Must not shade as flat inside-black.
