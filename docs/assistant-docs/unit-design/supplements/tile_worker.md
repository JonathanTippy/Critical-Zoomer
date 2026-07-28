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
