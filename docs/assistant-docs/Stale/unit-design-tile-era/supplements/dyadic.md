# Supplement: dyadic.md

Pairs with authoritative `docs/design/dyadic.md`. Non-authoritative.

## Sampling algorithm (UD-DYAD-1) — inferred + assumed

Authoritative text: sample static tiles onto the stencil; nearest available datapoint; up-left on ties; sample smaller→larger magnification among ≤8 layers; no repeated remapping.

**Rule (inferred):**

1. Candidate layers = all hoarded GPU tiles whose magnification is in the active ≤8-homothety set and whose spatial extent may cover the stencil.
2. For each screen seat/row, walk layers from **highest magnification to lowest**.
3. Map seat/row → complex neighborhood via homothety IntExp (decrease precision / integer techniques; no search).
4. Take the **nearest** tile point in that neighborhood. On exact equidistance, take the further **up-left** (smaller seat, then smaller row).
5. First hit wins (higher mag preferred).

**Missing coverage (UD-DYAD-2) — inferred from architecture nores rule:**

If no tile covers the pixel, emit **nores** (infinity-through-the-system), not black-as-set.

**Half-offset (UD-DYAD-3) — inferred:**

Authoritative notes call half-offset unproven / moot for static-tile sampling. Do **not** apply a half-pixel bias in the sampling shader unless a later authoritative edit requires it.

## Sampling stability (UD-DYAD-4) — inferred

Tiles stay fixed at their addresses; each frame samples them onto the stencil once (nearest, up-left on ties). Do not rewrite tile data under pan/zoom. Tests: same exact pixels under 2× zoom vs equivalent pan after a single sample pass — not multi-hop remap chains. Zoom-fill / `View::fill_from` is a dead design idea.
