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
| time_to_first_publish (screen-space filament derivative) | 72.07 ms (71.64–72.50 ms; +10.0% vs period-correctness fix) | 2026-08-07 | post-feature working tree | same |
| time_to_full_frame (screen-space filament derivative) | 316.72 ms (312.49–320.39 ms; +7.9% vs period-correctness fix) | 2026-08-07 | post-feature working tree | same |
| full_stack_ips (screen-space filament derivative) | ~4.0e7 (10,302,563 iterations, 24–26 typical shifts; 37.1–42.5M observed) | 2026-08-07 | post-feature working tree | same |
| time_to_first_publish (false-filament sign guard) | 71.60 ms (71.02–72.26 ms; no change detected) | 2026-08-07 | post-fix working tree | same |
| time_to_full_frame (false-filament sign guard) | 316.26 ms (308.65–325.35 ms; no change detected) | 2026-08-07 | post-fix working tree | same |
| time_to_first_publish (tendril raw-contrast guard) | 71.68 ms (71.19–72.19 ms) | 2026-08-07 | post-fix working tree | same |
| time_to_full_frame (tendril raw-contrast guard) | 314.77 ms (310.38–319.19 ms) | 2026-08-07 | post-fix working tree | same |
| time_to_first_publish (stencil-only lazy Replace) | 41.28 ms (40.89–41.68 ms; −42% vs tendril guard) | 2026-08-07 | post-feature working tree | same |
| time_to_full_frame (stencil-only lazy Replace) | 322.41 ms (315.76–328.09 ms; noise vs tendril guard) | 2026-08-07 | post-feature working tree | same |
| full_stack_ips (stencil-only lazy Replace) | ~3.6e7 (10,302,563 iterations, 26–29 typical shifts) | 2026-08-07 | post-feature working tree | same |

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

The screen-space filament derivative adds one complex multiply-add to every Mandelbrot
iteration. Its measured full-frame wall-time cost is 7.9%, within the 20% regression guard;
this is accepted feature cost for retaining in-filaments during interim zoom frames.

Stencil-only Replace drops the seeded context from the command channel and materializes seat
coordinates at first start. `time_to_first_publish` improves ~42%; full-frame wall time is
unchanged within noise (same 10,302,563 counted iterations).
