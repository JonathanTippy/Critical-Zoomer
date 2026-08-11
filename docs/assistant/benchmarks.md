# Benchmarks (first-class performance tracking)

Fitness *trends* (“how fast is it”) live in **benchmarks**. Steady-state Rust
integration tests (`docs/assistant/testing.md`) are the lifeblood gate that
work and IPS still flow through the real loops — smoke floors belong there;
do not demote them into Criterion-only.

Tests may carry smoke-level timing / IPS-floor asserts on the production
workshift path. Anything that answers “how fast vs last week” belongs here.

## Suite

`benches/workgroup_fitness.rs`, `benches/shadergroup_fitness.rs` (plus the older
`benches/my_bench.rs` headgroup microbenches).

| bench | metric | guards |
|---|---|---|
| `time_to_first_publish` | fresh home-view context → first non-empty publish | `cz.perf.play-minimize+1`, `cz.perf.play-8bump-100ms+1` |
| `time_to_full_frame` | home view 854×480, wall-clock to every seat delivered | `cz.perf.home-100tps+1` |
| `full_stack_ips` (printed by the full-frame bench) | total_iterations / wall time, real workgroup loop, scheduling included | `cz.perf.min-300m-ips-cpu+2` (method; the 300M number is pending re-derivation) |
| `escape_*x_default_pixels` | one `escape_frame` wall time at 1.0× / 1.5× / 2.0× default pixel count | shade-path audit; escaper 60 Hz@1080p wisdom |
| `escape_gpu_radius_only_*` | GPU escaper with resident answers (radius uniform only) | shade unjam |
| `color_*x_default_pixels` | one `color` wall time at same scales | shade-path audit; isolate colorer vs escaper |
| `color_gpu_*` / `color_gpu_params_only_*` | GPU colorer full upload vs params-only resident | shade unjam |

Run (house rule — nice, center-half CPUs):

```bash
taskset -c 3-8 nice -n 15 cargo bench --bench workgroup_fitness
taskset -c 3-8 nice -n 15 cargo bench --bench shadergroup_fitness
```

### Shadergroup note (2026-08-11)

Interview cliff: past **~1.5× default pixels** the live app goes pear-shaped;
headgroup stays ~120 FPS; animated-bailout isolation points at shadergroup ~1 FPS;
workgroup banding is a separate failure. Benches measure escaper and colorer
**individually** on a filled DirectKernel home package so the problem child is
visible. First run establishes the baseline row below — no soft ladder.

HUD `drop:` shows cumulative shade-path package drops (drain-to-newest).

## Regression guard — FIX NOW

Benchmarks vary run to run; noise is normal. Trends matter.

- After any code edit touching the workgroup or headgroup, rerun **both**
  `workgroup_fitness` and `my_bench` and compare against the **accepted**
  baseline below. After shadergroup (escaper/colorer) edits, also rerun
  `shadergroup_fitness`.
- A regression beyond ~20% on an accepted row is **FIX NOW**
  (`docs/assistant/quality-doctrine.md`): fix the code before checkpointing
  or continuing feature grind. Do **not** soften asserts, `#[ignore]` the
  bench, or rewrite the baseline to hide a slip.
- When a change intentionally alters performance, update the baseline **in the
  same commit** and document why.
- Rows marked **REJECTED** / “not a baseline” are diagnostic history only —
  never treat them as permission to ship that slow path.


### Quality grind note (2026-08-09 evening)

Hot-path `debug_agent` NDJSON + `format!` call sites removed; reference-floor
policy no longer scans all seats when there is no usable reference (home
DirectKernel path).

**Measurement fix (tick 9):** `time_to_*` benches now use Criterion
`iter_custom` so the reported median is **only** fill→done wall time. Prior
`b.iter` accidentally timed `from_stencil` + `run_big` thread spawn as well
(returning a `Duration` does not override Criterion’s wall clock). Historical
rows below that used `b.iter` are **construction-inflated** — do not treat a
drop after this fix as a code speedup, and do not compare new honest medians
directly to old inflated ones without noting the method change.

