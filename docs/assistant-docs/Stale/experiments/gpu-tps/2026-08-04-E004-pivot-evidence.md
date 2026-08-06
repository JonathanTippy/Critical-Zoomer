# E004 — Sync clean, TPS still ≪3000 (pivot evidence)

## Meta

- **Id:** E004
- **Date:** 2026-08-04
- **Step:** 7–15 evidence / pivot protocol
- **Preferred path still?** yes

## Hypothesis

With maps≈0–2, harvests=0, cpu_bridge=0, and compute-only IPS ≥30B, whole-TPS ~500 is not an ALU shortage. Disabling on-device period reaches ~94.9% fill then stalls; enabling period reaches 96% in ~170–240ms. Integrated every-iteration period on interior/boundary seats dominates the home fill wall under the preferred algorithm.

## Change

None for this row — measurement only (prior period-off experiment reverted).

## Commands

```bash
taskset -c 3-8 cargo test --release --lib home_800x480_fills_gpu_path_probe -- --nocapture
taskset -c 3-8 cargo test --release --lib gpu_ips_batch_1024_meets_30b -- --nocapture
```

## Metrics

| Metric | Value |
|--------|-------|
| whole-TPS | ~520 warm / ~380–470 typical |
| fill% | 96 (period on) / 94.9 stall (period off, reverted) |
| ms | ~173–240 to 96% |
| compute-only IPS | ≥30B |
| maps | 2 |
| waits | 0 |
| harvests | 0 |
| cpu_bridge_publishes | 0 |
| publish path | gpu handles |

## Verdict

fail-impl for gate ≥3000 with preferred path intact and sync clean (≥2 logged attempts: E002, E003/E004)

## Next

**suggest** design fallback (approval required before any impl):

1. **Period two-pass (D-PER-5)** — escape/membership first for move-on/TPS; certain period catch-up in Phase 2-like wave. Evidence: period-off nearly hits 95% from exterior alone; period-on spends the remaining wall on interior/boundary.
2. Do **not** suggest homescreen cheat or lowering the 3000 bar.

## Do not retry

- Blaming spiral / same-tile WIP while maps>0 (maps are clean now).
- Full-screen fused bout co-scheduling deep interior with exterior (minute hangs / ~88% stuck).
- Soft-skip of `cz.perf.home-10000tps-gpu+1`.
