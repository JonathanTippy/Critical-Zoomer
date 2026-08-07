# Bug / todo stack (live)

**2026-08-06: codebase reverted to v0.0.9 (e6a0560).** The tile machine that generated most of this stack is no longer in the live tree. All tile-machine bugs and design gaps are **closed by revert** — their mechanisms (batches, tile versions, publisher gates, mag columns, GPU waits) do not exist at v0.0.9, which is exactly the point: see `docs/design/workgroup-virtues.md` for why those failure shapes have no handles in the golden design. Full tile-era detail: `Stale/issue-stack-tile-era.md`.

Two lists: **true bugs** (incorrect behavior) vs **design gaps** (intended design not yet on the live path). Do not clear an item until verified fixed (tests and/or visual).

PO quotes archive: `../stale/less stale but still stale/grok-docs/he-said/`.

## Standing rule

Any new workgroup work (GPU, depth, perturbation) must not re-break the v0.0.9 invariants: one live target, one truth package, shared remap transform, small interruptible bouts, whole-snapshot publishes, fixed pivot order, no competing versions. A regression of these is a fail regardless of feature gain. Reference: `docs/design/workgroup-virtues.md` (enshrined).

## DAT failures (2026-08-01) — product watchlist on the v0.0.9 baseline

Developer acceptance test failed on the tile machine. Most items were tile-era implementation failures (closed by revert). The items below are **product-level**: they describe things v0.0.9 already did right, and they must keep working on the restored baseline. Verify headed before DAT.

- coloring options — v0.0.9 had the full layer menu; keep it complete (the tile era lost most kinds; do not repeat).
- normalization / period animation NaN — v0.0.9 is the golden oracle (RecipLn = ln(1/x)); GPU port must match it.
- animated / configurable bailout — v0.0.9 had it working; keep it working.
- intexp display and magnification in location readout — v0.0.9 displays these; keep the format clean.
- goto Apply accepts the HUD's own readout format and lands the same from any view (B-GOTO-1/2 lessons; v0.0.9-era coords behavior is the baseline to check).
- worker must never die silently and never constipate (B-PIVOT-1's lesson): at v0.0.9 the worker is a local 10ms-shift loop with hard-seat rotation — keep it that way; any GPU/depth extension must preserve interruptibility (virtues §4).
- zoom response: new work starts immediately on pivot at v0.0.9 (replace + remap); the 1–2s play regression was tile-era retarget machinery. Any successor must match the old latency, not "improve toward" it.
- unfinished pixels must never look like finished flat black: v0.0.9 fills from remap / provisional edge answers / Dummy-interior default; keep an honest-incomplete signal in any GPU port.

## True bugs (open)

- None known on the restored v0.0.9 baseline. Re-verify headed: home render, scroll zoom, drag, resize, settings layers, bailout slider.

## Design gaps (open)

- **GPU port of the golden design** (`docs/design/design-target.md`): views not tiles, full remap of old work, v0.0.9 semantics on GPU. Not started; design must follow `docs/design/workgroup-virtues.md`.
- **Depth** (perturbation, reference orbits, arbitrary precision): v0.0.9 iterates f64 directly. When added, keep the invariants — one live target, one package, small interruptible units — and see suspended D-REF decisions in `unit-design/decisions.md`.
- **Lookahead/hoard across mags**: v0.0.9 remaps one screen only. The tile era's thin-tower lookahead failed by fragmenting the truth store; any future lookahead must extend the remap discipline, not replace it (virtues §3, §11).

## Cleanup (from `docs/design/workgroup-virtues.md` §12 — the honest 10%)

Not bugs; provisional mechanisms that shipped because they beat nothing. None is load-bearing.

- **Delete token accounting** in the screen worker (`workshift.rs` / `screen_worker/mod.rs`): the budget check in the shift loop is commented out, wall-clock is the only law; the token fields and `spent_tokens_today` recomputation are dead code.
- **Remove the period-refinement stage** (`determine_period` with `timewarp_n_iterations` / `timewarp_4096`): never that good — timewarp slow, tighter-epsilon re-search untrusted; the one-line `iterations − loop_checkpoint` period sits commented out above the call and was nearly as good. The effective path is the newer derivative-based period-determination theory (not yet fully working anywhere); replace the whole apparatus when it lands rather than patching this one.
- **`Stec` → `Vec`** for the completion buffer (reserve once, reuse across contexts): same boundedness policy and pop-from-end freshness order, no 100k ceiling baked into the type, no inline array bulk.
- **Delivered-aware attention sampling**: the random walk re-picks finished seats; keep "gaze is a queue", replace the memoryless walk.
- **Incremental WorkContext construction**: a small generator spreading the O(pixels) build across the pivot window instead of one lump — a further play reduction.
- **Completion staging buffer vs channel**: possibly redundant (batching + LIFO order are its only distinct contributions); keep only if demonstrably earning it.

## Done (recent)

- Codebase reverted to v0.0.9 (e6a0560); tile-era code preserved in stash "tile-era code WIP before v0.0.9 revert"; `cargo check` green.
- v0.0.9 workgroup study enshrined: `docs/design/workgroup-virtues.md`.
- Tile-era design/unit-design/experiments moved to Stale; developer decisions annotated standing vs suspended.
