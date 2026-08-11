# Manual gear (paraphrase)

Source: developer request 2026-08-10. Debug control only.

## Intent

Give the developer more control over which **compute kernel** runs while
debugging. In this UI, “gear” means the **entire compute kernel** (Naive /
Naive GPU / Perturbation), not the per-seat F64 → ScaledF64 → FloatExp ladder
inside perturbation.

**Host type** (f64 vs FloatExp stack from depth admission) stays automatic.

## UI

Settings window (⚙):

- Toggle: **Manual gear**
- When enabled, radio buttons: **Naive** | **Naive GPU** | **Perturbation**
- When disabled: automatic PPS / depth kernel policy

## Behavior

- Forced Naive → `DirectKernel` (CPU)
- Forced Naive GPU → naive GPU wave when available, else CPU DirectKernel
- Forced Perturbation → `PerturbationKernel` (including zero-orbit floor)
- Automatic → existing `perturbation_kernel_required` / GPU / Direct dispatch

HUD `mode:` reflects the forced kernel when manual gear is on.
