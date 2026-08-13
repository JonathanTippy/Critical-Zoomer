# RCA: i64 four-quadrant grey

**Locus.** Mag 43–44, `stack:i64` `mode:naive`. Open.

**Picture.** Four axis-aligned greys, one slightly lighter. Drag: tiles stay
on the **window**, not the plane.

**Admit is fine.** Significand count (64 vs ~54). Not “refuse i64.”

**Screen space.** Top-left origin, +right, +down. Seats always ≥ 0. No
negative screen offsets.

**Drag.** Defect tracks UL seat/row (or other screen index), not a sliding
objective-`c` grid.

**Guess.** CopyIntExp add/mul/From on non-negative `origin + k*space` (or
iterate `z²+c`) drops pixel bits; or an accidental f64 on that path. Not
WorkUpdate `c`. Not collector.

Handoff: `docs/assistant/recontinuation-i64-grey.md`.
