Build V0.0.9 but on GPU

meaning:
- views rather than tiles
- full remap of old work
- etc

tiles / work collections prove way too complicated to get right.

## Current design lock (2026-08-08)

Normative write-up: `naive-gpu-design.md`.

In brief: wgpu (no NVIDIA requirement); F32 baseline with optional F64 when
`SHADER_F64` is available; keep craftsmanship **authority** serial (one live
view, queues as host control plane, `BoutCap` waves, pivot order) while making
**arithmetic** parallel inside a bout; IPS must track measured GPU-total vs
CPU-single-core FLOPs within about ±20% on iterate-heavy work.
