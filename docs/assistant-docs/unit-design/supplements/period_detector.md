# Supplement: period_detector.md

Pairs with authoritative `docs/design/period_detector.md`. Non-authoritative.

## Loop detector (UD-PER-LOOP-1) — D-PER-3

Use **power-of-two iteration-count snapshots** (not tortoise-and-hare). At each POT count, compare current z to the stored snapshot; equality produces a period contender `|i - snapshot_i|`.

## Twin test (UD-PER-TWIN-1) — D-PER-1, D-PER-2, A-PER-TWIN-N

When a contender appears:

1. Iterate the two z values (and their derivatives) for **N = 20** steps (**assumed**, A-PER-TWIN-N; locked to `PERIOD_CONFIRMATION_ITERATIONS`).
2. At each step, require relative equality scaled to the active gear’s precision (**assumed form**):  
   `|a - b| ≤ ε_rel · max(|a|, |b|, scale_unit)`  
   where `scale_unit` is one ulp / least significant unit of the active type at the working magnitude.
3. Same relative test on derivatives.
4. If all N steps pass, contender becomes the period.

## Certainty / tenacity (UD-PER-CERT-1) — inferred + issue-stack alignment

Auth: do not claim more knowledge than is certain. Emitting a **false** period violates tenacity.

**Inferred policy for regular iterate:** if twin test has not passed, emit `period == 0` (unknown). Full period resolve may still be a later phase on the in-edge (see live D-PER-1 in issue-stack); this supplement does not reopen that product sequencing — it only closes the detector algorithm choices left open in the unit doc.

## GPU (UD-PER-GPU-1) — inferred

POT snapshots + fixed-N twin loop are the GPU path. Branching twin test may be simplified but must still respect twin-test semantics before claiming a period.

## Bucket-fill points (UD-PER-FILL-1) — inferred

Auth: period bucket-fill still requires compute for small time / min magnitude. Period field may be propagated; other stats are not “done” until iterated.
