# Incomplete point churn (PO)

> Point churn in particular is a common issue in the old design. There should be no occasion for it, it is an antipattern to forestall an already started point which was started as part of edge tracing.

**Rule:** Do not rotate / preempt an unfinished seat once edge tracing has started it. Finish that seat before picking another edge seat.

Live: `tile_session.rs` `work_one_seat` — unfinished `Step::Edge` stays at queue front (no `rotate_step`).
