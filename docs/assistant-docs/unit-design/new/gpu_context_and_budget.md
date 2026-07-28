# New unit: GPU context & budget

No authoritative unit file. Present in tree as `gpu_context` / `gpu_budget`. Non-authoritative.

## GPU context (UD-GPUCTX-1) — inferred

Owns the shared wgpu device/queue/surface wiring used by:

- Headgroup sample + shade
- GPU tile workers
- Uploader / publisher GPU paths
- Reference or other GPU compute if any

Single logical device for the process unless hardware forces otherwise. Init failure is fatal for headed mode (GPU preferred always-on).

## GPU budget (UD-GPUBUD-1) — inferred + D-MEM-*

VRAM side of the memory ledger:

- Limit L from settings means L CPU + L VRAM.
- Tile costs on the GPU side use packed GPU answer/calibrated bytes (D-MEM-3).
- Headgroup and workgroup GPU residencies both count; tile manager keep-set must stay within the VRAM half when evaluating GPU tiles.
- Bumps use exact need (D-MEM-1) and move the slider (D-MEM-2).

## Interaction with tile manager (UD-GPUBUD-2) — D-MEM-4

Budget numbers are inputs to the same pure tile-manager function on both assemblies. No separate ad-hoc GPU LRU that diverges the keep-set.

## Explicit hole (UD-GPUBUD-HOLE-1)

Precise multi-buffer overhead (pipelines, staging, swapchain) vs packed-tile accounting: **assumed ignored** for limit math until measured; if staging dominates, raise as a design hole rather than silently expanding packed-only accounting.