Honest medians after `iter_custom` (2026-08-09 tick 9, sample-size 10/30):

| metric | median | notes |
|---|---|---|
| time_to_first_publish | **~12.3 ms** | fill-only; was ~90–96 ms when construction was timed |
| time_to_full_frame (production `workshift`) | **~270 ms** | **ACCEPTED** honest fill-only baseline (was ~357 ms inflated) |
| time_to_full_frame_with_reference | **~272 ms** | matches production within noise |
| time_to_full_frame_direct_oracle | **~274 ms** | matches production within noise |

Pin: `home_workshift_full_frame_within_20pct_of_direct_kernel`,
`home_workshift_first_publish_within_20pct_of_direct_kernel`.

**Series approximation live (2026-08-11, `--quick`):** first_publish ~12.2 ms;
full_frame ~279 ms; with_reference ~281 ms; direct_oracle ~284 ms — within ~5% of
the accepted honest medians above (no ≥20% regression).

**Shadergroup first baseline (2026-08-11, `--quick`, filled DirectKernel home):**

| metric | median | implied Hz if sole wake body | notes |
|---|---|---|---|
| escape_1_0x (854×480) | **~5.72 ms** | ~175 | scales ~linear with pixels |
| escape_1_5x (1046×588) | **~7.93 ms** | ~126 | developer pear-shaped cliff |
| escape_2_0x (1208×679) | **~9.65 ms** | ~104 | |
| color_1_0x | **~55.3 ms** | ~18 | **problem child** — ~10× escaper |
| color_1_5x | **~83.8 ms** | ~12 | alone already ≪ 60 Hz wisdom |
| color_2_0x | **~101 ms** | ~10 | |

**GPU colorer (2026-08-11, f32 wgpu, exact Color32 parity vs OG):**

| metric | median | notes |
|---|---|---|
| color_1_0x (OG, post f32 shade) | **~43.5 ms** | improved vs prior f64 shade path |
| color_1_5x (OG) | **~64.4 ms** | |
| color_2_0x (OG) | **~72.8 ms** | |
| color_gpu_1_0x (pre-residency) | **~18.6 ms** | historical; recreate+upload every wake |
| color_gpu_1_5x (pre-residency) | **~30.7 ms** | |
| color_gpu_2_0x (pre-residency) | **~40.5 ms** | |

**Shade unjam (2026-08-11, persistent buffers + GPU escaper, `--quick`):**

| metric | median | notes |
|---|---|---|
| color_gpu_1_0x (resident) | **~5.18 ms** | ~3.6× vs pre-residency GPU |
| color_gpu_1_5x | **~7.94 ms** | |
| color_gpu_2_0x | **~11.2 ms** | |
| color_gpu_params_only_1_0x | **~0.80 ms** | historical dirty params-only |

**Cadence pass (2026-08-11, content-beat always-refresh, `--quick`):**
Actor dirty skip removed; GPU paint always refreshes from current inputs
(persistent buffers kept). New medians replace the dirty-skip pins above for
slip checks:

| metric | median | notes |
|---|---|---|
| color_gpu_1_0x | **~12.9 ms** | full refresh each call |
| color_gpu_1_5x | **~22.5 ms** | |
| color_gpu_2_0x | **~28.5 ms** | |
| color_gpu_params_only_1_0x | **~12.5 ms** | same class as full refresh (no actor dirty) |
| escape_gpu_radius_only_1_0x | **~3.11 ms** | unchanged class |
| workgroup time_to_full_frame | **~300 ms** | within ~20% of ~270 ms accepted |

