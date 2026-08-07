# Phase 2 display regressions + tile sampling (PO, 2026-07-18)

## Regressions

1. **Immediate display missing (grey).** Headgroup must have a sampler shader that samples from the tile collection stored on the GPU and owned by the headgroup.
2. **~15 fps.** On the GPU this process is practically free and should be extremely high fps. Easy to test.
3. **No escaper shader phase is unacceptable.** Opening the settings page also turns the window grey.
4. **Nitpick:** location UI appears in a seemingly random place; no clear input box.

## How sampling works with tiles (PO)

The screen is made of many tiles. Tiles are **not** children of a view or stencil; they are independent and **share a homothety**.

- **CPU:** compute seat differences for the GPU side, keeping IntExp operations O(1).
- **GPU:** sample the answer frame using those values (trivial), bailout with a small max iteration count (trivial), compute edges (almost trivial), and shade (trivial).

No excuse for low fps — sample / escape / edge / shade run on the GPU.

## Follow-up (2026-07-19)

PO: still losing tiles on move; drag/zoom vertical inversion; colors/settings must match old coloring_script exactly; partial on-screen edge tiles missing. Need full feature parity.

Fix direction: keep Answer tile hoard across same-zoom pans (remap origins); display via Color32 proximate `fill_from` + full escape+`color()` recolor when answers/settings change; clip edge-tile atlas uploads.