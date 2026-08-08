# Supplement: intratile_scheduler.md

Pairs with authoritative `docs/design/intratile_scheduler.md`. Non-authoritative.

## Phase preference (UD-ITS-1) — inferred

Auth order (highest preference first when choosing work):

1. fill out  
2. edge  
3. scredge  
4. period edge  
5. flood in  
6. in  

Jobs as in auth. Tracing: depth-first boundary tracing.

## Preemption (UD-ITS-2) — D-SCH-3

A higher-preference phase **immediately** suspends a lower-preference mid-job. Suspended job state must be retained so it can resume when it again becomes the preferred work (tenacity; no discard of progress for preference alone).

## Period-unknown in-fill (UD-ITS-3) — auth + D-PER-4 + D-GPU-11

Auth: under unknown period, spread whatever period will be sent — same across the fill so it will not create a false in-filament. Calibrated may carry **in, period unknown** until twin-test (D-PER-4). In-fill does **not** supply min-magnitude/small-time; tile may move on; Phase 2 catch-up is required (D-GPU-11). Period-edge claims use **certain** periods (D-PER-6).

## Control plane vs worker (UD-ITS-4) — D-GPU-9

Default tile-worker schedule is GPU spiral-in unless this scheduler has provided more specific seats/queues. Intratile may stay on CPU as indices / do-don’t / phase (not point payloads, not per-bout chaperone). Worker owns iteration and dense WIP refill.
