# RCA: i64 four-quadrant grey

**Locus.** Mag 43–44, `stack:i64` `mode:naive`. Open.

**Picture.** Four axis-aligned greys, one a hair lighter. Crosshairs on the
**window**, not the plane: drag and the quadrants stay put. Shade is a pipe.

**Admit is fine.** Significand count (64 vs ~54) is the right gate. This is
not “refuse i64.”

**What the drag kills.** Objective `c` (UL + seat, or a fixed plane grid).
Those blocks would slide. These don’t. The split is **screen-center signed
δ** — left/right and up/down of the view, four sign pairs, four tiles.

**Best guess.** CopyIntExp mishandles **negative screen offsets** (new
add/mul/shift on a one-word two’s-complement tape). Same look if that δ is
accidentally `to_f64`’d (`DirectKernel::start_seat` when the shell is
relative). Either way: screen-space sign, not objective precision, not
WorkUpdate `c`.

**Not.** OG-black (that was f64 naive). Collector. Putting `c` on the wire.
