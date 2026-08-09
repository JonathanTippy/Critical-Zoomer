# Naive GPU design (views, not tiles)

Status: **implemented (first pass, 2026-08-08)** — live Naive GPU compute island
behind the restored view workgroup. Perturbation remains CPU. Sparse finish
harvest feeds the existing collector→escaper→colorer path. Live init runs on a
dedicated thread (nested `pollster` inside the async actor was unreliable);
adapter probe tries Vulkan then GL; `/tmp/cz_naive_gpu_status.txt` records
outcome. Headed/Xvfb HUD shows `mode:naive-gpu` when the adapter is up
(`CZ_FORCE_CPU_NAIVE=1` forces CPU). FLOP→IPS ratio tuning remains open.

Developer target: **v0.0.9 semantics on GPU** (`design-target.md`) — one live
view, full remap of old work, small interruptible bouts, whole-truth publishes.
Tiles / work collections stay out of scope.

## What this buys

Naive (direct) iteration is the shallow-view kernel (`DirectKernel` on CPU today).
Porting it to GPU is the first compute residency step: high IPS on home/shallow
frames without inventing a second scheduling world. Perturbation-on-GPU is later;
this doc is the naive path only.

## Platform

- **API:** wgpu compute. On Linux this is the common path (Vulkan via Mesa for
  Intel/AMD; GL fallback where needed). No NVIDIA-only API or CUDA requirement.
- **Baseline numeric type:** `f32` always. Every supported adapter must run the
  F32 pipeline.
- **Optional `f64`:** when the adapter exposes wgpu `SHADER_F64`, create the
  device with that feature and bind an F64 compute pipeline. When absent, stay
  on F32 (and/or fall back to CPU f64 / deeper gears for precision — depth design
  owns that ladder). Prefer F64 as a **precision gear**, not as “always faster.”
  FP64 is often much slower even when present.
- **Shader packaging:** two pipeline variants (F32 and F64), selected at device
  init from probed features — not one shader that switches scalar width at
  runtime.

## Parallelism vs craftsmanship (the hinge)

The golden machine looks serial because **authority** is serial. FLOPs need not be.

| Layer | Contract | GPU shape |
|---|---|---|
| Live target | One `LiveTarget` / one `WorkContext` + `frame_info` | Same. Discard in-flight GPU generation with the old world on `Replace`. |
| Coalesce / pivot | Drain-to-newest; flush completions then announce new frame | Same message order. No frame-A writes into frame-B package. |
| Scheduler | Slot rotation, queues, attention, neighbor discovery | Stays host **control plane**: indices, phase, do/don’t. |
| Bout | Bounded work unit (`BoutCap`) | **Wave of seats**, each still capped. Not one seat forever; not unbounded GPU runs. |
| Workshift | Wall-clock interruptibility (~10 ms law) | Return to the loop head often enough for pivots; adapt wave size / iter budget to GPU speed, do not lengthen the shift into stall territory. |
| Delivery | Provisional never marks delivered; undeliver on full buffer | Same honesty. Provisional edge guesses must not set final/done bits. |
| Publish | One current truth (whole snapshot / whole view) | Prefer GPU-resident view buffer + notify; full point-buffer readback is banned on the hot path. |

**Kernel seam.** Keep `SeatKernel`-shaped ownership: scheduler does not embed
Mandelbrot recurrence; the GPU kernel materializes/arms seats, runs
`BoutCap`-bounded waves, and maps finished seats to answers/calibrated commits.
Widening a bout from 1 seat to N seats is the intended change — not a second
scheduler.

### Dense WIP, not per-seat babysitting

1. Host arms a dense WIP index list from the existing queues (attention /
   scredge / edge / out / in), same policies as CPU.
2. One compute bout iterates that WIP with per-seat `BoutCap`.
3. Progressed seats commit **on device** (partial calibrated / answer state).
4. Tiny on-device done signals (counter / bitset) are the only completion
   authority the host may read for “final enough.”
5. Refill WIP from unfinished seats of the **same live view**; do not keep
   iterating seats that are already done enough.
6. Hard-seat policy preserved in spirit: rotate slow outs; provisional scredge
   stays undelivered until final.

