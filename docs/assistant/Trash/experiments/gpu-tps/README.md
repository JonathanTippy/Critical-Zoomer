# GPU home TPS experiments

Persistent log for the ≥3000 whole-TPS ladder. Preferred algorithms stay preferred; failures are assumed **impl / sync / residency** until proven otherwise. Design fallbacks may be **suggested** with evidence only — never implemented without approval.

## How to run

- **Release only** for TPS / fill bars (`--release`).
- Pin to **center half** of CPUs (12-logical machine → `taskset -c 3-8`):

```bash
taskset -c 3-8 cargo test --release -p critical_zoomer \
  --test tile_session_tests home_800x480_fills_gpu_path_probe -- --nocapture
```

(Adjust package/test binary name if the probe lives in the lib test target.)

- Compute-only IPS: use the GPU worker compute-only path / standards_perf GPU IPS as documented in each experiment row.
- Do not soft-skip `cz.perf.home-10000tps-gpu+1`.

## Metrics

| Metric | Meaning |
|--------|---------|
| whole-TPS | Headgroup-shaped completed whole tiles / second (probe) |
| fill% | `gpu_resident_fill_percent` |
| compute-only IPS | Bout math without readback/Wait |
| maps / waits / harvests / cpu_bridge | Instrument tallies on stationary fill |
| publish path | `gpu_bind` (D-PUB-4) vs `cpu_bridge` |

## Process

1. Copy [TEMPLATE.md](TEMPLATE.md) to `YYYY-MM-DD-step-N-short-slug.md`.
2. Fill every field; run the commands; paste numbers.
3. Add a row to [INDEX.md](INDEX.md).
4. Re-trying a **Do not retry** anti-pattern requires explaining what changed.

## Pivot rule

After fail-impl with clean sync (`maps=0`, `harvests=0`, `cpu_bridge=0`) and TPS still ≪3000 for ≥2 logged attempts: suggest one design fallback in the log and **stop for approval**.
