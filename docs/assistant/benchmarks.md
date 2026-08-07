# Benchmarks (first-class performance tracking)

Fitness numbers live in **benchmarks**, not tests. Tests carry only smoke-level timing asserts;
anything that answers "how fast is it" belongs here.

## Suite

`benches/workgroup_fitness.rs` (plus the older `benches/my_bench.rs` headgroup microbenches).

| bench | metric | guards |
|---|---|---|
| `time_to_first_publish` | fresh home-view context → first non-empty publish | `cz.perf.play-minimize+1`, `cz.perf.play-8bump-100ms+1` |
| `time_to_full_frame` | home view 854×480, wall-clock to every seat delivered | `cz.perf.home-100tps+1` |
| `full_stack_ips` (printed by the full-frame bench) | total_iterations / wall time, real workgroup loop, scheduling included | `cz.perf.min-300m-ips-cpu+2` (method; the 300M number is pending re-derivation) |

Run (house rule — nice, center-half CPUs):

```bash
taskset -c 3-8 nice -n 15 cargo bench --bench workgroup_fitness
```

## Regression guard

Benchmarks vary run to run; this is not an exact science and that's fine.

- After any code edit touching the workgroup or headgroup, rerun the suite and compare against
  the baseline below **by eye**.
- A regression beyond ~20% must be investigated before the change is accepted. Noise is normal;
  trends matter, single runs don't.
- When a change intentionally alters performance, update the baseline **in the same commit** as
  the change.

## Baseline

| metric | value | date | commit | machine |
|---|---|---|---|---|
| time_to_first_publish | ~68 ms (median 68.3) | 2026-08-06 | post-revert working tree | linux 6.17, 12-core, nice'd CPUs 3-8 |
| time_to_full_frame | ~12.43 s | 2026-08-06 | post-revert working tree | same |
| full_stack_ips | ~8.3e5 (10,302,563 iterations, ~1130 shifts) | 2026-08-06 | post-revert working tree | same |
| time_to_first_publish (pre-period-pipeline) | 65.58 ms (65.21–65.97 ms) | 2026-08-06 | pre-change working tree | same |
| time_to_full_frame (pre-period-pipeline) | 12.30 s (12.23–12.37 s) | 2026-08-06 | pre-change working tree | same |
| full_stack_ips (pre-period-pipeline) | ~8.42e5 (10,302,563 iterations, 1106–1137 shifts) | 2026-08-06 | pre-change working tree | same |
| time_to_first_publish (derivative pipeline) | 66.27 ms (65.88–66.68 ms; +1.0%, noise) | 2026-08-06 | post-change working tree | same |
| time_to_full_frame (derivative pipeline) | 234.22 ms (231.81–236.96 ms; −98.1%) | 2026-08-06 | post-change working tree | same |
| full_stack_ips (derivative pipeline) | ~5.8e7 (10,302,563 iterations, 17–18 shifts) | 2026-08-06 | post-change working tree | same |
| time_to_first_publish (period-correctness fix) | 65.50 ms (65.23–65.77 ms) | 2026-08-06 | post-fix working tree | same |
| time_to_full_frame (period-correctness fix) | 293.57 ms (291.12–295.86 ms; −97.6% vs pre-pipeline, +25% vs first pipeline cut) | 2026-08-06 | post-fix working tree | same |

Pre-change test baseline: `cargo test` ran 39 tests; 38 passed and only the known
`assemblies::views::zoom_in_associativity_test` failed, reproducing
`location=(0,513), resolution=(1,2), zoom 0 → +1 → +15`.

The post-change run completed the exact same 10,302,563 counted Mandelbrot iterations as the
pre-change run. The 98% wall-time reduction comes from deleting the uncounted 100,000-step
timewarp and tighter-epsilon re-search on every interior completion, not from doing less
screen work.

The period-correctness fix (ascending partials instead of last-record-only, minimal-period
reduction, tail-started Newton, period-0 = unknown) costs ~25% on full-frame versus the first
pipeline cut — the price of the per-completion partial replay — while remaining ~42× faster
than the timewarp code it replaced.
