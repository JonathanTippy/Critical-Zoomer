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

- **B-SCH-3 home banding — fixed by f64 restore (2026-08-07).** Rectangular black columns at home (`1.3359375 + 0.125i mag 2^-2`) were caused by the FloatExp live-actor experiment. Restoring f64 production (worker→collector channel, perturb kernel, workshift, controller gate) eliminates the banding; `tmp/capture_at.sh` reports `black_cols_80=0`, matching known-good `ea27b4f`. The smaller `relative_location_from_index` divisor fix (data_res.0) is kept.
- Re-verify headed: resize, settings layers, bailout slider. (2026-08-07: home render, scroll zoom, and drag verified normal headed after the kernel-seam + reference-actor wiring.)
- **Phase-two home render corruption — closed under readiness wait (2026-08-07).** Earlier captures of giant black disk / rectangular discontinuity / flat purple were either pre-ready frames (purple: zero gray holes falsely treated as filled) or superseded by escaped-reference rejection + settled capture. Settled Xvfb home now shows coherent Mandelbrot structure; `e2e_visual.sh` passes with structure+baseline readiness (crop RMSE ~9.5k ≤12k). Keep the readiness gate; do not regress to gray-hole-only completion.
- **Uncovered sticky reference → classic glitch blobs (2026-08-07) — fix landed; headed corroboration via `scripts/e2e_faux_hard_path.sh` (PASS structure stdev≥800 on final).** Zooming into hard areas / minibrots (repro: `-0.161913425661 + 1.035546905361i mag 2^20`) while carrying a previous-view reference that no longer covers the viewport produced perturbation glitch blobs; dead-reckon goto to the same place did not. Root cause: `pending_reference` / `from_stencil` carried any non-escaped prior ref with no coverage gate, and sticky selection could request an off-screen prior interior. Fix: `reference_c_covers_frame` on carry + selection; drop uncovered pending; unit lock in `faux_user_zoom_to_hard_minibrot_matches_direct` + `sticky_selection_drops_interior_outside_new_view`. Faux-user headed path: `scripts/e2e_faux_hard_path.sh` + in-app PPM snip (`CZ_SNIPREQ`). Headed symptom also includes pps collapsing to 0 (`unfinished_frame_never_zero_pps_streak`). Short covering incomplete center refs can still disagree with DirectKernel in unit fixtures — production must not publish truncated orbits (`r[cz.depth.reference-until-done+1]`).
- **Finished-frame busy-spin — fix landed.** `percent_completed` was derived from the empty-queue break index (`(N-1)/N*100`), so completed frames never idled. Now delivered-fraction. Pin via never-stall / load-proportional follow-up.
- **Harness zombie processes — fixed (2026-08-07).** Repeated Xvfb captures left orphaned `critical_zoomer` / `Xvfb` processes. `tmp/capture_at.sh` now has an `EXIT` trap that stops the session and kills session-scoped app/Xvfb processes.
- **Short published orbit soft-stall / iteration wall — fix landed.** Library: missing iterate = `Unfinished`, not Glitch. Seats soft-continue on zero orbit (`δz ← z`). Reference worker no longer publishes at an artificial `max_iterations`/`MAX_BOUT` length wall — only period or escaped (`r[cz.depth.reference-until-done+1]`).

## Known issues (open)

- **Resolution changes not handled well.** v0.0.9 responds poorly to screen resolution changes — mostly going fullscreen / to a larger window. Suspected capacity issue somewhere (possibly channel capacities; never diagnosed). The stencil-only Replace + lazy seat init removes the ~47 MB per-pivot context transfer that was a plausible contributor; leave open pending headed verify that resize behavior improved. Channel capacities may still need attention. Does not break the four guarantees or the design.
- **High-res viewport lag (1080p) — display path, not worker fill speed.** Developer clarification (2026-08-07): the symptom is that everything under the viewport *feels sluggish and gets behind* — the pipeline accumulates lag — not that the worker fills the frame too slowly. Worker fill speed is at most a separate issue (isolated 1080p home frame: **688 ms** vs **228 ms** at 854×480; sub-linear vs pixel count). Prime suspects are per-frame repeated costs on the display path, each multiplied by 2.07M pixels at 1080p: the escaper's full-frame CPU pass (~8ms cadence, incl. neighbor scans + extra iterations), the colorer's multiple full-image passes plus frame-sized clone/allocation, and the window's full sampling pass plus pixel-buffer allocation, texture recreation/upload, and forced repaint. Per-pivot remapping also a candidate. None measured yet; profile each stage at 1920×1080 (escaper / colorer / sampler / texture upload / final frame time) before any redesign. Channel capacity and view/remap are not established causes. App otherwise verified working normally headed (2026-08-07).
- **Out-filament highlighting absent where verification is difficult.** After period correctness fixes, cloudy false positives are gone, but difficult areas can remain period 0 (unknown); unknown periods correctly create no out-filaments, so highlighting is absent there. Do not fix by publishing guessed periods. The resolution is stronger verification/continuation so difficult interior points eventually get verified periods.

