# Critical-Zoomer — testing specification

NO EDITS TO THIS FILE BY AI ASSISTANTS WITHOUT EXPRESS APPROVAL

**TESTING_SPEC.md** — sister to [SPEC.md](SPEC.md).

| Document | Role |
| -------- | ---- |
| **[SPEC.md](SPEC.md)** | Product behavior only — what the app MUST do (`r[name]` + `// r[impl name]`) |
| **TESTING_SPEC.md** (this file) | All verification — how we test, prove, and triage quality |
| **[docs/tracey.md](docs/tracey.md)** | Tracey install, config, commands |
| **[docs/delivery/](docs/delivery/)** | Phased delivery (liminal, not normative) |

[SPEC.md](SPEC.md) MUST NOT contain testing methodology, mutants workflow, coverage recipes, or run commands. It MAY link here for verification (“verified per TESTING_SPEC”). This document MAY name related SPEC `r[name]` entries where a test exercises product behavior.

**No GitHub Actions CI** in this repository. Local proof: `cargo build --release`, tracey, mutation testing (§12), and the pipeline tests in §10.

---

## 1. Test the app, not a function

Pipeline tests MUST exercise the **whole production-shaped actor graph**—the same wiring the desktop app uses: window (headless stand-in), sampler, colorer, screen worker, work collector, point escaper.

**Entry point:** `run_pipeline_test` / `run_pipeline_test_with_after_stop` in `src/tests/harness.rs`. Test modules MUST NOT build or wire the graph themselves.

**Oracle:** Every pipeline test MUST wait on **`pipeline_e2e_ready`** (whole-graph colorer + collector + preview delivery) before asserting product behavior. Partial milestones without full-graph delivery are **not allowed**.

**Why:** Only a full graph catches actor ordering, channel backlog, remap drift, and idle-spin issues that unit tests miss.

**Enforced at compile time:** integration test lint (whole-graph rules).

---

## 2. Steady-state best practices

Tests that touch steady-state graphs MUST follow the vendored lessons:

- Poll an **oracle** until a condition holds or a **timeout** fails the test; use `std::thread::yield_now()` in poll loops—not fixed sleeps to “wait for settle.”
- Use `SteadyRunner::test_build()` with adequate stack size (`STACK_SIZE`); pair `graph.start()` with `request_shutdown` and `block_until_stopped` when a test owns a graph directly.
- Prefer production channel patterns (`testing_send_all`, harness injection APIs)—not puppet `simulated_behavior` for Critical-Zoomer actors.

**Deep reference:**

- `OTHER/steady-state-stack/lesson-on-testing.md`
- `OTHER/steady-state-stack/lesson-on-actor-testing.md`
- `OTHER/steady-state-stack/docs/testing.md`
- `OTHER/steady-state-performant/README.md` and `OTHER/steady-state-performant/lesson-02B-performant`

**Enforced at compile time:** steady-state test lint on `src/tests/**/*.rs` with `#[test]`.

**Note:** `steady_lint_fixtures/good/example_ok.rs` is a **lesson/demo shape** for the steady linter—not a template for `pipeline_*.rs` tests.

---

## 3. Production fidelity

When tests exist, they MUST match production runtime shape unless documented otherwise:

- **Resolution:** `DEFAULT_WINDOW_RES` (800×480), or `// test-resolution:` with explanation.
- **Settings:** `DEFAULT_COLORING_SCRIPT` and the same window settings path as production (`walk_check::production_settings`).
- **Graph:** production `actor::*::run` with `never_simulate(true)` in the harness.

**Enforced at compile time:** steady-state lint (resolution); integration lint (harness-only wiring).

---

## 4. Test fairness — the umpire

Busy host CPU MUST NOT make pipeline tests **unfair**. Slow from another process spiking CPU is not the same as slow from a broken pipeline.

The **umpire** (`src/tests/umpire.rs`, wired from `harness.rs`):

