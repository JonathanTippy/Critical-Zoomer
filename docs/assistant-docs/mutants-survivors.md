# Mutants campaign continuity (assistant-owned)

**Do not kill or restart** the live scoped campaign unless the developer asks.
Artifacts live in `mutants.out/`; lock in `mutants.out/lock.json`.

## Completed: `range.rs` only (2026-08-01)

- **Command:** `scripts/mutants_core.sh src/range.rs` (log: `/tmp/cz_mutants_range.log`)
- **Result:** 178 mutants in ~31m — **109 caught**, **43 missed**, **26 unviable** (~72% kill on viable)
- **MISSED clusters:** `min`/`max` boundaries; `is_agnostic`; `can_eq`/`must_eq`/`can_ne`/`must_ne`/`can_lt`/`must_lt`/`can_gt`/`must_gt`; `guess_biased`; `get_uuid`
- **Next:** triage MISSED (justify or add tests); then run `src/intexp.rs` scoped pass

## Prior live campaign (superseded for range.rs)

- **Process:** `cargo-mutants` scoped to `src/range.rs` + `src/intexp.rs` (~430 mutants)
- **Tmux:** socket `/tmp/tmux-1000/default`, window `0` — leave this pane alone if still attached
- **Command shape:** `scripts/mutants_core.sh src/range.rs src/intexp.rs`
- **Excludes:** `src/main.rs`, `window/mod.rs`, `settings.rs`
- **Test filter:** `--lib` with skips for `home_800x480`, `gpu_ips`, `standards_perf::`, `foveation_balance`
- **Builds:** mutants uses `/tmp/cargo-mutants-…tmp`; do not clear `mutants.out/` or steal `lock.json`
- **How to watch later:**
  - `tmux -S /tmp/tmux-1000/default attach -t 0`
  - or `tmux -S /tmp/tmux-1000/default capture-pane -t 0 -p -S -40`
  - `wc -l mutants.out/caught.txt mutants.out/missed.txt`
  - `tail -f /tmp/mutants_core3.log` (if that tee is still attached)

## Continuity rules

1. Do not block other V2V work on this campaign finishing.
2. Avoid competing heavy `cargo` jobs on the same center-half CPUs when possible; prefer `nice` and never send keys to window `0`.
3. Prefer not to edit `src/range.rs` / `src/intexp.rs` until this run ends (keeps MISSED/caught meaningful vs current tree).
4. Triage MISSED only after the run completes (or the developer interrupts it).

## Strengthened tests (already landed for this campaign)

NaN `min`/`max` survivors addressed by:
- `range::tests::min_propagates_nan_ignorance`
- `range::tests::min_returns_lesser_finite`
- `range::tests::max_propagates_nan_ignorance`

## MISSED to triage (`range.rs` run)

- `min`/`max` boundary ops (`<` vs `<=`, `>` vs `>=`, `==` vs `!=`) — likely need equal-finite and NaN-edge cases
- `is_agnostic` → always `true`
- `can_*` / `must_*` comparison helpers — broad survivor surface; many `&&`↔`||` and forced bool returns
- `guess_biased` — `>`/`>=` and `<`/`<=` on bounds
- `get_uuid` → `0`

## Review policy when results land

Any MISSED must be listed here with justification (equivalent / untestable) or killed by a new test. Do not claim Mutants() B/S from an unfinished run.
