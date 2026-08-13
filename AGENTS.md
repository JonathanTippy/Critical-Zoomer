# Critical-Zoomer — working agreement for agents

This repo's value is a small set of *proven* mechanisms, restored and verified.
Do not redesign them by accident. Read these before editing.

## Authoritative docs (read in this order when touching the workgroup)

1. `docs/assistant/design/workgroup-virtues.md` — the normative design and *why*
   each mechanism exists.
2. `docs/assistant/tracey/craftsmanship-rules.md` — the binding rules
   (`r[cz.craft.*]`), one per golden mechanism, each with acceptance criteria and
   pinned tests.
3. `docs/assistant/issue-stack.md` — live bugs, design gaps, and the standing
   rule that new work must not re-break the v0.0.9 invariants.

## The invariants (summary — full text in the rules file)

`.cursor/rules/critical-zoomer-invariants.mdc` is the always-on version. In
brief: no unbounded calls (use `BoutCap`), one live target (the `LiveTarget`
struct), whole-snapshot publishes, pivot two-message order, provisional never
final (the `Delivery` enum), small interruptible bouts.

## Enforcement layers

Five layers, cheapest-first:

1. **Types** — `BoutCap` (no unbounded call), `Delivery` (provisional cannot set
   `delivered`), `push_delivery` (buffer slot + flag atomically, `#[must_use]`),
   `LiveTarget` (one live target structural). Compile-time; cannot regress.
2. **The pre-edit hook** — `.cursor/hooks.json` runs
   `.cursor/hooks/workgroup-rules.sh` on every `Write`/`StrReplace` into
   `screen_worker/` or `colorer/`; it injects that file's rule summaries as agent
   context at the moment of the edit. Fails open.
3. **Test leftover reaper** — `.cursor/hooks/kill-test-zombies.sh` runs before/after
   shell commands matching `cargo test|cargo bench|xvfb_screenshot_check`, and on
   agent `stop`. Reaps repo `target/` app/bench binaries and `/tmp/cz_*` Xvfb
   sessions only (never headed `/usr/bin` or Cursor sandboxes). Log:
   `/tmp/cz_zombie_kill.log`. Fails open.
   **Never** use raw `kill`/`pkill`/`killall` (including `kill <pid>` after
   `pgrep`) — that trips safety prompts. `.cursor/hooks/guard-raw-kill.sh`
   blocks those commands and runs the reaper instead. Manual sweep:
   `.cursor/hooks/kill-test-zombies.sh` only. Always-on rule:
   `.cursor/rules/test-zombie-reaper.mdc`.
4. **No approval during loops/plans** — Auto-review / approval cards halt the
   agent until the developer returns. Never run approval-gated commands, never
   set `request_smart_mode_approval`, and never retry a block "with approval."
   Always-on rule: `.cursor/rules/no-approval-during-loops.mdc`.
5. **Hygiene gate** — `.cursor/hooks/hygiene-gate.sh` (bacon check + full
   `cargo test --all-targets` + all Criterion benches + `tracey query validate`
   and status). `stop` hook `.cursor/hooks/hygiene-on-stop.sh` runs it when the
   turn touched code/benches/tracey and follow-ups the agent if red. Log:
   `/tmp/cz_hygiene.log`. Skip with `CZ_HYGIENE=0`. Fail-closed Tracey (no
   soft-skip). `scripts/` is **only** for Xvfb screenshot check.

## Two rules that prevent most regressions

- **Prefer a type over a comment** for any invariant. If the bad state can be
  made unrepresentable, make it unrepresentable (`BoutCap` is the pattern).
- **The change protocol:** a deliberate redesign updates the tracey rule, the
  virtues doc, and the pinned test together, in one change. A failing rule is a
  regression until the developer says otherwise.

## Verify

Run the **full** test suite after workgroup/colorer edits — not a hand-picked
subset. Prefer `cargo test --all-targets` (and release when performance pins
matter), or `.cursor/hooks/hygiene-gate.sh` which is the lock-step gate (check +
full tests + all three Criterion benches + Tracey). Keep tracey links intact
(every `r[impl ...]` resolves to a rule; every rule's tests exist); run
`tracey query validate` when docs/markers move (there is no `tracey validate`
CLI). Prefer `cargo test` and `cargo bench` over shell. After workgroup/headgroup
perf-affecting edits, run **all** Criterion benches (`workgroup_fitness`,
`shadergroup_fitness`, `my_bench`) and compare to `docs/assistant/benchmarks.md`
(~20% regression bar). `scripts/` is
**only** for the isolated Xvfb screenshot check — see `scripts/README.md`; do not
add new e2e shell suites or check in PNGs there.

Dense PPS grind loop prompt (fixed `/loop`, full regression gate each tick):
`docs/assistant/pps-grind-loop-prompt.md`. Pause loops with
`.cursor/hooks/stop-agent-loops.sh` only.

Quality grind / no soft-skip: `docs/assistant/quality-doctrine.md` and
`docs/assistant/quality-slip-review.md`. Never `#[ignore]` / soft-floor a
failing invariant; Criterion ≥20% slip is FIX NOW.

Headgroup/shadergroup edits outside location bar + HUD need an explicit note
(`docs/assistant/headgroup-charter.md`).

**Steady-state Rust integration tests are the lifeblood of testing** (see
`docs/assistant/testing.md`). When changing scheduling, naive GPU, or HUD
telemetry, extend `steady_state_*` tests in `craftsmanship_tests.rs` so IPS and
completions are proven through screen-worker and workgroup chains — not only
in micro probes or Criterion.

For any change that can affect rendered output, also follow
`docs/assistant/manual-testing.md`. This is assistant-owned work: build the
current release, run `scripts/xvfb_screenshot_check.sh`, and inspect the PNG
directly. Never capture the developer's desktop and never hand the procedure to
the developer.

## Same-workspace checkpoint commits (recoverable delegated work)

The developer keeps control of permanent / main history. Agents may still create
**checkpoint commits on the current non-main feature branch** in this same
workspace so interruptions and whole-file mishaps are recoverable:

1. Work on a non-main branch in the current workspace (visible; no parallel
   worktree required for ordinary delegated work).
2. Before risky delegated edits, commit a clearly labeled recovery checkpoint
   (`checkpoint: ...`) even if some gates are still red.
3. One foreground agent at a time; no parent+child concurrent edits of the same
   tree. Prefer narrow diffs over whole-file replacements; never stash ad-hoc
   copies inside the repo as the recovery mechanism.
4. After each coherent green unit (focused tests pass), create another
   checkpoint commit. Stop if a diff deletes unrelated tests or exceeds a large
   unexplained deletion threshold.
5. Never merge, rebase, squash, push-force, or otherwise alter main / permanent
   history unless the developer explicitly directs it. Present commits for
   review; the developer chooses squash, cherry-pick, merge, or discard.
