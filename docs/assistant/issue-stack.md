# Bug / todo stack (live)

**2026-08-06: codebase reverted to v0.0.9 (e6a0560).** The tile machine that generated most of this stack is no longer in the live tree. All tile-machine bugs and design gaps are **closed by revert** — their mechanisms (batches, tile versions, publisher gates, mag columns, GPU waits) do not exist at v0.0.9, which is exactly the point: see `docs/assistant/design/workgroup-virtues.md` for why those failure shapes have no handles in the golden design. Full tile-era detail: `Trash/issue-stack-tile-era.md`.

Two lists: **true bugs** (incorrect behavior) vs **design gaps** (intended design not yet on the live path). Do not clear an item until verified fixed (tests and/or visual).

PO quotes archive: `Trash/stale/less stale but still stale/grok-docs/he-said/`.

## Standing rule

Any new workgroup work (GPU, depth, perturbation) must not re-break the v0.0.9 invariants: one live target, one truth package, shared remap transform, small interruptible bouts, whole-snapshot publishes, fixed pivot order, no competing versions. A regression of these is a fail regardless of feature gain. Reference: `docs/assistant/design/workgroup-virtues.md` (enshrined).

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

- Re-verify headed: home render, scroll zoom, drag, resize, settings layers, bailout slider.

## Known issues (open)

- **Resolution changes not handled well.** v0.0.9 responds poorly to screen resolution changes — mostly going fullscreen / to a larger window. Suspected capacity issue somewhere (possibly channel capacities; never diagnosed). Believed fixable without breaking the design: the incremental WorkContext construction idea (a small cGenerator spreading the build across the pivot window — see Cleanup below) would likely address it, though channel capacities may also need attention. Never figured out; does not break the four guarantees or the design.

## Design gaps (open)

- **GPU port of the golden design** (`docs/assistant/design/design-target.md`): views not tiles, full remap of old work, v0.0.9 semantics on GPU. Not started; design must follow `docs/assistant/design/workgroup-virtues.md`.
- **Depth** (perturbation, reference orbits, arbitrary precision): v0.0.9 iterates f64 directly. Design starts from `mandelbrot-library/` — `perturbation.md`, `series-approximation.md`, `numerics-and-precision.md`, `reference-orbit-strategy.md` — not from dead-reckoning; when added, keep the invariants — one live target, one package, small interruptible units — and see suspended D-REF decisions in `unit-design/decisions.md`.
- **Lookahead/hoard across mags**: v0.0.9 remaps one screen only. The tile era's thin-tower lookahead failed by fragmenting the truth store; any future lookahead must extend the remap discipline, not replace it (virtues §3, §11).
- **Headgroup/shadergroup test strategy** (open problem): the workgroup now has property tests bound to its craftsmanship rules; the headgroup does not. The screenshot harness is the only net for visual bugs but needs use on every edit and image-description trust is imperfect; oracles can rot when output legitimately changes; the only known visual property so far is real-axis reflection symmetry. Needs a stronger strategy before the GPU shade port — the shadergroup was cut back last time partly for lack of tests.

## Salvage ports (from `salvage-from-code.md` — detail and evidence there)

Tile-era code worth porting, in suggested order. Delete each entry when ported.

- **Location readout + goto panel** (coords.rs in the tile-era stash, ~25 tests; un-stubs `SetPos`).
- **PPS counter** (`TpsCounter` in rolling.rs; feed = point counts in `update_sampling_context`; HUD string at window/mod.rs).
- **Coloring-layer add/remove + settings UI fixes** (settings.rs `template` factory; widgetize period-slider and animated-bailout change-detection fixes).
- **Zoom-debt input feel** (inputs.rs scroll/key debt accumulators; skip the mag_velocity helpers).

## Cleanup (from `docs/assistant/design/workgroup-virtues.md` §12 — the honest 10%)

Not bugs; provisional mechanisms that shipped because they beat nothing. None is load-bearing.

- **Delete token accounting** in the screen worker (`workshift.rs` / `screen_worker/mod.rs`): the budget check in the shift loop is commented out, wall-clock is the only law; the token fields and `spent_tokens_today` recomputation are dead code.
- **`Stec` → `Vec`** for the completion buffer (reserve once, reuse across contexts): same boundedness policy and pop-from-end freshness order, no 100k ceiling baked into the type, no inline array bulk.
- **Delivered-aware attention sampling**: the random walk re-picks finished seats; keep "gaze is a queue", replace the memoryless walk.
- **Incremental WorkContext construction**: a small generator spreading the O(pixels) build across the pivot window instead of one lump — a further play reduction. Likely also the fix for the resolution-change known issue above.
- **Completion staging buffer vs channel**: possibly redundant (batching + LIFO order are its only distinct contributions); keep only if demonstrably earning it.

## Done (recent)

- View remap associativity fixed: large-zoom remaps select source pixels from absolute plane
  positions; the saved `(0,513)` regression and generated associativity property are green.
- Period refinement replaced by the atom-domain candidate → Newton attractor → multiplier-test
  pipeline (`verified_period`); full-frame benchmark improved from 12.30 s to 234 ms with the
  same 10,302,563 counted iterations.
- Codebase reverted to v0.0.9 (e6a0560); tile-era code preserved in stash "tile-era code WIP before v0.0.9 revert"; `cargo check` green.
- v0.0.9 workgroup study enshrined: `docs/assistant/design/workgroup-virtues.md`.
- Tile-era design/unit-design/experiments moved to Stale; developer decisions annotated standing vs suspended.