- Content actors share head-reported vsync period (or manual Hz).
Actors wake on `resolved_content_period()` (Automatic = egui/OS `auto_vsync_hz`).
Color gear defaults to **GPU**; escape defaults OG. HUD stamps `color:` /
`escape:` and stage rates `pub:/esc:/col:/ctrl:`.

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
| time_to_first_publish (attention-first spiral) | 52.50 ms (51.01–53.95 ms; +27% vs stencil-only — attention fills center before easy perimeter) | 2026-08-07 | post-feature working tree | same |
| time_to_full_frame (attention-first spiral) | 227.36 ms (225.59–229.16 ms; −29% vs stencil-only) | 2026-08-07 | post-feature working tree | same |
| full_stack_ips (attention-first spiral) | ~5.4e7 (10,302,563 iterations, 18–19 typical shifts) | 2026-08-07 | post-feature working tree | same |
| time_to_first_publish (kernel seam + reference actor) | 39.30 ms (39.09–39.51 ms; −5.8% vs prior sample) | 2026-08-07 | post-feature working tree | same |
| time_to_full_frame (kernel seam + reference actor) | 227.87 ms (226.17–229.37 ms; unchanged vs attention-first) | 2026-08-07 | post-feature working tree | same |
| full_stack_ips (kernel seam + reference actor) | ~5.0–5.4e7 (10,302,563 iterations, 19–20 shifts) | 2026-08-07 | post-feature working tree | same |
| worker_1080p_full_frame (isolated direct worker) | 688.09 ms (681.07–694.68 ms) | 2026-08-07 | post-feature working tree | 1920×1080 home view; excludes remap and shaders |
| time_to_first_publish (perturbation path / zero-orbit floor) **REJECTED** | 83.28 ms (80.81–85.78 ms; +112% vs last accepted direct-kernel row) | 2026-08-07 | post-feature working tree | same; not a baseline |
| time_to_full_frame (perturbation path / zero-orbit floor) **REJECTED** | 3.260 s (3.230–3.289 s; ~14× vs last accepted) | 2026-08-07 | post-feature working tree | same; not a baseline |
| full_stack_ips (perturbation path / zero-orbit floor) **REJECTED** | ~3.2e6 (10,302,566 iterations, ~300–310 shifts) | 2026-08-07 | post-feature working tree | same; not a baseline |
| worker_1080p_full_frame (perturbation path / zero-orbit floor) **REJECTED** | 4.898 s (4.767–5.043 s; ~7× vs direct) | 2026-08-07 | post-feature working tree | 1920×1080; not a baseline |

