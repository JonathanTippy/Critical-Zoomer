# Experiment template — GPU TPS ladder

Copy to `YYYY-MM-DD-step-N-short-slug.md`.

## Meta

- **Id:** (e.g. `E001`)
- **Date:**
- **Step:** (plan step number / letter)
- **Preferred path still?** yes / no

## Hypothesis

(Impl claim — not “spiral is slow”.)

## Change

(Files / 1–3 sentences. “None — baseline” is fine.)

## Commands

```bash
taskset -c 3-8 …
```

## Metrics

| Metric | Value |
|--------|-------|
| whole-TPS | |
| fill% | |
| workshifts | |
| ms | |
| compute-only IPS | |
| maps | |
| waits | |
| harvests | |
| cpu_bridge_publishes | |
| nomap_scatters | |
| counter_polls | |
| publish path | gpu_bind / cpu_bridge / mixed / n/a |

## Verdict

pass / fail-impl / inconclusive

## Next

continue ladder / suggest design fallback: _(name + evidence only)_

## Do not retry

(Anti-pattern learned this run, if any.)
