# Gearbox — engine and type switching

Status: **landing** — `src/gearbox/` owns compute-gear vocabulary, PPS kernel
selection helpers, and the test-only Oracle path. Perturbation delta stepping
still lives in `delta_gear.rs` until fully moved; callers should prefer
`gearbox` for policy and HUD labels.

## Vocabulary (2026-08-12)

| Term | Meaning |
|---|---|
| **Gear** | Seat-worker / **compute kernel** path (`DirectKernel`, `PerturbationKernel`, Naive GPU, …). Not a numeric type. |
| **Type** | Numeric representation inside a gear (f32/f64/ScaledF64/FloatExp, …). |
| **Admit** | C-generator says this gear+type can carry the values actually iterated for the stencil (absolute `c` or δc), with render margin — see `docs/assistant/paraphrase-authoritative/c-generator-admit-margin.md`. |

Decision rule: **among gears that admit, lock highest expected PPS; within a gear,
smallest admitted type** (smaller type ⇒ better PPS for that gear). New stencil →
re-evaluate. Temporary blockiness from remapping old work is unrelated; rectangular
precision blocks while a deeper admitting option exists means admit/selection failed
(suspected: shallow false admit without ~10-bit C-gen margin).

## Separation of concerns

| Concern | Owner |
|---|---|
| Fastest safe delta recurrence (F64 → ScaledF64 → FloatExp) | `delta_gear` / gearbox policy |
| View admission → default **type** inside a gear | `gearbox::policy` + `CGenerator` |
| **PPS race** among legal compute kernels (**gears**) | `gearbox::policy` + `workshift` |
| Production naive absolute f64 | `DirectKernel` (scheduler-owned) |
| Production perturbation | `PerturbationKernel` |
| **Test-only** deep naive truth | `gearbox::oracle::OracleKernel` |

The scheduler (`workshift`) stays free of gear arithmetic. Gears answer “how
do I step this seat?”; queues, remap, and publish stay craftsmanship.

## PPS kernel selection (`r[cz.perf.pps-selected-kernel+1]`)

Do **not** assume Naive GPU is fastest. When Manual gear is **off**, on each
absolute view sample legal kernels (Naive CPU, Naive GPU if present, Perturbation)
for **one workshift each** (~10ms), then **lock** the highest measured
completed-points-per-second winner.

**Product default (2026-08-12):** Manual gear on, Naive (CPU DirectKernel).
Perturbation and Naive GPU stay in the settings radios; they are too buggy to
run from the automatic PPS race.
**Re-evaluate every ~500ms** (`PPS_REEVAL_INTERVAL`) so a gear that slows mid-fill
(especially Naive GPU) can lose the lock without spending most of the window
cycling. Relative (dishonest-naive) views hard-lock Perturbation with no race.

Manual gear (settings) overrides the race. Host numeric **type** remains automatic
from C-generator admission (with render margin).

## Oracle gear (bulletproof quality, tests only)

**Goal:** one answer oracle that remains correct at arbitrary depth without
perturbation, so every production gear can be red-teamed against it.

**Mechanism:** absolute Mandelbrot iteration in **FloatExp** (f64 mantissa +
wide exponent — the “slidy” float). No reference orbit, no series skip, no
f64 absolute collapse. Slow; never selected for the live app.

**Rules:**

- `r[cz.depth.oracle-gear+1]` — Oracle is test/bench only; production dispatch
  must not call it.
- Shallow views: Oracle membership/escape must match `DirectKernel` on the
  same absolute `c`.
- Deep views where f64 DirectKernel is not a membership oracle: compare
  production pert (or future gears) to Oracle, not to DirectKernel.

Salvage prose: precision-doubling rug oracle (`r[cz.depth.oracle-doubling+1]`)
remains the gold check for *perturbation math*; FloatExp Oracle is the
*scheduler-shaped* SeatKernel twin for workshift parity tests.
