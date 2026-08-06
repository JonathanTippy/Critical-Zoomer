# E2E Tracey rules (assistant-owned headed contracts)

Atomic rules for headed interaction under the frozen `cz_ctl` harness.
Normative product text: `docs/requirements.md` (including E2E Addendum).
Unit/integration verifies alone do not satisfy these ids.

r[cz.e2e.harness-stack+1]

**Normative summary.** Input/screenshot stack (xvfb, fifo, xdotool, import/compare)
is proven and frozen before product e2e scripts rely on it.

**Acceptance criteria.**
- [x] `scripts/harness_selftest.sh` covers lifecycle, capture, input delivery, settle, isolation
  (`harness_selftest.sh`; frozen command surface on `cz_ctl_lib.sh`).

r[cz.e2e.controls-bindings+1]

**Normative summary.** Scroll (hover origin), Shift/Space (center origin), pan/drag bindings
match requirements; scroll-up zooms in.

**Acceptance criteria.**
- [ ] Headed verifies for scroll / key zoom / pan bindings
  (`scripts/e2e_controls.sh`: shift-zoomin, space-zoomout, scroll10, arrow-pan). CI does not gate headed e2e yet.

r[cz.e2e.controls-no-jump+1]

**Normative summary.** Controls do not jump or do weird things: 2× per bump, no tick
backlog under 10 bumps/300ms, opposite Shift vs Space, hover-fixed scroll.

**Acceptance criteria.**
- [ ] Headed property-style verifies for no-jump / tick sustain / opposite zoom
  (`e2e_controls.sh`: zoomout nearer home than zoomin; scroll10 dispatch; nonzero RMSE steps).

r[cz.e2e.perf-home-fill+1]

**Normative summary.** Home screen fills within &lt;5s (oracle-quality settle), without
flat-black empty panes mid-wait.

**Acceptance criteria.**
- [ ] Timed home fill headed verifies (time, non-black mid, settled quality)
  (`scripts/e2e_performance.sh`).

r[cz.e2e.fill-first-tile-1s+1]

**Normative summary.** After startup/home, at least one tile of Mandelbrot structure
must be visible within **1s** (fitness ceiling — product target is much faster).

**Acceptance criteria.**
- [ ] Headed fitness poll: frame stdev ≥3000 within 1s of `home`
  (`scripts/e2e_home_fill_fitness.sh`).
- [x] Workgroup unit: all whole tiles emitted within 10s on 800×480 home
  (`tile_session_tests::home_800x480_emits_all_whole_tiles_within_10s`).

r[cz.e2e.fill-all-tiles-10s+1]

**Normative summary.** Home view must complete **all** screen tiles (zero NORES-grey
holes in the fitness crop) within **10s** of startup (fitness ceiling).

**Acceptance criteria.**
- [ ] Headed fitness poll: `e2e_count_gray_holes` ≤0 within 10s of `home`
  (`scripts/e2e_home_fill_fitness.sh`).
- [x] Workgroup unit: `headgroup_completed_whole_tiles` ≥ tile count at 100% host fill
  (`tile_session_tests::home_800x480_emits_all_whole_tiles_within_10s`).

r[cz.e2e.perf-zoom-simple+1]

**Normative summary.** Zooming into simpler areas stays apparently perfect: keeps pace
and full/oracle quality (no sustained low-res lag).

**Acceptance criteria.**
- [ ] Simple-region zoom headed verifies (`e2e_performance.sh` exterior goto + zoomin).

r[cz.e2e.perf-zoom-hard+1]

**Normative summary.** Zooming into less-simple areas may go lower-res but must still
keep pace (continuity; not stalled empty panes).

**Acceptance criteria.**
- [ ] Hard-region (seahorse) zoom/goto headed verifies (`e2e_performance.sh`).

r[cz.e2e.visual-oracle+1]

**Normative summary.** No visual artifacts vs known-good oracles: compute oracles from
known-good code, prove with tests, compare live captures/metrics against them.

**Acceptance criteria.**
- [x] Oracle proving unit tests (`src/e2e_oracle.rs`; CI)
- [ ] Headed compares (`scripts/e2e_visual.sh`) — not CI-gated yet

r[cz.e2e.visual-assistant-review+1]

**Normative summary.** Assistant views screenshots as required corroboration (fallible;
never sole pass/fail).

**Acceptance criteria.**
- [x] Review notes logged for visual-relevant capture sets
  (`e2e_visual.sh` stages `/tmp/cz_e2e_visual_review`; assistant Read of home/deep/pan PNGs).
