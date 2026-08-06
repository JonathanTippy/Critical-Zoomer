# Collected wisdom (surviving content from stale docs)

Harvested 2026-08-06 during the v0.0.9 revert cleanup. Each entry: the rule, its source, and whether v0.0.9 already honors it. Anything contradicted by the golden design was dropped; contradictions are resolved in favor of `docs/design/workgroup-virtues.md`.

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
- Escaper target: 60 Hz at 1080p worst case (every pixel at max escaper iterations, including the antenna).
- v0.0.9 does bailout at iterate time with an animated bailout that works (DAT watchlist); a split escaper stage is a GPU-era design decision, not a baseline requirement.

## Process

- Keep a live bug/todo stack (`issue-stack.md`); developer quotes archive under `docs/stale/.../he-said/`.
- When the developer answers a design question with a name + mechanism (not just a value), record it verbatim in `unit-design/decisions.md` — bare defaults miss binding detail.