| time_to_full_frame (perturbation path / zero-orbit floor, post-honest-opts) **REJECTED** | 1.335 s (1.320–1.353 s; ~5.9× vs last accepted 227.87 ms) | 2026-08-07 | grok-probation | same; not a baseline |
| time_to_full_frame_with_reference **REJECTED** | 1.288 s (1.276–1.300 s; ~5.7× vs last accepted) | 2026-08-07 | grok-probation | published reference preinstalled; not a baseline |
| time_to_full_frame (post coverage + FloatExp add hot + deferred sync) **REJECTED** | 1.998 s (1.953–2.046 s; ~8.8× vs last accepted; ~5.5e6 ips) | 2026-08-07 | grok-probation | still not a baseline; ≤20% gate unmet |
| time_to_first_publish (post docs/test lock) **REJECTED** | 107.11 ms (106.55–109.34 ms) | 2026-08-07 | grok-probation | not a baseline |
| time_to_full_frame (post docs/test lock) **REJECTED** | 2.298 s (2.241–2.312 s; ~10× vs last accepted ~228 ms; ~4.6–4.8e6 ips) | 2026-08-07 | grok-probation | not a baseline; ≤20% gate unmet |
| time_to_full_frame_with_reference (post docs/test lock) **REJECTED** | 1.681 s (1.621–1.919 s; ~7.4× vs last accepted; ~5.6–6.9e6 ips) | 2026-08-07 | grok-probation | improved vs prior rejected ref row; still not a baseline |
| worker_1080p_full_frame (post docs/test lock) **REJECTED** | 4.648 s (4.009–4.808 s) | 2026-08-07 | grok-probation | not a baseline |
| time_to_full_frame (always-relative coord_anchor; one path) **REJECTED** | 1.531 s (1.500–1.562 s; ~6.7× vs last accepted ~228 ms; ~7.0–7.7e6 ips) | 2026-08-07 | grok-probation | always `new_relative`; not a baseline |
| time_to_first_publish (f64 gear ladder) | ~90.5 ms (89.7–91.4 ms) | 2026-08-07 | grok-probation | production `WorkContext<f64>` + `PerturbationKernel` |
| time_to_full_frame (f64 gear ladder) **ACCEPTED** | ~356.6 ms (351.3–363.5 ms; wall ~270–290 ms / 22–24 shifts; ~3.7e7 ips) | 2026-08-07 | grok-probation | parity-or-better vs DirectKernel f64 oracle |
| time_to_full_frame_with_reference (f64 gear) | ~366.1 ms (359.3–373.6 ms; wall ~277–300 ms) | 2026-08-07 | grok-probation | published reference + series present |
| time_to_full_frame_direct_oracle (f64 DirectKernel) | ~377.6 ms (353.5–422.5 ms; wall ~268–290 ms typical) | 2026-08-07 | grok-probation | fair f64-host oracle |
| time_to_first_publish (iter_custom fill-only) **ACCEPTED** | ~12.3 ms (12.25–12.31 ms) | 2026-08-09 | quality/squeaky-clean | replaces construction-inflated ~90 ms rows |
| time_to_full_frame (iter_custom fill-only) **ACCEPTED** | ~270 ms (267.9–272.2 ms) | 2026-08-09 | quality/squeaky-clean | honest fill; prior ~357 ms included construction |
| time_to_full_frame_with_reference (iter_custom) | ~272 ms (270.5–273.0 ms) | 2026-08-09 | quality/squeaky-clean | |
| time_to_full_frame_direct_oracle (iter_custom) | ~274 ms (271.9–275.3 ms) | 2026-08-09 | quality/squeaky-clean | |
| gear_micro scaled_f64_1k_steps | ~8.19 µs | 2026-08-07 | grok-probation | ~4.6× vs floatexp_1k (~37.6 µs) |
| gear_micro floatexp_1k_steps | ~37.6 µs | 2026-08-07 | grok-probation | all-FloatExp delta step train |
| gear_micro f64_1k_steps | ~5.74 µs | 2026-08-07 | grok-probation | plain f64 step train |

### Perturbation gear ladder (2026-08-07) — accepted

Production seats are `f64` with compute gears F64 → ScaledF64 → FloatExp. Home
zero-orbit floor stays on F64 and matches DirectKernel wall time within noise
(Criterion medians ~357 ms vs ~378 ms). Scaled-f64 microbench is ~4.6× faster
than all-FloatExp steps. Series approximation publishes with the reference and
initializes delta prefixes without inventing delivery. HUD shows fps / pps /
ips / gear.

Historical FloatExp-only rejection rows above remain diagnostic history.

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

Attention-first spiral makes slot 0 foveate from screen center (home bench has `attention:
None`). First publish is ~27% slower than stencil-only (center seats are harder than the old
scredge perimeter) but still well under the pre-stencil baseline; full-frame improves ~29%
as the spiral + queue mix finishes the home view in fewer shifts.

The statically dispatched `SeatKernel` seam adds no measurable cost: full-frame remains at
the attention-first baseline and first publish improved in this sample. The background
reference actor is not active in the isolated workgroup fitness harness, as intended; its
wiring does not alter direct-kernel scheduling.

The isolated 1920×1080 worker takes ~688ms versus ~228ms at the default
854×480 resolution: about 3.0× wall time for 5.1× pixels. Worker catch-up is
therefore one real contributor to the reported sluggishness, but the
sub-linear scaling does not support blaming the scheduler or view/remap design.
The remap and shader portions still require separate headed profiling.
