# PPS grind loop prompt (dense tick)

Paste into `/loop` (prefer **fixed** interval, e.g. `/loop 15m …`). Do **not** use
one-shot `sleep`+`AGENT_LOOP_WAKE` re-arms. Never use approval-gated commands.
Pause with `.cursor/hooks/stop-agent-loops.sh` only (never raw `kill`).

## Prompt (copy as the loop body)

```
Grind naive-GPU home PPS toward ~160× CPU (FLOP-class aspiration), measured by steady_state_home_pps_gpu_vs_cpu_ratio (best-of-3). Each tick must do substantial work — not a measure-only or single-knob poke.

Per tick (mandatory order):
1. Measure current PPS ratio + note continuous-outputs / IPS / deep-cusp / F64-escalate pins.
2. Implement the largest plausible finish/sync/publish lever(s) toward 160× (prefer one coherent change that moves the needle; do two only if tightly related). Keep ≥1× PPS floor, continuous outputs (quiet gaps ≤~50ms / pin), deep-cusp never-stall, F64 gear escalate. Do not treat unfinished hard seats as “give up.”
3. Re-measure the PPS probe and the related steady_state_* pins you touch.
4. REGRESSION GATE (non-negotiable — do not skip or narrow):
   - Full test suite: cargo test --all-targets (release, taskset center-half + nice). Fix every failure you introduced or uncovered before checkpointing. “Only the tests I think are relevant” is forbidden.
   - All Criterion benches: cargo bench --bench workgroup_fitness and cargo bench --bench my_bench (same CPU niceness). Investigate ~≥20% regressions vs docs/assistant/benchmarks.md baselines; update baselines in the same commit only when the change intentionally moves them.
   - Tracey health: run `tracey validate` (or the repo’s equivalent link audit). Every r[impl …] must resolve; every rule’s pinned tests must exist. Fix broken links / orphan markers before checkpoint.
5. Checkpoint: automatic checkpoint <datetime> on the feature branch. Update issue-stack PPS note.
6. Do not re-arm wakes; the fixed /loop sleeper owns cadence. No request_smart_mode_approval. Cleanup only via .cursor/hooks/kill-test-zombies.sh.

Stop the loop only when best-of-3 PPS ratio ≥160× with green full suite + benches + tracey, or when blocked on a design gap that needs the developer (state the blocker clearly).
```

## Why denser than the old prompt

| Old | New |
|-----|-----|
| Measure → one lever → a few steady_state tests → lib only | Same grind, but **all-targets**, **both benches**, **tracey** every tick |
| Easy to ship host micro-opts while suite/benches rot | Regression gate is the tick exit condition |
| Implied “relevant tests” | Explicit ban on selective testing |
