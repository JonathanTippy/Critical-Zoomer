# New unit: Tile & answer encodings

No authoritative unit file. Non-authoritative.

## Tile (UD-TILE-1) — inferred

- Edge length **64** (power of two); may change later for throughput/aesthetics.
- Same magnification ⇒ shared homothety.
- Address = (homothety identity / mag + integer tile seat/row in that mag grid).
- Never transformed in place; sampling maps static tiles to viewport.

## Answer (published / headgroup) (UD-ANS-1) — inferred from auth tile_worker + architecture

Finished (best-effort) point stats for shading:

- Membership: Inside(period) | Outside(escape_time_r2, escape_z)
- min_magnitude_time (small time)
- min_magnitude (smallness)
- Slope angles as needed by shade annotation (escape-time slope; smallness slope) — store if not recomputable cheaply from neighbors

**Nores:** Outside with escape after the infinity convention (architecture / NORES); shade must not treat as inside-black.

All fields numeric for publisher clamp (D-PUB-2).

## CalibratedAnswer (pre-publisher) (UD-CAL-1) — inferred

Same conceptual fields as ranges (and agnostic union when in/out not yet proven). Highlights may exist as ranged bools for scheduler/shade prep; publisher collapses to Answer.

## GPU packing (UD-GPU-1) — assumed layout pending impl lock

Packed bytes for memory accounting (D-MEM-3) = `sizeof` the GPU answer texel × 64 × 64.

**Assumed texel (replace if encoding hardens differently):** fixed-size POD matching shade bindings (escape/period/smallness/angles as f32/u32 fields). Cost uses this packed size, not Vulkan allocation padding.

## CPU vs GPU variants (UD-TILE-2) — inferred

Workgroup may hold CPU or GPU-resident calibrated/work tiles. Headgroup hoard is GPU answers only. Uploader converts when needed; GPU-native work bypasses upload.
