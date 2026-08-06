# Supplement: period_detector.md

Pairs with authoritative `docs/design/period_detector.md`. Non-authoritative.

## Loop detector (UD-PER-LOOP-1) — D-PER-3

**Every iteration:** compare current z to the loop-detector reference (prescribed algorithm). Cost is almost always one or two extra comparisons beside escape. Equality produces a period contender. Twin-test required before claiming period.

Auth allows tortoise-and-hare or POT as detector family; locked live choice is every-iteration equality + twin-test (not POT-only).

## Twin test (UD-PER-TWIN-1) — D-PER-1, D-PER-2, A-PER-TWIN-N

When a contender appears:

1. Iterate the two z values (and their derivatives) for **N = 20** steps (**assumed**, A-PER-TWIN-N; locked to `PERIOD_CONFIRMATION_ITERATIONS`).
2. At each step, require relative equality scaled to the active gear’s precision (**assumed form**):  
   `|a - b| ≤ ε_rel · max(|a|, |b|, scale_unit)`  
   where `scale_unit` is one ulp / least significant unit of the active type at the working magnitude.
3. Same relative test on derivatives.
4. If all N steps pass, contender becomes the period.

## Certainty / tenacity (UD-PER-CERT-1) — D-PER-4

Auth: do not claim more knowledge than is certain. Emitting a **false** period violates tenacity.

If twin-test has not passed but interior is certain: calibrated may emit **in, period unknown**. Certain period only after twin-test. Unknown period alone does **not** make a seat TPS-done (D-GPU-1: all fields determined).

## Default path (UD-PER-PATH-1) — D-PER-4

**Integrated** with ordinary iterate (auth: no separate stalling period phase). Period determination must not delay escape, play, or per-bout calibrated notify of partial truth. Expectation: only slightly slower than in-determination alone.

## Design fallback (UD-PER-PATH-2) — D-PER-5

**Two-pass fields** is a recorded design fallback only (standing rule in `decisions.md`: suggest with evidence, no impl without approval).

## GPU (UD-PER-GPU-1) — D-PER-6

Every-iteration equality + fixed-N twin-test. Branching confirm is fine; compact sparse contenders so the main bout stays uniform. Never invent a period. Period-edge / same-period fill uses certain periods only; unknown-inside is enough for early interior veto.

## Bucket-fill points (UD-PER-FILL-1) — D-GPU-1 / D-GPU-11

Auth: period bucket-fill still requires compute for small time / min magnitude. In-fill may omit those so the tile can **move on**; Phase 2 must still determine them (required two-phase, not a fallback).
