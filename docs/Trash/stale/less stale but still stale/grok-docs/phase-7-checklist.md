# Phase 7 — polish checklist

Non-authoritative ship checklist distilled from `docs/requirements.md` and `docs/expectations.md`. Batch G locked defaults in `he-said/batch-G.md`.

## Must-pass manual script (Batch G)

- [ ] Home settle capture looks tenacious (no unknown-as-black antenna gaps)
- [ ] Cardioid browse: escape time + in-filaments visible; no whole-screen black flood before edge walk
- [ ] One deep seahorse-style location: still responsive; low-res interim OK; no max-iter wall

## Seamless

- [ ] No max-iteration setting / wall
- [ ] No perturbation toggle; references chosen in background
- [ ] No GPU toggle
- [ ] Pan/zoom keep prior work visible (big pixels OK); no flat black panes when low-res exists

## Deep / Fast

- [ ] Scroll tick = 2×; input never waits on compute
- [ ] Sustain ~10 ticks / 300ms gesture feel (refinement may lag)
- [ ] Depth path not blocked on naive-only (perturbation follow-up tracked separately)

## Tenacious / Calibrated

- [ ] Unknown seats use NORES (Outside escape-1), not set-black — B-TEN-1
- [ ] Regular iterate never emits false periods (`period == 0` until resolve) — B-PER-2 / D-PER-1
- [ ] Exterior `small_time == 0` paints; STE ridge does not spur on zeros — B-STE-1 partial

## Hoarding / Display

- [ ] Cosmetic settings recolor from hoard without recompute
- [ ] Tile hoard not cleared on pan
- [ ] Coord field + apply/goto; settings window does not grey main view — B-DISP-1 partial

## System policy (Batch G.1)

- [ ] Memory budget constant hardcoded to requirements floor (512MB reference budget already); slider UI deferred

## Out of this ship (Batch G.3)

- N-perturbation telescoping

## Capture

- Target: `/tmp/cz_ctl_capture/phase7_home.png` via `cz_ctl` home settle