## Design gaps (open)

- **Series approximation deferred (2026-08-08).** Live series on `PublishedReference` / `apply_series_skip` removed from the production path until relative `delta_c` + escaped-ref soft-continue membership stay green under `pin_exterior_not_marked_in_at_zoom_52` and `pin_not_blocky_delta_c_at_zoom_49`. Dormant code: `src/series.rs`. Rule `r[cz.depth.series-approximation+1]` is deferred.
- **Deep exterior black/"in" vs blockiness (2026-08-08) — root cause fixed.** Flip-flop from stuffing generator `delta_c` into the zero-orbit δc slot (false interior) vs iterating collapsed f64 absolute `c` without a reference (blocky). Production bug: `workshift` called `perturbation_reference_active()` *after* `latest_reference.take()`, so the held orbit was always dropped and seats iterated zero-orbit with generator `delta_c` → false `repeats` at iters≈2 (flat black) while HUD still showed `mode:pert`/`ref:complete`. Fix: decide publish-orbit use from the held snapshot. Pins (workshift path): `pin_exterior_not_marked_in_at_zoom_52`, `pin_not_blocky_delta_c_at_zoom_49`. Soft-continue still uses absolute `c` in the δc slot.
- **Depth integration — final gear push (2026-08-07).** **Closed for finish-line gates** except series (now deferred): live f64 host + F64→ScaledF64→FloatExp compute gears, HUD gear+IPS+PPS, home perturbation parity vs DirectKernel
  (~357 ms vs ~378 ms Criterion), scaled-f64 ~4.6× vs all-FloatExp microbench,
  ≥2^3600000 representable, visual suite / hard path / precision wall green.
  FloatExp-*host* banding root cause remains open but moot on the f64-host path.
- **First reference job length still = `MAX_BOUT` (1000); no mid-view extend.** **Closed (2026-08-07):** publish only on period/escape; no length wall. Intermediate snapshots before done remain an open question (see depth-design).
- **Reference fallback cache / pin / coverage chain only partially implemented.** Coverage gate + sticky drop are in; byte-budgeted cache and pin/evict are not.
- **Display-path latency profiling.** The high-res lag is a *display pipeline* problem (see Known issues): measure per-stage per-frame cost at 1920×1080 — escaper, colorer, window sampling, texture allocation/upload, repaint cadence — to find where backlog accumulates. Do not assume view/remap is the dominant contributor. Defer until after the deep-zoom type-switch milestone (or run in parallel if headed profiling is available).
- **GPU port of the golden design** (`docs/assistant/design/design-target.md`, `docs/assistant/design/naive-gpu-design.md`): views not tiles, full remap of old work, v0.0.9 semantics on GPU. **First Naive GPU pass landed 2026-08-08** (wgpu island, F32+optional F64, wave workshift, sparse harvest into collector). Remaining: FLOP→IPS ratio tuning, GPU-native shade/publish (not this pass), headed polish. Still must not re-break `workgroup-virtues.md`.
- **Certified `Boundary` completion state.** Dyadic pixel centers can only hit algebraically certifiable boundary parameters: exact parabolic points via rational cycle/multiplier checks, and Misiurewicz points via exact preperiodic repetition. Add a third completion state and separate coloring; do not impose an app effort cap. Explicitly deferred from the perturbation-core round, not forgotten.
- **Lookahead/hoard across mags**: v0.0.9 remaps one screen only. The tile era's thin-tower lookahead failed by fragmenting the truth store; any future lookahead must extend the remap discipline, not replace it (virtues §3, §11).
- **PPS-selected kernel (naive vs pert)** (`r[cz.perf.pps-selected-kernel+1]`): **in progress** — dual `DirectKernel` / `PerturbationKernel` dispatch; soft trial policy partial.
- **Headgroup/shadergroup test strategy** (open problem): the workgroup now has property tests bound to its craftsmanship rules; the headgroup does not. The screenshot harness is the only net for visual bugs but needs use on every edit and image-description trust is imperfect; oracles can rot when output legitimately changes; the only known visual property so far is real-axis reflection symmetry. Needs a stronger strategy before the GPU shade port — the shadergroup was cut back last time partly for lack of tests. In-app PPM snip (`snip.rs` / `CZ_SNIPREQ`) is a start for faux-user paths. Workgroup membership pins: `pin_exterior_not_marked_in_at_zoom_52`, `pin_not_blocky_delta_c_at_zoom_49`. Series package oracles are deferred with series itself. Paint/headgroup still screenshot-only.
- **HUD truth (2026-08-08):** metrics top-left show stack/mode/ref/gear; location+goto
  panel bottom-right (`coords-parse+2`, `location-readout+2`); mode flips
  `naive`→`pert` when reference installs; ref stays `complete` with reused refs,
  `wip` on cold start or post-glitch until newer generation installs.

