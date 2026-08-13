# Manual gear (paraphrase)

Source: developer request 2026-08-10. Debug control only.

## Intent

Give the developer control over which **compute kernel** runs.
In this UI, “gear” means the **entire compute kernel** (Naive /
Naive GPU / Perturbation), not the per-seat F64 → ScaledF64 → FloatExp ladder
inside perturbation.

**Host type** (f64 vs FloatExp stack from depth admission) stays automatic.

Product **default** (2026-08-12): Manual gear **on**, Naive. Perturbation and
Naive GPU remain available as explicit radios; they are too buggy to pick
automatically. Uncheck Manual gear to restore the PPS race.

## UI

Settings window (⚙):

- Toggle: **Manual gear**
- When enabled, radio buttons: **Naive** | **Naive GPU** | **Perturbation**
- When disabled: automatic PPS / depth kernel policy
- **Default: enabled, Naive**

## Behavior

- Forced Naive → `DirectKernel` (CPU)
- Forced Naive GPU → naive GPU wave when available, else CPU DirectKernel
- Forced Perturbation → `PerturbationKernel` (including zero-orbit floor)
- Automatic → existing `perturbation_kernel_required` / GPU / Direct dispatch

HUD `mode:` reflects the forced kernel when manual gear is on.
