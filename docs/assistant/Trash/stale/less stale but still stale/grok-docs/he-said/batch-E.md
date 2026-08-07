# Batch E — before Phase 5 (2026-07-19)

Phase-start answers (plan defaults; naive GPU already on live path):

1. **wgpu entry:** use existing `wgpu` crate already in the app (egui-wgpu path); no new dependency.
2. **First GPU worker:** naive f32 tile filler first (landed), then perturbation f32.
3. **i32+i32 on GPU:** defer until FloatExp-f32+i32 works on CPU (Phase 6).
