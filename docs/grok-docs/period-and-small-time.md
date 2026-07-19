# Period and small_time

## Antipattern (narrow)

Bolting a heavy `determine_period` (timewarp + re-search) onto **every** mid-loop repeat — desperation for period bugs. Not the product shape.

## Required division

| Phase | Role |
|--------|------|
| Regular iterate | Step `z`, update `small_time` / `|z|`, bailout → Outside. Simplest **certain** inside check only — **never false periodicity** (tenacity). May leave period unknown. |
| After boundary + out-fill | **Period determination** on the in-edge (necessary complexity). |
| Later | Small-time edge trace; filament traces (D-SCH-3). |

## Paint

Out-filament (and related) must tolerate **Inside period-unknown** vs **Inside period-known** without an ugly seam.

Live iterate: `naive_cpu_worker.rs` `iterate_point_bout`. Scheduler phases: `tile_session.rs` (D-PER-1 not yet implemented).
