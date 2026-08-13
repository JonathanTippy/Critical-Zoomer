# E2E / headed Tracey rules (assistant-owned)

Headed visual corroboration is **not** a shell test suite. Normative product
text: `docs/authoritative/requirements.md` (including E2E Addendum). Unit and
integration verifies in Rust are the primary gate; the isolated Xvfb screenshot
check corroborates that truth reaches the display.

> **2026-08-08 scripts policy.** Do not reintroduce the former `e2e_*.sh` /
> harness-selftest / checked-in PNG zoo (`docs/assistant/Trash/scripts-sprawl-2026-08-08/`).
> Live headed bar is `scripts/screenshot_check.sh` + assistant image inspection
> (`docs/assistant/manual-testing.md`). `full_check.sh` / `coverage.sh` /
> `mutants.sh` are allowed wrappers, not a second e2e suite.

Defined in requirements.md: r[cz.e2e.harness-stack+1]

**Normative summary.** Input/screenshot stack (xvfb, fifo, capture/settle) is
available for the single screenshot check.

**Acceptance criteria.**
- [x] `scripts/screenshot_check.sh` (+ `screenshot_session.sh` / `screenshot_session_lib.sh`) can
  start the release binary under Xvfb, settle, and write a PNG under `/tmp`.
- ~~`scripts/harness_selftest.sh`~~ **Retired** (scripts policy 2026-08-08).

Defined in requirements.md: r[cz.e2e.controls-bindings+1]

**Normative summary.** Scroll (hover origin), Shift/Space (center origin), pan/drag bindings
match requirements; scroll-up zooms in.

**Acceptance criteria.**
- [ ] Prefer Rust / in-app path tests where possible; headed corroboration via
  screenshot check after control-affecting edits (assistant inspects PNG).
- ~~`scripts/e2e_controls.sh`~~ **Retired**.

Defined in requirements.md: r[cz.e2e.controls-no-jump+1]

**Normative summary.** Controls do not jump or do weird things: 2× per bump, no tick
backlog under 10 bumps/300ms, opposite Shift vs Space, hover-fixed scroll.

**Acceptance criteria.**
- [ ] Same as bindings: Rust first; optional headed PNG inspect after edits.
- ~~`e2e_controls.sh`~~ **Retired**.

Defined in requirements.md: r[cz.e2e.perf-home-fill+1]

**Normative summary.** Home screen fills within &lt;5s (oracle-quality settle), without
flat-black empty panes mid-wait.

**Acceptance criteria.**
- [ ] Measure with `cargo bench` / workgroup fitness where possible; headed
  screenshot check must not show empty/flat-black home after settle.
- ~~`scripts/e2e_performance.sh`~~ **Retired**.

r[cz.e2e.fill-first-tile-1s+1]

**Normative summary.** After startup/home, Mandelbrot structure must be visible
quickly (fitness ceiling — product target is much faster). Rule id keeps the
historical “tile” token; the live machine is a single view.

**Acceptance criteria.**
- [ ] Headed: settled PNG from `screenshot_check.sh` shows structure
  (assistant inspects; mean/stdev floors are helpers only).
- ~~`scripts/e2e_home_fill_fitness.sh`~~ **Retired**.

r[cz.e2e.fill-all-tiles-10s+1]

**Normative summary.** Home view must complete without dishonest empty panes
within a short settle window. Rule id keeps the historical “tiles” token; live
fills are one view with Dummy placeholders for unfinished seats.

**Acceptance criteria.**
- [ ] Headed screenshot check + craftsmanship/workgroup Rust fills; unfinished
  seats are Dummy placeholders (no tile NORES pack).
- ~~`scripts/e2e_home_fill_fitness.sh`~~ **Retired**.

Defined in requirements.md: r[cz.e2e.perf-zoom-simple+1]

**Normative summary.** Zooming into simpler areas stays apparently perfect: keeps pace
and full/oracle quality (no sustained low-res lag).

**Acceptance criteria.**
- [ ] Rust / bench where possible; optional headed PNG after zoom edits.
- ~~`e2e_performance.sh`~~ **Retired**.

Defined in requirements.md: r[cz.e2e.perf-zoom-hard+1]

**Normative summary.** Zooming into less-simple areas may go lower-res but must still
keep pace (continuity; not stalled empty panes).

**Acceptance criteria.**
- [ ] Same as simple zoom; hard-path unit pins remain in Rust craftsmanship tests.
- ~~`e2e_performance.sh`~~ **Retired**.

Defined in requirements.md: r[cz.e2e.visual-oracle+1]

**Normative summary.** No visual artifacts vs known-good oracles: compute oracles from
known-good code, prove with tests, compare live captures against them when needed.

**Acceptance criteria.**
- [ ] Oracle proving unit tests in Rust (v0.0.9 / restored code is the known-good).
- [ ] Headed: `screenshot_check.sh` + assistant Read of PNG
  (`docs/assistant/manual-testing.md`).
- ~~`scripts/e2e_visual.sh`~~ **Retired**.

Defined in requirements.md: r[cz.e2e.visual-assistant-review+1]

**Normative summary.** Assistant views screenshots as required corroboration (fallible;
never sole pass/fail).

**Acceptance criteria.**
- [x] After render-affecting edits, assistant Reads the PNG from the screenshot
  check and records what was seen (not only script statistics).
