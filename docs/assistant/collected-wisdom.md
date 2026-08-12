# Collected wisdom (surviving content from stale docs)

Harvested 2026-08-06 during the v0.0.9 revert cleanup. Each entry: the rule, its source, and whether v0.0.9 already honors it. Anything contradicted by the golden design was dropped; contradictions are resolved in favor of `docs/assistant/design/workgroup-virtues.md`.

**Trust tier 1** — vetted against the v0.0.9 study. For unvetted material rescued from the Trash docs (tier 2, lower trust), see `salvage-from-trash.md`; promote entries here only after checking them against the v0.0.9 code and, where behavioral, the developer.

## Correctness rules (binding at any implementation)

- **Unknown is NORES, never black.** Unfinished pixels must read as "escape after ~1 iteration" (infinitely far), not interior black. Source: developer quote 2026-07-18 (B-TEN-1). v0.0.9 honors this: unset pixels are `Answer::Dummy { iter_count: 0 }` — outside-looking, never flat interior. The "broken real axis" antenna artifact was this bug (home Im=0 lands exactly on seat y=256 before the half-step offset), not a math error.
- **Points outside r=2 have small_time 0.** That is a plain 0, not a discontinuity; shaders should not special-case it. v0.0.9's `smooth_start_offset` (0 for first-iteration escape) implements this.
- **Never false periodicity.** The regular iterate may only apply the simplest *certain* inside check; it may leave period unknown, but must never claim a wrong period. Heavy period machinery bolted onto every mid-loop repeat is an antipattern. Period determination belongs after boundary + out-fill. Paint must tolerate Inside-period-unknown next to Inside-period-known without a seam. v0.0.9's orbit-log loop detection is exactly the "simple certain check" (finds the true loop on the cycle, no false positives); its `determine_period` refinement stage (timewarp + tighter-epsilon re-search) was never that good and is slated for removal — the newer derivative-based period-determination theory is more effective but has no fully working implementation yet. When it lands, it replaces that stage wholesale.
- **Period confirmation fixed-N = 20** (developer answer, batch B1.3) when a confirmation pass is used.

## Screen-edge and fill scheduling (v0.0.9 behavior, must survive any rewrite)

- **Walk the entire screen edge first.** Mandatory, not optional: islands of the set that appear disconnected in the current view all connect to the outer region somewhere off-screen, so the out-bucket-fill only catches them if the whole border was seeded. v0.0.9 does this (the `scredge` wall-walk is its first-class queue, not a side phase).
- **After edges are walked, hint in-areas as inside with period propagation** from the enclosing edge — but still compute each point for its own min magnitudes; take the easy win on top of the real work, not instead of it. v0.0.9 gets most of this via neighbor propagation from seats; a dedicated in-fill hint pass is a legitimate future refinement.
- Bands of unfinished work with regular shapes are a scheduling/channel symptom, not math — check the queues before the arithmetic.

## Storage, hoard, memory (product rules; v0.0.9 implements the screen-sized core)

- **One answer per view.** No competing versions of a pixel. (v0.0.9: one package, one id; tile era violated this and failed.)
- **Pan:** remapping old work into the new frame is simple and mandatory — hoard everything still in frame.
- **Zoom in:** remap the previous frame and write new work on top; the small inefficiency is acceptable.
- **Zoom out:** if memory allows, restore old work instead of redoing it. v0.0.9 does this — the replaced package is kept as the hoard and its (center_x, center_y, mag) transform restores on zoom-out.
- **Eviction order under pressure:** historical hoard first, then lookahead hoard. Current and lookahead reference orbits are never evicted and count toward the minimum.
- **Memory floor depends only on screen size**; recompute at startup, resize, and slider change. On-screen viewport work is never evicted. Slider default 512 MB, max 1024 MB; if the floor exceeds 1 GB, cap and enforce a screen-size bumper instead.
- **Animatables by timestamp + speed calculation**, not by frequent settings-update messages.

## Lookahead / prefetch (future work; rules when it returns)

