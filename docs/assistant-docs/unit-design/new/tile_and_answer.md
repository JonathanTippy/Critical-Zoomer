# New unit: Tile & answer encodings

No authoritative unit file. Non-authoritative.

## Tile (UD-TILE-1) — inferred

- Edge length **64** (power of two); may change later for throughput/aesthetics.
- Same magnification ⇒ shared homothety.
- Address = (homothety identity / mag + integer tile seat/row in that mag grid).
- Never transformed in place; sampling maps static tiles to viewport.

## Answer (published / headgroup) (UD-ANS-1) — inferred from auth tile_worker + architecture

Finished (best-effort) point stats for shading. Architecture: **agnostic Answers are impossible** after the publisher — every published seat is a concrete Answer (possibly nores).

- Membership: Inside(period) | Outside(escape_time_r2, escape_z)
- min_magnitude_time (small time)
- min_magnitude (smallness)
- Slope angles as needed by shade annotation (escape-time slope; smallness slope) — store if not recomputable cheaply from neighbors

**Nores:** Outside with escape after the infinity convention (architecture / NORES); shade must not treat as inside-black.

All fields numeric for publisher clamp (D-PUB-2).

## CalibratedAnswer (pre-publisher) (UD-CAL-1) — inferred

Same conceptual fields as **ranges**. Membership may still be unproven here; that is calibrated honesty, not a published agnostic Answer. Publisher collapses to Answer via proximate bias (D-PUB-2). **In, period unknown** is allowed when interior is certain but twin-test has not passed (D-PER-4).

## GPU packing (UD-GPU-1) — assumed layout pending impl lock

Packed bytes for memory accounting (D-MEM-3) = packed size of one GPU calibrated/answer seat × 64 × 64.

**Assumed seat packing (replace if encoding hardens differently):** fixed-size POD matching publisher/shade bindings (escape/period/smallness/angles as f32/u32 fields). Cost uses this packed size, not allocator padding.

## CPU vs GPU variants (UD-TILE-2) — D-PUB-4 / D-GPU-7

- **WIP (GPU path):** dense active iterating state; finished seats removed and replaced from the **same tile** (D-GPU-8). Cross-tile refill = design fallback, approval required.
- **Calibrated tile (GPU path):** persistent per-seat calibrated buffer in VRAM; **every progressed seat** updated every bout, including partial ranges (D-GPU-7); publisher notified and binds directly.
- **Answer tile (headgroup):** publisher output only; GPU-resident.
- Uploader converts CPU calibrated → GPU calibrated when the worker was CPU. GPU-native work never goes through the uploader.
