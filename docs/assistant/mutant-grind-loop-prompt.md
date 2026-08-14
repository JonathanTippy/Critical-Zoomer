# Mutant grind loop prompt

Paste into `/loop` (prefer **fixed** interval). Pause with
`.cursor/hooks/stop-agent-loops.sh` only. Never approval-gated commands.

Baseline: `mutants.out/missed.txt` (4307 after 2026-08-14 full lib run).
Re-verify with `taskset -c 3-8 scripts/mutants.sh <file>` on the swath you touched.

## Prompt (copy as the loop body)

```
Mutant grind. Ghost hunt is PAUSED (developer 2026-08-14). Hunt missed mutants from mutants.out.

Per tick (mandatory order):
1. Count lines in mutants.out/missed.txt (baseline reference in docs/assistant/mutant-hunt-2026-08-14.md).
2. Pick one well-scoped swath: one file or one function cluster from missed.txt. Prefer house pins (range/utils/floatexp/constants) before workgroup/shade charter files.
3. Kill or classify: add `mutant_kill` unit test, or document equivalent/unviable survivor with one sentence in the hunt log. No soft-skip. Do not weaken production code to please mutants.
4. Re-run `taskset -c 3-8 scripts/mutants.sh <file>` when you added pins (or subset with --file).
5. If the tick touched src/, run focused `cargo test` on the new pin; full_check is not required every tick.
6. Update docs/assistant/mutant-hunt-2026-08-14.md with swath, outcome, missed count delta if known.
7. Checkpoint: automatic checkpoint <datetime>. No re-arm wakes. Cleanup via .cursor/hooks/kill-test-zombies.sh only.
```
