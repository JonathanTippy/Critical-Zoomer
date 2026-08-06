# Supplement: headgroup.md

Pairs with authoritative `docs/design/headgroup.md`. Non-authoritative.

## Frame loop (UD-HG-1) — inferred

Each frame (capped 60fps):

1. Ingest available GPU tiles into the headgroup tile collection (tile manager keep-set).
2. Sample collection → full-frame answers (sampling shader; dyadic supplement).
3. Shade → colors (shading shader; shaders supplement + settings).
4. Present; HUD/widgets on top.

## Stencil ownership (UD-HG-2) — inferred + D-STEN-1

Headgroup owns the live stencil. On any change to homothety, resolution, mouse, or mag velocity, bump sequence number and send stencil to workgroup.

## Controls timing (UD-HG-3) — inferred

- Pan: integrate using **elapsed time**.
- Scroll zoom: apply bumps with **debt gaps** so fast spinning does not skip or backlog ticks (requirements Fast).
- Drag-anchor location stored in full IntExp for zoom-out then zoom-back (auth headgroup).

## Off-screen arrows (UD-HG-4) — inferred from auth thresholds

Auth gives geometric tests for off / mostly-off / too-small / mostly-too-small using the r=2 circle. Red arrows when those fire. Details of arrow chrome: `new/window_controls.md`.

## Settings residency (UD-HG-5) — inferred

Settings struct lives in headgroup; coloring script drives shade. Model: `new/settings_and_coloring.md`. Memory bumps update the slider visibly (D-MEM-2).
