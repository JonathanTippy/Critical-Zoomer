# Testing doctrine — steady-state Rust integration

**Rust steady-state integration tests are the lifeblood of Critical-Zoomer
testing.** Prefer many of them over shell e2e sprawl, microbenches-as-gates, or
“trust the HUD.”

## What “steady-state” means here

Drive the **real production loop** for long enough that cadence, queues, and
telemetry settle — then assert on observable outcomes:

- Screen worker: repeated `workshift` / `workshift_with_kernel` until a home
  (or other) frame is delivered; measure iterations and wall time.
- Workgroup chain: worker → `WorkUpdate` → collector `ViewHud` → headgroup
  `RateCounter` (same path the HUD IPS uses).
- Include scheduling, publish buffers, and remap where the claim is
  full-stack — not a kernel microbench alone.

Microbenches and Criterion fitness remain useful for *how fast* trends
(`docs/assistant/benchmarks.md`). Steady-state **tests** carry smoke floors
that prove numbers and invariants still flow through the machine.

## Timeout pyramid

Wall clocks that **must fire** (a hung thread is a fail, not an infinite `join`):

| Tier | Cap | Count | Timeout |
|------|-----|-------|---------|
| Unit | the rest | 1s | Craftsmanship fills, mutant kills, GPU probes that should be instant |
| Integration | ≤10 | 15s | Module `integration_tier` (`cargo test --lib integration_tier`) |
| E2E | ≤3 | 60s | `e2e_tier` (10s park) + `tests/pipeline_cadence.rs` (OG + GPU) |

Cadence still measures ~5s after first color so settling is real. The wgpu dir lock
waits up to 180s for a **live** holder and reclaims only dead/stale dirs — it must
not delete a lock out from under dummy-head cadence.

## Compile profile

House tests are **`cargo test`** (dev/test): `debug-assertions` and overflow
checks stay on; `opt-level = 3` in `Cargo.toml` so Hz/IPS floors see a fast
compile. `--release` is the wrong gate — it drops those checks. Criterion
benches still use the release profile. Headed screenshot builds stay
`--release`.

## Where they live

| Layer | Location | Role |
|---|---|---|
| Screen worker alone | `craftsmanship_tests/integration_tier.rs` (`steady_state_screen_worker_*`) | Fill + IPS floors on DirectKernel / naive-GPU path |
| Post-fill park settle | `steady_state_home_stays_parked_for_10s_after_fill` | 10s wall idle after home fill: seats stay delivered, 0 iters |
| Workgroup chain | `craftsmanship_tests/integration_tier.rs` (`steady_state_workgroup_*`) | `iterations_delta` / `points_delta` survive into HUD `RateCounter` |
| **Pipeline cadence (dummy head)** | `tests/pipeline_cadence.rs` | One harness (not lib+bin duplicate). Cross-process lock. Floors: OG esc ≥15, GPU esc ≥40. House compile is **debug + opt-level 3** (`profile.dev` / `profile.test`), not `--release` (that drops overflow checks). `full_check.sh` runs this last among tests. Headed HUD is still the product check. |
| Home PPS ratio | `steady_state_home_pps_gpu_vs_cpu_ratio` | GPU vs CPU wall PPS (climb toward ~FLOP ratio) |
| GPU host queues | `steady_state_naive_gpu_home_neighbor_queues_grow` | Finals grow out/in/edge queues (no bulk skip) |
| GPU no CPU mop | `steady_state_naive_gpu_home_fills_without_cpu_mop` | Home closes on GPU; no ≥N% DirectKernel mop |
| GPU no Dummy holes | `steady_state_naive_gpu_home_no_dummy_holes` | Collector grid fully filled after GPU home |
| F64 gear escalate | `steady_state_naive_gpu_f64_gear_via_faux_user_zoom` | Faux-user zoom past F32 wall → GPU F64 / CPU fallback |
| Deep cusp never-stall | `steady_state_naive_gpu_deep_cusp_never_stalls` | Progress every shift at hard cusp (resume, not reset) |
| Probe / FLOP ratio | `naive_gpu` smoke tests | GPU vs CPU ratio method (D-NGPU-5) |
| Fitness trends | `benches/workgroup_fitness.rs` | Wall-clock baselines, eye regression guard |

IPS/PPS steady-state tests must **not** assume a CPU residual mop phase after
bulk GPU fill. Completeness is a GPU+host-queue property
(`r[cz.craft.gpu-host-queue-discovery+1]`).

## Rules of thumb

1. **Assert the pipe, not only the ALU.** If the worker iterates but
   `WorkUpdate.iterations_delta` is wrong, the HUD lies — that is a failing
   steady-state test.
2. **Many small steady-state tests beat one giant e2e shell.** Keep them in
   Rust next to the code; pin with Tracey where they guard a rule.
3. **Screenshots remain assistant-owned visual corroboration**
   (`scripts/screenshot_check.sh` only) — not a substitute for these.
4. **When adding a performance-sensitive path** (naive GPU, gear switch,
   telemetry), add or extend a steady-state test in the same change.

## House run

`scripts/full_check.sh` runs tests cheapest-first so a unit failure does not
wait on 15s/60s suites:

```bash
taskset -c 4-11 nice -n 15 cargo test --lib -- --skip integration_tier --skip e2e_tier
taskset -c 4-11 nice -n 15 cargo test --lib integration_tier -- --nocapture
taskset -c 4-11 nice -n 15 cargo test --lib e2e_tier -- --nocapture
taskset -c 4-11 nice -n 15 cargo test --test pipeline_cadence -- --nocapture
```

Pipeline cadence lives in `tests/pipeline_cadence.rs` (one harness; dir lock).
**Release** is the bar. Headed `esc:` with `escape:GPU` is still the product check.
