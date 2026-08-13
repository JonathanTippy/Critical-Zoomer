# RCA: i64 four-quadrant grey

**Locus.** Mag 43–44, `stack:i64` `mode:naive`. Open.

**Picture.** Four axis-aligned greys, one slightly lighter. Drag: tiles stay
on the **window**, not the plane.

**Admit is fine.** Significand count (64 vs ~54). Not “refuse i64.”

**Screen space.** Top-left origin, +right, +down. Seats always ≥ 0. No
negative screen offsets.

**Drag.** Defect tracks UL seat/row (or other screen index), not a sliding
objective-`c` grid.

**Guess.** CopyIntExp add treated unsigned carry-1 on a negative high limb as a
new word (`exp+64` → imag 4096 on row 1). Sign extension now keeps the word.
`headed_mag_43_get_c_unique_count_at_window_res` pins `get_c`. Headed 2×2 may
still be iterate. Not WorkUpdate `c`.

Handoff: `docs/assistant/recontinuation-i64-grey.md`.
