# Phase 5 — naive GPU worker (assistant notes)

Non-authoritative. Batch E locked in `he-said/batch-E.md`.

## Landed

- `NaiveGpuWorker` live on `TileSession` (`USE_PERTURBATION_CPU = false`)
- wgpu compute path for naive f32 tile fill (`naive_gpu_bout.wgsl` + worker)
- Period-unknown Inside answers emit `period == 0` (B-PER-2 / tenacity)
- Headgroup GPU display path consumes tile uploads (sampler → escape → shade)

## Follow-up (not Phase 5 closeout)

- `perturbation_gpu_worker` still empty — next GPU worker after naive
- Flip live path to perturbation when CPU/GPU parity + reference collection depth ready
- i32+i32 / FloatExp-f32 on GPU deferred (Batch E.3 / Phase 6)