## Salvage ports (from `salvage-from-code.md` — detail and evidence there)

Tile-era code worth porting, in suggested order. Delete each entry when ported.

- **PPS counter** (`TpsCounter` in rolling.rs; feed = point counts in `update_sampling_context`; HUD string at window/mod.rs).
- **Coloring-layer add/remove + settings UI fixes** (settings.rs `template` factory; widgetize period-slider and animated-bailout change-detection fixes).
- **Zoom-debt input feel** (inputs.rs scroll/key debt accumulators; skip the mag_velocity helpers).

## Cleanup (from `docs/assistant/design/workgroup-virtues.md` §12 — the honest 10%)

Not bugs; provisional mechanisms that shipped because they beat nothing. None is load-bearing.

- **Delete token accounting** in the screen worker (`workshift.rs` / `screen_worker/mod.rs`): the budget check in the shift loop is commented out, wall-clock is the only law; the token fields and `spent_tokens_today` recomputation are dead code.
- **`Stec` → `Vec`** for the completion buffer: storage is now a heap `Vec` with a fixed capacity (still 100k, still LIFO pop-from-end). The old inline array form is gone because worker-side shell install could not put two ~8 MB arrays on a default stack. Remaining cleanup: drop the fixed ceiling if a growable policy is preferred.
- **Delivered-aware attention sampling**: done as the attention square-ring spiral (`cz.craft.attention-spiral+1`).
- **Incremental WorkContext construction**: done as stencil-only Replace + lazy `ensure_started` (see `cz.craft.stencil-only-replace+2`). Chunked amortization beyond first-start laziness remains optional if install-time shell work ever shows up in play.
- **Completion staging buffer vs channel**: possibly redundant (batching + LIFO order are its only distinct contributions); keep only if demonstrably earning it.

## Done (recent)

- Perturbation delta kernel (milestone 2) — **closed for correctness** (gear ladder,
  series, glitch handling). Production dispatch migrating to PPS-selected naive vs pert
  (`r[cz.perf.pps-selected-kernel+1]`); `DirectKernel` is production naive path and
  test oracle.
- In-filament detection now carries the Mandelbrot derivative through remap and extrapolates
  the escape field across the four screen neighbors before applying the existing one-pixel
  peak test. Derivative, convergence, and 2x/4x ridge-survival tests are green. Pending headed
  visual verification that interim zoom frames retain the expected thin filaments. The same
  data-vs-screen defect remains open for small-time node highlighting.
- Stencil-only Replace: controller sends `frame_info` only; worker builds an uninitialized
  shell and materializes seat `c`/`z`/`dc` from `CGenerator` at first start. Reuses points /
  mixmap / completion buffers across pivots. `time_to_first_publish` −42%; full-frame unchanged.
- Attention-first square-ring spiral owns slot 0; `Option` attention (`None` = pointer
  off-screen → center anchor). Full-frame −29% vs prior; first-publish tradeoff noted in
  benchmarks.
- View remap associativity fixed: large-zoom remaps select source pixels from absolute plane
  positions; the saved `(0,513)` regression and generated associativity property are green.
- Period refinement replaced by the atom-domain candidate → Newton attractor → multiplier-test
  pipeline (`verified_period`); full-frame benchmark improved from 12.30 s to 234 ms with the
  same 10,302,563 counted iterations.
- Codebase reverted to v0.0.9 (e6a0560); tile-era code preserved in stash "tile-era code WIP before v0.0.9 revert"; `cargo check` green.
- v0.0.9 workgroup study enshrined: `docs/assistant/design/workgroup-virtues.md`.
- Tile-era design/unit-design/experiments moved to Stale; developer decisions annotated standing vs suspended.
