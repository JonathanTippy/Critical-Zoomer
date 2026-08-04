# Supplement: tile_worker.md

Pairs with authoritative `docs/design/tile_worker.md`. Non-authoritative.

## Responsibilities (UD-TW-1) — inferred

Input: tile address (+ current stencil/reference context). Output: `Tile<CalibratedAnswer>` progressing under ranges. Tenacious: finish current tile before starting next unless cancelled for leaving the screen.

## Gear selection (UD-TW-2) — D-GEAR-1

Choose the smallest gear that can discriminate all stencil points (C generator succeeds). GPU preferred over CPU at equal capability.

**Mid-tile gear escalation is not a design path:** a gear sufficient for screen resolution is sufficient for iteration. Do not plan restart/convert paths for “gear too weak mid-tile.”

Gears list as auth (f32…rug). Adaptive heap gear last resort.

## Cancellation (UD-TW-3) — D-CANCEL-1

If the tile completely leaves the screen: cancel active work but **keep partial calibrated work in the hoard** so return can resume.

## Glitch (UD-TW-4) — inferred

Standard `|Z + z| << |Z|` test → fall back to big-z=0 orbit. Does not notify reference worker. Precision pressure comes from C generator / gear choice on subsequent work, not mid-tile escalate.

## Series approximation (UD-TW-5) — D-SERIES-1 (in scope)

**Design (inferred from standard perturbation practice + auth mention):**

1. Reference stores series coefficients / SA term (reference_worker).
2. Before full perturbation iterate, attempt series skip while **catastrophic absorption** holds (auth).
3. **Assumed absorption check:** skip while `|δz_series - δz_perturbed_estimate|` stays below a fraction of `|Z|` (fraction **assumed** 1e-3 of `|Z|` until tuned); on failure, fall back to normal perturbation from the last good iterate.
4. SA is an acceleration only; calibrated honesty unchanged — ranges still widen/narrow by proven bounds.
5. SA runs on CPU and GPU paths where the type supports the coefficient table; if a gear cannot hold SA terms, skip SA for that gear (still valid work).

## Default schedule (UD-TW-6) — inferred

Spiral in from outer edge unless intratile scheduler supplies queues. Derivative tracked for angle fields.

## CalibratedAnswer fields (UD-TW-7) — inferred from auth list

As auth: escape time & z; period; in/out; small time; smallness; escape-time slope angle; smallness slope angle. Encoding: `new/tile_and_answer.md`.

## GPU-native completion (UD-TW-8) — D-GPU-1…6

Host scheduling may still prefer tenacious focus on a tile. On the GPU-native path, **multiple tiles may be in flight in parallel** (D-GPU-6) as long as the interface stays the same:

1. **Complete** = every seat has **escaped or repeated** (D-GPU-1). Identical criterion to CPU.
2. Final Answers stay in the GPU tile/atlas. **No full Answer/point-buffer readback** (D-GPU-2).
3. Observe completion via an **on-device per-tile counter** bumped in the same finish path that commits the terminal **calibrated** seat (escaped/repeated) (D-GPU-3, D-GPU-4). Host may map/poll only that tiny counter for TPS / “tile done.”
4. Host done-bitmaps are for **arming/scheduling only**, not for declaring completion (D-GPU-5).
5. **Publisher** still receives the worker’s **calibrated tile** (GPU-resident on bypass) and biases with proximate — same as CPU (D-PUB-4). Publisher does **not** own completion (D-PUB-3); worker on-device counter does (D-GPU-*).
6. **Multi-tile concurrency** uses separate atlas slots + per-tile counters; do not invent a screen-wide completion protocol (D-GPU-6).

Edge tiles: counter target = valid seat count. Cancel because left screen: drop counter + production slot; do not count TPS (D-CANCEL-1 + D-GPU-4).