1. **Coast clear** — Wait until non-self CPU is low enough before a fair attempt (logged backoff with elapsed wait).
2. **Fair attempt** — Run graph body and polls; measure **testing time** (fair work only).
3. **T-bone retry** — If non-self CPU **spikes during** the attempt (including `after_stop`), **discard** the attempt and **retry** after coast clear. Never pass, never “inconclusive OK.”
4. **Poll timeout = failure** — Oracle timeout is a **failed test**, not a contention retry. A timeout on a correct product is a **rotten strawberry** risk if the cap is wrong; on a real bug it is a **strawberry**.

Umpire wait and retries are **excluded** from testing-time speed warnings (§6).

### Visibility is part of the contract

While working, the umpire MUST **log to stderr** (and on Unix, the tty when available) so a human watching `cargo test` is not left guessing during long runs:

- coast-clear checks and backoff (“still waiting …”, elapsed time),
- attempt monitoring, discard, and retry (peak non-self CPU, contention retry),
- fair attempt proceeding.

**Silence during real umpire work is a defect**—the umpire must not “work properly” invisibly. If the host is busy and the umpire is waiting or retrying, status lines MUST appear.

**Enforced at compile time:** umpire test lint (inconclusive-exit patterns); harness fairness wiring. **Visibility** is validated under load (§10.1) and in normal runs, not by lint alone.

---

## 5. Strawberries, rotten strawberries, and green beans

### Strawberries (good)

A **strawberry** is a test that **fails because it found the bug it was designed to catch**. That failure is success: the test did its job.

A strawberry MUST fail with a **complete, useful explanation**—expected vs observed, anchors, wire/collector context, `r[name]` where helpful—so you can tell whether the **product** or the **test** is wrong. Examples: `first_walk_mismatch`, walk checkpoint messages, assert text citing `r[pipeline.initial_frame_complete_within_1s]`.

Lint can require behavioral asserts and umpire-wrapped polls; it **cannot** prove the message is good enough—that is discipline and review.

### Rotten strawberries (bad failure)

A **rotten strawberry** is a test that **fails when it should not**—a false alarm. The product may be fine; the oracle, timing, fairness, or test logic is wrong. Rotten strawberries waste time and erode trust in the suite.

Reduce them with fair umpire runs (§4), full-graph oracles (§1), walk structure discards (§8), and review when a failure “does not make sense.”

### Green beans (evil pass)

A **green bean** is a test that **passes when it should fail**. Green beans are **always passing tests**. They let bugs ship. **Evil.**

**Green beans cannot be fully enforced.** No linter knows “this pass should have been a fail.” Culture, review, and the patterns below reduce them—not eliminate them.

**Anti-patterns** (some are blocked at compile time because they are easy to spot; that is **not** the same as detecting all green beans):

| Pattern | Why it is a green bean | Compile-time |
| ------- | ---------------------- | ------------ |
| Poll timeout treated as “maybe later” then pass | Bug not detected | Umpire lint |
| Inconclusive `Ok(())` on unfair load | Pass despite bad run | Umpire lint |
| “Soft target” timing violated but test passes | Pass despite broken timing | Umpire lint |
| Bare `poll_until` without umpire | Pass via weak exit | Umpire lint |
| Partial E2E / milestone-only pass | Pass while graph broken | Integration lint |
| All walk anchors discarded but test passes | Pass with nothing compared | Walk test logic |

When a test **passes**, ask: **would a strawberry have failed here if the bug were present?**

When a test **fails**, ask: **strawberry or rotten strawberry?**

---

## 6. Test speed

### Guideline

The pipeline lib suite SHOULD finish in **under 10 seconds** wall clock on a **recommended** fair run (§10), pass or fail. **Not a test failure**—hard caps would create green beans.

### Warnings

Fair **testing time** over guidance emits **stderr warnings** only. Treat as: speed checkup needed.

### Optimize before raising the budget

**Optimize test speed first**—shorter valid polls, less redundant work, harness oracles—not higher caps. Pipeline tests are **often much slower than they need to be**. Only after a minimal honest test should caps or guidance move.

A slow **strawberry** with a good failure message is still a strawberry. Warnings mean sharpen the test, not weaken asserts into green beans.

