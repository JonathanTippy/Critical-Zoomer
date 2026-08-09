# E2E / headed Tracey rules (assistant-owned)

Headed visual corroboration is **not** a shell test suite. Normative product
text: `docs/authoritative/requirements.md` (including E2E Addendum). Unit and
integration verifies in Rust are the primary gate; the isolated Xvfb screenshot
check corroborates that truth reaches the display.

> **2026-08-08 scripts policy.** `scripts/` may contain only the Xvfb screenshot
> entry point and its private ctl harness (`scripts/README.md`). The former
> `e2e_*.sh` / harness-selftest / checked-in PNG zoo was moved to
> `docs/assistant/Trash/scripts-sprawl-2026-08-08/`. Do not reintroduce it.
> Acceptance below that cited those scripts is **historical**; live headed bar is
> the screenshot check + assistant image inspection
> (`docs/assistant/manual-testing.md`).

r[cz.e2e.harness-stack+1]

**Normative summary.** Input/screenshot stack (xvfb, fifo, capture/settle) is
available for the single screenshot check.

**Acceptance criteria.**
- [x] `scripts/xvfb_screenshot_check.sh` (+ `cz_ctl.sh` / `cz_ctl_lib.sh`) can
  start the release binary under Xvfb, settle, and write a PNG under `/tmp`.
- ~~`scripts/harness_selftest.sh`~~ **Retired** (scripts policy 2026-08-08).

r[cz.e2e.controls-bindings+1]

**Normative summary.** Scroll (hover origin), Shift/Space (center origin), pan/drag bindings
match requirements; scroll-up zooms in.

**Acceptance criteria.**
- [ ] Prefer Rust / in-app path tests where possible; headed corroboration via
  screenshot check after control-affecting edits (assistant inspects PNG).
- ~~`scripts/e2e_controls.sh`~~ **Retired**.

r[cz.e2e.controls-no-jump+1]

**Normative summary.** Controls do not jump or do weird things: 2× per bump, no tick
backlog under 10 bumps/300ms, opposite Shift vs Space, hover-fixed scroll.

**Acceptance criteria.**
- [ ] Same as bindings: Rust first; optional headed PNG inspect after edits.
- ~~`e2e_controls.sh`~~ **Retired**.

r[cz.e2e.perf-home-fill+1]

**Normative summary.** Home screen fills within &lt;5s (oracle-quality settle), without
flat-black empty panes mid-wait.

**Acceptance criteria.**
- [ ] Measure with `cargo bench` / workgroup fitness where possible; headed
  screenshot check must not show empty/flat-black home after settle.
- ~~`scripts/e2e_performance.sh`~~ **Retired**.

r[cz.e2e.fill-first-tile-1s+1]

**Normative summary.** After startup/home, Mandelbrot structure must be visible
quickly (fitness ceiling — product target is much faster).

**Acceptance criteria.**
- [ ] Headed: settled PNG from `xvfb_screenshot_check.sh` shows structure
  (assistant inspects; mean/stdev floors are helpers only).
- ~~`scripts/e2e_home_fill_fitness.sh`~~ **Retired**.

r[cz.e2e.fill-all-tiles-10s+1]

**Normative summary.** Home view must complete without dishonest empty panes
within a short settle window.

**Acceptance criteria.**
- [ ] Headed screenshot check + craftsmanship/workgroup Rust fills; v0.0.9 has no
  tile NORES pack — unfinished seats are Dummy placeholders.
- ~~`scripts/e2e_home_fill_fitness.sh`~~ **Retired**.

r[cz.e2e.perf-zoom-simple+1]

**Normative summary.** Zooming into simpler areas stays apparently perfect: keeps pace
and full/oracle quality (no sustained low-res lag).

**Acceptance criteria.**
- [ ] Rust / bench where possible; optional headed PNG after zoom edits.
- ~~`e2e_performance.sh`~~ **Retired**.

r[cz.e2e.perf-zoom-hard+1]

**Normative summary.** Zooming into less-simple areas may go lower-res but must still
keep pace (continuity; not stalled empty panes).

**Acceptance criteria.**
- [ ] Same as simple zoom; hard-path unit pins remain in Rust craftsmanship tests.
- ~~`e2e_performance.sh`~~ **Retired**.

r[cz.e2e.visual-oracle+1]

**Normative summary.** No visual artifacts vs known-good oracles: compute oracles from
known-good code, prove with tests, compare live captures against them when needed.

**Acceptance criteria.**
- [ ] Oracle proving unit tests in Rust (v0.0.9 / restored code is the known-good).
- [ ] Headed: `xvfb_screenshot_check.sh` + assistant Read of PNG
  (`docs/assistant/manual-testing.md`).
- ~~`scripts/e2e_visual.sh`~~ **Retired**.

r[cz.e2e.visual-assistant-review+1]

**Normative summary.** Assistant views screenshots as required corroboration (fallible;
never sole pass/fail).

**Acceptance criteria.**
- [x] After render-affecting edits, assistant Reads the PNG from the screenshot
  check and records what was seen (not only script statistics).