Host bitsets/queues may choose *what to arm*; they are not completion authority.

## Queues and overhead

Host queue collect/arm of **indices** is expected to be negligible against the
FLOP→IPS bar when WIP waves are wide enough to feed the GPU.

Likely **not** an issue:

- `VecDeque` / spiral / edge pops that only produce seat indices between large
  waves.

Likely **is** an issue (avoid):

- Tiny WIP or one-seat-shaped dispatches (launch/fence dominate).
- Per-bout upload/download of full point or calibrated payloads.
- Host chaperoning every seat every bout with a sync wait.
- Full-screen rescans to rebuild work each bout.

Measure the FLOP→IPS ratio on **iterate-heavy** seats (deep/hard work). Shallow
exterior floods correctly show more scheduling/dispatch fraction; treat that as
a separate play/overhead check, not as failure of the arithmetic ratio.

## Performance target (particular)

Measure, on the same machine and workload class:

1. **CPU single-core FLOPs** (or a calibrated proxy tied to the live naive
   iterate), and **CPU naive IPS** full-stack (scheduling included).
2. **GPU total FLOPs** (or the same proxy for the GPU naive iterate), and
   **GPU naive IPS** full-stack under the view pipeline.

**Bar:**  
`IPS_gpu / IPS_cpu ≈ FLOPs_gpu / FLOPs_cpu_single` within about **±20%**, on
iterate-heavy work.

The older “≈20× CPU IPS” line (`D-GPU-10` classic wording) was a stand-in for one
hardware class. Absolute billions-IPS auth numbers stay suspended until the port
exists; the **method** is the measured FLOP ratio above. Absolute home TPS bars
remain separate product floors when reinstated.

Playability on weak GPUs: keep shift/tick period in the user-felt band; scale
work **N per shift** with a speed probe so 1× and 200× FLOP GPUs both keep
continuous visible output — not longer blocking GPU waits.

## Precision gears (naive only)

- Prefer GPU F32 when it distinguishes the view’s seats.
- Escalate to GPU F64 when features allow and F32 cannot honestly represent the
  view.
- Beyond that, CPU gears / perturbation (depth design) — do not pretend naive
  GPU covers deep zoom alone.

## Relationship to suspended tile-era GPU decisions

`docs/assistant/unit-design/decisions.md` still records tile-era D-GPU-* answers.
For this view port, keep the **policies** that still apply (no hot-path
readback; host control plane; dense WIP; provisional ≠ final; FLOP-tracking IPS)
and drop tile address / multi-tile tenacity as live requirements. Re-derive any
tick design from the virtues wall-clock workshift, not from the suspended
50 ms tile play tick as written.

## Out of scope (this doc)

- Perturbation / reference orbits on GPU.
- Tile managers, mag columns, publisher gates from the tile era.
- Redesigning the five-way queue rotation or pivot protocol.
- Implementing design fallbacks (multi-view GPU in flight, fully on-device
  intratile scheduler) without explicit approval.

## Acceptance sketch (when implementing)

- [x] wgpu path runs on non-NVIDIA Linux adapters with F32.
- [x] F64 pipeline selected only when `SHADER_F64` is present; F32-only adapters
      still complete views (or fall back).
- [x] One live view; pivot still flush-then-announce; no crossed frame writes
      (generation bump on Replace).
- [x] Workshifts remain interruptible; no unbounded GPU call (`BoutCap` waves).
- [x] Provisional publishes never set final/done.
- [x] Hot path does not read back full point/calibrated buffers (sparse finishes only).
- [~] Measured IPS ratio tracks measured FLOP ratio within ~±20% on
      iterate-heavy full-stack runs (probe scaffold in `workgroup_fitness` bench).
      **2026-08-08 grind:** compute/header-only path ≈ **140–160×** CPU on GTX 1080 Ti
      F32 (in FLOP-theory band). Sparse finals harvest still pays a ~10× sync tax
      (~12× CPU) even with few finals — finish-buffer readback next.
- [x] Queues remain index control plane; WIP width sufficient that queue time is
      noise in the iterate-heavy profile.