**Implementation:** warnings in `src/tests/umpire.rs` (`SUITE_TESTING_TIME_CAP`, `SUITE_TESTING_TIME_CAP_TARGET`).

---

## 7. Tracey (product + verification)

[tracey](https://tracey.bearcove.eu/) links specs to code. **Not** [wolfpld/tracy](https://github.com/wolfpld/tracy) the profiler.

Tooling: [docs/tracey.md](docs/tracey.md) · [`.config/tracey/config.styx`](.config/tracey/config.styx)

### Two specs

| Tracey spec name | Document | `r[name]` in | Code tag |
| ---------------- | -------- | ------------ | -------- |
| `critical-zoomer` | [SPEC.md](SPEC.md) | Product register | `// r[impl name]` |
| `critical-zoomer-testing` | **TESTING_SPEC.md** (appendix) | Verification register | `// r[verify name]` |

Both scan `src/tests/**/*.rs` for `verify`. Implementation scans `src/**/*.rs` for `impl`.

### Rules

1. **No numeric ids** (`REQ-001`, …) — dotted names only.
2. Every product `r[name]` in SPEC MUST have at least one `// r[impl name]` in `src/**/*.rs`.
3. Every `// r[verify name]` MUST be defined in **this appendix** or in SPEC (when verifying a product requirement by that name).
4. Tag the **smallest owning unit**.
5. Pipeline tests use `verify` per the appendix below.

### Local proof

```bash
tracey query validate
tracey query uncovered
tracey query untested
tracey web --open   # optional
```

Version bumps: when requirement text changes materially, use `r[name+N]` in the owning document and matching tags in code; run `tracey query stale`. See [tracey version tracking](https://github.com/bearcove/tracey#version-tracking).

---

## 8. Walk testing (remap / pixels don’t walk)

`pipeline_pixels_dont_walk_moment_after_zoom_in` (`pipeline_a_walk.rs`) checks preview vs collector after zoom at pinned post-anchor wire frames. Zoom targets **fractal structure** at 800×480, not window center or a flat void.

**Product behavior** ([SPEC.md](SPEC.md)): remap agreement under lagged delivery and zoom—`r[pixels_dont_walk.lagged_colorer_delivery]`, `r[pixels_dont_walk.random_viewport_fuzz]`, `r[pixels_dont_walk.window_zoom_ahead_remap]`. **Verification** here: `r[pixels_dont_walk.pipeline_after_zoom_in]`.

### Procedure (per anchor)

1. `pipeline_e2e_ready`; first complete home frame; walk trace on; **screen worker inhibit**.
2. **Five** zoom clicks (`WALK_ZOOM_CLICKS`).
3. **Initial estimate** on the wire → **structure gate** (below).
4. **Primary checkpoint:** **3** post-anchor wire frames + roll-in; `assert_walk_pipeline_checkpoint`.
5. **Secondary checkpoint:** **5** post-anchor wire frames; second checkpoint assert.

Anchors: `(200, 240)`, `(400, 120)`, `(400, 360)`.

### Structure gate — discard (not pass, not fail)

`walk_check::degenerate_snapshot_reason` (`MAX_DOMINANT_COLOR_FILL_PERCENT` = **60**) may reject a snapshot:

| Condition | Result |
| --------- | ------ |
| All comparable seats IDK | No RGB to walk — **discard** anchor. |
| **> 60%** of non-IDK seats are one RGB | Not enough structure — **discard** anchor. |

**Discard** = log reason (dominant color, fill %, wire/collector seq, zoom center), **try next anchor**. Neither pass nor fail for that point.

Same gate on estimate, primary, and secondary snapshots when applicable.

| Outcome | Test |
| ------- | ---- |
| ≥ one anchor **compared** and checkpoints OK | **Pass** |
| **Discard** at every anchor | **Fail** (strawberry-style report)—not pass; passing here would be a **green bean** |
| Structured anchor, checkpoint mismatch | **Fail** (strawberry) |

### Walk determinism — `scripts/walk_test_repeatability.sh`

Checks that the walk test’s **outcome class** is **stable** across eight `cargo test` command variants (with/without `--lib`, with/without `--test-threads`, etc.). Same bug must not flip pass/fail/discard class because of CLI shape alone.

Runs `N` repetitions per config (default `WALK_REPEAT_N=5`), prints **live stderr** (including umpire coast-clear lines), and **fails** if configs disagree or a config is internally unstable. Default CPU pin: `taskset -c 4-10,13,14` (override with `WALK_REPEAT_TASKSET`).

This is a **determinism** check, not the main pipeline proof.

---

## 9. CPU pin and actor count

**Six** pipeline actors: window, sampler, colorer, screen worker, work collector, escaper.

Fair runs SHOULD pin **at least six** CPUs. This repo uses **nine**:

```text
taskset -c 4-10,13,14
```

(CPUs 4–10 inclusive, plus 13 and 14.) PO and recommended commands share this pin. The mutants job uses a **different** pin (§12).

---

## 10. Running tests

Use **`--release`** for pipeline timing and behavior.

### PO habitual

```bash
taskset -c 4-10,13,14 cargo test --release
```

No `--lib`, no `--test-threads` — default Cargo parallelism may run multiple tests at once on the pin.

### Recommended (agents, fairness, speed tuning)

```bash
taskset -c 4-10,13,14 cargo test --release --lib -- --test-threads=1
```

**Single test:**

```bash
taskset -c 4-10,13,14 cargo test --release <test_fn_name> --lib -- --test-threads=1
```

| Piece | PO | Recommended |
| ----- | -- | ------------- |
| `taskset -c 4-10,13,14` | Yes | Yes |
| `--release` | Yes | Yes |
| `--lib` | No | Yes — `src/tests/` via library crate |
| `--test-threads=1` | No | Yes — one six-actor graph at a time |

**`--lib`:** tests attached to `critical_zoomer` library (`src/lib.rs` → `src/tests/`), not other targets.

**`--test-threads`:** serializes **test functions**, not actors inside one test. Use `1` on the recommended path so multiple pipeline tests do not each spawn a full graph on the same pin.

**Build:** `cargo build` / `cargo check` runs compile-time test linters (§13).

### Pipeline inventory

| Module | Test function | `r[verify]` |
| ------ | ------------- | ----------- |
| `pipeline_a_walk.rs` | `pipeline_pixels_dont_walk_moment_after_zoom_in` | `pixels_dont_walk.pipeline_after_zoom_in` |
| `pipeline_start.rs` | `pipeline_initial_frame_complete_within_1s` | `pipeline.initial_frame_complete_within_1s` |
| `pipeline_zoom.rs` | `pipeline_zoom_anchor_preview_color_stable_at_various_anchors` | `viewport.zoom_center_fixed_under_pipeline` |

**Support:** `harness.rs`, `umpire.rs`, `walk_check.rs`, `mod.rs`.

**Lint demos (source shape only):** `./scripts/steady_lint_demo.sh`, `./scripts/integration_lint_demo.sh`, `./scripts/umpire_lint_demo.sh`.

Re-grep `src/tests/pipeline_*.rs` if this table drifts.

---

## 10.1 Auxiliary test scripts

Manual / PO tools—not compile-time linters. They validate methodology.

### Walk determinism — `scripts/walk_test_repeatability.sh`

See §8. Compares outcome class at anchor `(200, 240)` across command variants. Optional env: `WALK_REPEAT_N`, `WALK_REPEAT_TASKSET`, `WALK_REPEAT_TIMEOUT_SEC`.

### Umpire fairness — `scripts/umpire_fairness_test.sh`

Runs pipeline test(s) under **hostile CPU** (a deliberate hog on CPUs outside the test pin). **Pass** means: tests still end **pass or fail** on correctness only, and the **umpire** emitted visible coast-clear / wait / discard / retry lines on stderr when it was working—not a silent hang or silent success under load.

**Under test:** `src/tests/umpire.rs` and harness wiring. The shell script starts the hog and invokes `cargo test`; the **umpire** explains itself (§4). If the machine is loaded and the umpire is waiting or retrying but produces **no** status lines, that is an **umpire failure**, not an acceptable slow run.

Separate from `umpire_lint_demo.sh` (static source checks only).

---

## 11. Line coverage (`cargo llvm-cov`)

[`cargo llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov) — LLVM line coverage.

```bash
cargo llvm-cov --release --lib
```

- `--text` — annotated source; **0** = uncovered for that run  
- `--html` / `--html --open` — browse by file  

Install: `cargo install cargo-llvm-cov`.

Matches `Cargo.toml` `[package.metadata.cargo-mutants]` `test_args` (`test --release --lib`).

---

## 12. Mutation testing (`cargo mutants`)

Mutation testing finds code **no test catches** when changed. Surviving mutants are **untrusted** until you add a test, delete dead logic, or skip with a **written reason** in a spec revision.

A full tree run is **thousands** of mutants and **slow**—background job, not a quick gate. It does **not** watch the repo; restart or use `--file` after meaningful changes.

With only a **small** pipeline suite (§10), most mutants remain **missed** until targeted tests exist. Good **strawberries** shrink that gap; **green beans** leave mutants **missed**.

### CPU affinity (`taskset`) — mutants job (separate from test pin)

Pin mutants off the interactive path (~¼ of the machine):

| Logical CPUs | Cores for mutants | Example `taskset -c` |
| ------------ | ----------------: | -------------------- |
| 4 (0–3)      | **1**             | `2` or `3`           |
| 8 (0–7)      | **2**             | `5,6` or `6,7`       |
| 16 (0–15)    | **2**             | `11,12`              |

**How to pick cores:** avoid core 0; narrow band in the middle (on 16 CPUs, **`11,12`**); consecutive IDs; adjust per machine.

**Normative on 16 CPUs:**

```bash
taskset -c 11,12 cargo mutants -- --release
```

### tmux `cz-mutants`

One maintained session; do not start a second full-tree job.

```bash
tmux new-session -d -s cz-mutants -c "$PWD" \
  'taskset -c 11,12 cargo mutants -- --release; echo done; exec bash -l'

tmux attach -t cz-mutants
```

Read `mutants.out/missed.txt` and `mutants.out/` as results grow.

### Run methodology — avoid re-running

Each invocation walks a **snapshot** list. Re-running the full tree mostly re-classifies old mutants.

| Do | Don’t |
| -- | ----- |
| Let `cz-mutants` run forward; read `mutants.out/` | Kill/restart full tree after every small edit |
| `cargo mutants --file path/you/changed.rs` (same `taskset`) | Second full-tree job in parallel |
| `--in-diff` for branch-sized deltas | Re-run thousands of mutants to confirm one new test—run that test (§10) |
| Restart when the long job died or config changed materially | Expect the job to notice saves |

**Goal:** maximize **new** outcomes per hour—**caught** (strawberry on the mutant) and **missed** (triage).

### Rules

1. **Caught** (test fails): good strawberry; remove from `mutants.presumed-caught.txt` if listed.
2. **Missed** (tests still pass): likely green-bean territory—add a test that would fail, remove/simplify code, or `#[mutants::skip]` with written reason in SPEC or TESTING_SPEC revision as appropriate.
3. Do not weaken the tool or skip modules without a documented revision.
4. **Avoid full-tree re-runs** when one tmux job + `--file` / `--in-diff` suffices.

### Presumed caught — **tested** path

[`mutants.presumed-caught.txt`](mutants.presumed-caught.txt) — the running `cz-mutants` job does not see edits until it catches the mutant for real.

When you **test** a missed mutant:

1. `cargo llvm-cov --release --lib` — site **uncovered**
2. Add test; run with **recommended** command (§10)
3. `cargo llvm-cov --release --lib` — site **covered**
4. Add mutant line to `mutants.presumed-caught.txt`
5. Remove line when `cz-mutants` **actually catches** that mutant

**Deleted** mutants: add to presumed caught when code is removed; remove when the job catches it.

---

## 13. Compile-time linters (checkers)

This document is **authoritative for intent**; linters **enforce** at `cargo build`. Keep them aligned when behavior changes.

| Linter | Intent | Demo |
| ------ | ------ | ---- |
| **Steady-state** (`build_support/steady_test_lint.rs`) | Lesson rules: sleeps, `test_build`, shutdown, tracey tags, resolution. | `./scripts/steady_lint_demo.sh` |
| **Integration** (`build_support/integration_test_lint.rs`) | Whole-graph only: `run_pipeline_test`, `pipeline_e2e_ready`, harness-only wiring. | `./scripts/integration_lint_demo.sh` |
| **Umpire** (`build_support/umpire_test_lint.rs`) | Forbids known *inconclusive* poll patterns; does not detect green beans or rotten strawberries in general. | `./scripts/umpire_lint_demo.sh` |

---

## Appendix — verification requirements register

Prefix: **`r`** (tracey spec `critical-zoomer-testing`). Use `// r[verify name]` above each `#[test]`.

---

r[pipeline.initial_frame_complete_within_1s]  
On cold pipeline start, first complete home frame (collector home + `pipeline_e2e_ready`) MUST be observable within **1 s** fair testing time from graph start.

*Exercises SPEC themes:* `r[pipeline.actors_idle_not_hot_spin]`, home delivery.

---

r[viewport.zoom_center_fixed_under_pipeline]  
At 800×480, zoom at off-center anchors MUST keep preview color stable at the anchor (structure, not window middle).

*Exercises SPEC themes:* `r[viewport.magnify_single_step_anchor_fixed]`, `r[viewport.magnify_chained_steps_anchor_fixed]`, `r[viewport.magnify_off_center_anchor_fixed]`.

---

r[pixels_dont_walk.pipeline_after_zoom_in]  
After five-click zoom at each walk anchor (screen worker inhibited), preview and collector MUST agree at post-anchor checkpoints (3 and 5 wire frames). Snapshots with all-IDK or **> 60%** single RGB on non-IDK seats MUST **discard** that anchor (try next); MUST NOT count as pass. Fail if no anchor compares or a structured anchor fails—with full explanation.

*Exercises SPEC themes:* `r[pixels_dont_walk.lagged_colorer_delivery]`, `r[pixels_dont_walk.random_viewport_fuzz]`, `r[pixels_dont_walk.window_zoom_ahead_remap]`.

---

r[testing.whole_graph_integration]  
Pipeline tests MUST use `run_pipeline_test` and `pipeline_e2e_ready`; MUST NOT use partial E2E, unit puppets, or graph wiring in `pipeline_*.rs`.

---

r[testing.umpire_fair_host]  
Pipeline tests MUST use umpire coast-clear, t-bone discard/retry, and hard failure on poll timeouts.

---

r[testing.umpire_visible_status]  
While waiting for coast clear or discarding a t-boned attempt, the umpire MUST emit human-readable status on stderr (backoff/elapsed, peak non-self CPU, retry). Silent correct behavior is not acceptable.

---

r[testing.strawberry_explanatory_failure]  
Failing verification tests SHOULD fail with a complete, human-readable explanation when the failure is a true strawberry. Not machine-enforced.

---

r[testing.rotten_strawberry_avoidance]  
Verification tests SHOULD NOT fail without a product bug or a clearly wrong oracle; false failures SHOULD be fixed by correcting the test or fairness, not by weakening product asserts into green beans.

---

r[testing.suite_speed_guidance_10s]  
Pipeline suite SHOULD finish in under **10 s** on a recommended §10 run; testing-time over guidance SHOULD warn only, never fail.

---

r[testing.optimize_before_raise_budget]  
Slow tests SHOULD be tightened before raising time caps or suite guidance.

---

r[testing.cpu_pin_nine_cores]  
Pipeline test commands SHOULD use `taskset -c 4-10,13,14` (nine CPUs, ≥ six actors) unless host topology changes.

---

r[testing.walk_structure_discard_60pct]  
Walk anchors MUST discard (not pass/fail) when `walk_check` reports all-IDK or > 60% dominant RGB fill; MUST try next anchor; MUST fail only if every anchor is discarded or a structured compare fails.

---

*End of TESTING_SPEC.md*
