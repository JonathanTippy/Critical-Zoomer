# Ghost grind loop prompt

Paste into `/loop` (prefer **fixed** interval). Do **not** use one-shot
`sleep`+`AGENT_LOOP_WAKE` re-arms. Never use approval-gated commands.
Pause with `.cursor/hooks/stop-agent-loops.sh` only.

**Ghost** = assistant misunderstanding that slipped into comments, docs,
names, or behavior after the project became assisted at v0.0.9. Developer
ideas that later evolved are *candidates*, but treat hand-coded /
design-via-iteration with deference. Dictator-phase (dev-by-spec) material
belongs in Trash: informative about how the developer was thinking, not
live law. Assistant vs developer commits: `automatic checkpoint` vs `WIP`
(and other human messages).

## Prompt (copy as the loop body)

```
Ghost grind. A ghost is an assistant misunderstanding in comments, docs, names, or behavior since v0.0.9. Evolved developer ideas are candidate ghosts — deference to hard-won hand-coded design; Trash spec-dictator docs are history, not law.

Per tick (mandatory order):
1. Estimate, best effort, what fraction of remaining ghosts this hunt has caught relative to all ghosts you still believe exist. If that guess is 100%, stop and say so. Do not claim 100% because the last swath was clean.
2. Choose one well-scoped swath (one directory, one mechanism, or one contradiction class). Name it. Do not boil the ocean.
3. Catch those ghosts: make the note/name/code match live truth and the v0.1 interview. Do not declare headed bugs fixed. Do not soften tests. Do not resurrect Trash spec as binding. Prefer types over comments. v0.0.9 golden mechanisms stay.
4. Hygiene: if the tick touched src/, benches, Cargo, or Tracey rules, the stop hook / `.cursor/hooks/hygiene-gate.sh` is the gate (release full suite + benches + tracey). Docs-only ticks skip the suite. Checkpoint: automatic checkpoint <datetime>.
5. Update docs/assistant/ghost-hunt-2026-08-12.md with the swath, what was caught, and the new % guess.
6. Do not re-arm wakes. No request_smart_mode_approval. Cleanup only via .cursor/hooks/kill-test-zombies.sh.
```
