# How quality slipped after v0.0.9 (operational review)

Scope: `e6a0560` (v0.0.9) → `quality/squeaky-clean` tip. Evidence from git
log, issue-stack, V2V re-score (2026-08-09), and DAT history.

## What went well

- Virtues doc + craftsmanship Tracey rules pinned golden mechanisms.
- Type-level invariants (`BoutCap`, `Delivery`, `LiveTarget`) prevent whole
  classes of pivot/publish bugs.
- Steady-state Rust tests for GPU queues / no-mop / IPS plumbing landed.

## How the bar softened

1. **Soft floors and ignores.** GPU PPS asserted `ratio >= 0.5` with WARN
   below 1×; series package tests `#[ignore]`; Tracey series verification
   marked “parked / ignored.” That is the soft-skip pattern that failed DAT.
2. **Broken delivery tools.** `bacon.toml` still called `scripts/coverage.sh`
   and `scripts/mutants_core.sh` after those scripts were moved to Trash —
   coverage/mutants became ceremonial.
3. **Feature grind over baseline.** Naive GPU PPS and depth gears absorbed
   most agent attention; CPU DirectKernel “feel” was assumed untouched while
   `workshift.rs` accrued dozens of commits (derivative, period pipeline,
   gear policy, trial pert, agent logs).
4. **Headgroup/shadergroup drift.** Post-revert diffs show large colorer /
   escaper / window changes beyond location+HUD — charter violated unless
   each change is design-justified.
5. **Tile-era docs still adjacent.** `docs/assistant/design/Stale/` kept
   tile designs next to live design; assistants re-read them as if current.
6. **Benchmarks as history dump.** Many **REJECTED** rows without a hard
   FIX NOW rule made slow pert paths look like “documented experiments”
   rather than release blockers.

## Operational suggestions

| Practice | Why |
|---|---|
| Quality doctrine is binding | `quality-doctrine.md` — no ignore/soften |
| FIX NOW on Criterion | Stop grind on ≥20% accepted-baseline regression |
| Oracle gear for deep parity | Never use f64 DirectKernel as deep membership oracle |
| Trash tile Stale out of live design/ | Reduce accidental reimplementation |
| Restore coverage + mutants scripts | Bacon must fail closed or run real jobs |
| Charter check on colorer/headgroup | Diff vs e6a0560 each quality tick |
| Checkpoint only after hard green | No “ignore to land” |

## V2V note

Re-score 2026-08-09 put the tree at **C (~6.1)** — down from historical tile-era
**B (7.0)** — primarily DeliveryPipeline / Coverage / Mutants / soft bars, not
lack of craft pins. Squeaky-clean means raising those dimensions without
softening asserts.
