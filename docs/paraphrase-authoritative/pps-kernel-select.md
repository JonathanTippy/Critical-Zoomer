# PPS kernel selection (paraphrase)

Source: developer 2026-08-10 — gearbox must not assume GPU is fastest.

## Intent

Among **legal** compute kernels (gears), **measure** completed points per second
and **lock** the winner for the view. Do not prefer Naive GPU by default.

Host / numeric **type** stays automatic from depth admission.

## Legal set

| View | Candidates |
|---|---|
| Relative / precision wall (naive dishonest) | Perturbation only |
| Absolute (naive honest) | Naive CPU, Naive GPU (if device up), Perturbation |

Manual gear override still wins over probing.

## Probe

On a fresh view: sample each legal candidate for **one tick** (5 workshifts ≈
50ms), lock the highest PPS. **Re-open the race every 500ms** while the view is
still working — Naive GPU in particular tends to slow down as fill progresses,
so a one-shot lock is wrong, but 100ms re-races spent most of the time cycling
equally. Manual override still wins. Completions during probes still count as
real work.