- Pan prefetch: ~1× screen neighborhood from pan velocity. Focus-zoom prefetch: coin-sized patch from eye gaze.
- Prefetch cancels on view change; viewport work always wins; prefetch stays within the memory limit.
- The tile era proved the failure mode: lookahead that stores into a *separate* structure fragments the truth. Future lookahead extends the one-package remap discipline (virtues §3, §11).

## Two-stage escape / animated bailout

- Screen worker iterates to ‖z‖² > 4. An escaper stage continues escaped points to the animatable bailout radius. Changing bailout reruns the escaper only, never re-iterates the worker.
- **Why a separate bailout phase works (binding principal, 2026-08-11):** once a
  point has escaped, the Mandelbrot map is *superpower* escaping — magnitude
  grows so fast that continuing from R≈2 out to a large animated bailout takes
  **literally not more than ~10 iterations in almost all cases**
  (`bailout_max_additional_iterations` defaults to 10). The split is not “cheap
  for small radius, expensive for large radius.” Large animated radius does
  **not** create a heavy iterate grind on the escaper; the tail stays short by
  math, not by hoping the radius stays near 2. That is the point of the phase
  cut: workgroup owns membership / escape-to-4; shadergroup owns a tiny continue
  + paint so bailout anim never re-runs the worker.
- Escaper target: 60 Hz at 1080p worst case (every pixel at max escaper iterations, including the antenna).
- v0.0.9 does bailout at iterate time with an animated bailout that works (DAT watchlist); a split escaper stage is a GPU-era design decision, not a baseline requirement.
- **Single path (2026-08-11):** animated vs static bailout is the same `escape_frame` / `color` body — only numbers change (`shadergroup-virtues.md`). Guard that; do not fork paths.
- **2026-08-11 Criterion:** colorer is ~10× escaper wall time on filled home; problem child for the ~1.5×-pixel cliff (`shadergroup_fitness`).
- **GPU escape RCA (2026-08-11 night):** host data shipping dominates the GPU
  escaper (pack/upload + full-frame readback), not bailout arithmetic. See
  `shadergroup-virtues.md` § GPU shipping economics and the interview
  `interviews/2026-08-11-shade-gpu-residency.md`. Do **not** explain the slowdown
  as “needs a large radius to amortize.”
- **Colorer upgrades:** honest rewrite only — feature parity, same results, no simplifications, tests for every behavior. Settings **color gear** (OG vs GPU), manual like worker gear — not auto (`assembly-boundaries.md`).
- **Assemblies:** workgroup = answers; shadergroup = bailout tail + coloring; headgroup = present colors (+ stencils). Separation of concerns is what keeps the project workable. Study v0.0.9 for the hundreds of unlisted design decisions.
- **Stencil touch is O(1) per loop, never O(pixels) (2026-08-12).** `PointStencil`
  holds two `IntExp` values that grow large at design depth; per-pixel
  `stencil.clone()` (or equivalent) is a deep-zoom bomb. Hoist origin/space/width
  once per package, then use index arithmetic (`shadergroup-virtues.md`).

## Process

- Keep a live bug/todo stack (`issue-stack.md`); developer quotes archive under `docs/assistant/Trash/stale/less stale but still stale/grok-docs/he-said/`.
- When the developer answers a design question with a name + mechanism (not just a value), record it verbatim in `unit-design/decisions.md` — bare defaults miss binding detail.

## Series approximation (developer 2026-08-11 — binding intent)

- Product feel is video-game free motion: never frozen by iteration count, never
  chugged by backlog; tick rate is protected first if forced to choose, but
  v0.0.9 shows tick and throughput need not fight.
- SA win is deep zoom; always-on; skip is seat init and must be free (binary
  search); coeffs fused one-step-per-reference-iterate. See
  `r[cz.depth.series-approximation+1]`, `depth-design.md`, `D-SERIES-2`…`6`.
- Prior live SA sketch failed that bar; replaced 2026-08-11 by the fused /
  O(log N) production path.
