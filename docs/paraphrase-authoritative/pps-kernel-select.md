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

On a fresh view (or when selection is cleared): run a short PPS sample on each
legal candidate, then stick with the highest PPS until Replace / honesty bump /
manual override. Completions during the probe still count as real work.
