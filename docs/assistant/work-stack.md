# Assistant work stack (live)

Operational queue for **what the assistant runs next**. Product bugs and design
gaps stay in `issue-stack.md`. Hunt tick tables stay in each hunt log.

**Top = now.** Update this file when focus shifts; one line per item is enough.

## Active

| # | Work | Status | Doc / hook |
|---|---|---|---|
| 1 | **Mutant grind** — house files (`utils`, `range`, `floatexp`), then deliberate expansion | active | `mutant-hunt-2026-08-14.md`, loop `mutant-grind-loop-prompt.md` |
| 2 | **Groove tier on park** — tracey → unit → integration → e2e → screenshot | standing | `groove_check_on_stop.sh`, `scripts/zoomer_groove_check.sh` |
| 3 | **Full check on park** — cadence + benches + tracey (skips groove) | standing | `full_check_on_stop.sh` (`CZ_FULL_CHECK_SKIP_GROOVE=1`) |

**Mutant baseline:** 4307 missed (`mutants.out/missed.txt`, 2026-08-14 31h run).

**Loops:** **all paused/cancelled** (developer 2026-08-14) — `stop-agent-loops.sh` stopped≈1. Do not re-arm until directed.

## Paused

| Work | Why paused | Resume when |
|---|---|---|
| **All agent loops** (`/loop`, `AGENT_LOOP_*`) | Developer 2026-08-14: pause & cancel | Developer says re-arm |
| Ghost hunt (~76% caught guess) | Developer 2026-08-14: mutant hunt first | `mutant-hunt` ends or developer redirects — `ghost-hunt-2026-08-12.md` |
| Ghost grind loop (`AGENT_LOOP_TICK_ghost_grind`) | Ghost hunt paused | Do not re-arm until ghost hunt resumes |
| Mutant grind loop (`AGENT_LOOP_TICK_mutant_grind`) | Loops cancelled | Developer says re-arm `/loop` |

## Wake (no park)

- **Loops cancelled** — no `/loop` or agent sleepers until developer re-arms.
- **Code-edit park:** groove hook → full check hook (agent does not start either manually).
- **Stop loops:** `.cursor/hooks/stop-agent-loops.sh` only (never raw kill).

## Recently closed (stack hygiene)

| Item | Outcome |
|---|---|
| Ghost grind loop sleeper | Aborted 2026-08-14; ghost hunt paused — correct, not re-armed |
| `utils.rs` `signed_shift` `input < 0` | Pin `signed_shift(0, -40) == 0`; `assert_ne!(0,-40), -1)` | tick 1 |
| `range.rs` min/max `<`/`<=` on ties | **Equivalent** — ties pick same branch; `nan_min_max` covers NaN | classified |
| `floatexp.rs` `shift > 54` vs `>=` at shift 54 | **Equivalent** — f64 cannot resolve gap; drop matches no-op | classified |
