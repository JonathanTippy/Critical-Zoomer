# Reference reuse (paraphrase)

Source: developer interview on `docs/authoritative/feasibility.md` (2026-08-10).
Not authoritative prose — a restatement for implementers.

## Product shape (already decided)

The central product requirement is **continuous browse UX**: pan and zoom stay
live via the headgroup; work keeps producing outputs while sitting still; there
must be no stalls. Re-continuation already buys continuous paint. What still
blocks feasibility is that **cost still shows through** — sitting still after a
move feels too slow for how much work should already be reusable. A loading-bar
“commit to a view” app (e.g. SuperFractalThing) can hide a different cost model;
this app cannot.

## The implementation suspect

Under-reuse of reference orbits is the main suspect, ahead of “perturbation is
inherently too expensive.” Zoom almost always drives the previous reference
**off-screen**; that is the normal case, not a reason to discard. A reference
that left the view is still near, on the scale of when it shared the screen with
the region being zoomed into, and will often still work.

Today’s “reference must cover the viewport” carry rule fights that reality.

Hot-loop craft (FloatExp constants, scaled divisions, gear overhead) may still
matter later; home zero-orbit f64 was brought near DirectKernel. The first fix
is reuse policy.

## Desired policy

1. **Greedy keep.** Keep references liberally. Do not drop a reference merely
   because it left the view, and do not assume a new orbit is required on every
   pan/zoom.
2. **Best ref per seat.** For each point, use the kept reference that seems best.
   Prefer that over a single global “current frame” reference gate.
3. **Glitch is local.** If a seat glitches against its chosen reference, that
   seat falls back to the **zero orbit** until a suitable reference exists for
   it. Do **not** abandon the reference for the whole screen because some seats
   glitched.
4. **Discard is secondary.** Memory must not leak unbounded forever, but budget
   and eviction are not the priority until liberal reuse is working. A plausible
   later rule: discard a reference if **no** seat on the screen used it (total
   glitch / unused). Do not block progress on inventing discard policy.

## What this is not

- Not “one reference per view, replaced at every pivot.”
- Not “viewport AABB coverage as the keep/drop gate.”
- Not “whole-reference invalidation on glitch.”
- Not a request to soft-floor correctness or stall rules while chasing speed.

## Status (2026-08-10)

Landing in code:

- Sticky selection keeps off-screen interiors.
- `from_stencil` / pending install carry refs without a viewport coverage drop.
- `reference_library` + `best_reference_for_c` for per-seat nearest-ref bind.
- Glitch remains local (`direct_only` → zero orbit).

Discard / byte-budget eviction still deferred.
