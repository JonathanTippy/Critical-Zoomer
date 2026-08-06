# GPU TPS experiment index

| Id | Date | Step | TPS | IPS | sync notes | Verdict | Link |
|----|------|------|-----|-----|------------|---------|------|
| E001 | 2026-08-04 | 1–2 | ~380 | n/a | maps~45 spin | pass | [E001](2026-08-04-E001-baseline.md) |
| E002 | 2026-08-04 | 3–5 | ~470 warm | ≥30B compute | maps=2 waits=0 harvests=0 bridge=0 | pass (TPS still low) | [E002](2026-08-04-E002-kill-sync.md) |
| E003 | 2026-08-04 | 6 | ~520 | ≥30B | bridge=0 handle queue | pass slice | [E003](2026-08-04-E003-dpub4.md) |
| E004 | 2026-08-04 | 7–15 | ~520 | ≥30B | sync clean; period wall | fail-impl → suggest period two-pass | [E004](2026-08-04-E004-pivot-evidence.md) |
