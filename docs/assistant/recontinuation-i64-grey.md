# Recontinuation — i64 four-quadrant grey

Paste into a new chat. Do not revive retracted theories.

## Job

Headed **mag 43–44**, `stack:i64` `mode:naive`: **four axis-aligned grey
tiles**, one slightly lighter. Screen worker. Not fixed. Shade is a pipe.

Loci (screenshots in prior chat assets):

- `mag 2^43  -0.1761779392230477 + 1.0870336335448237i` (`HEADED_I64_GREY_*`)
- `mag 2^43  -0.2067325560057166 + 1.1075689870974698i`
- `mag 2^44  -0.104628673854 + 0.959133336119i`

HUD `gear:F64` is the OG DirectKernel stamp. Read `stack:`.

## Binding

- **Admit is correct.** Significand bits, not exponent range. 64 vs ~54 is enough.
- **Screen seats are always ≥ 0.** Origin = **top-left**. +seat = right, +row =
  down. There are **no negative screen offsets.** Do not invent center-relative
  signed δ.
- **Drag test:** tiles stay on the **window**, they do not slide with the plane.
  So the defect tracks **seat/row** (or something else indexed from UL), not a
  fixed objective-`c` grid.
- `get_c`: `origin + space * from_u32(seat)`, `origin − space * from_u32(row)`
  (`c_generator.rs`). Iterate: `z ← z² + c` on `CopyIntExp<1>` (`iterate_with_c`).
- WorkUpdate: **f32** recontinuable `z`, **f64** scalars (smallness, times).
  Worker must **not** emit `c`. `docs/assistant/design/work-update.md`.
- Mag-~38 **black** was OG naive **f64**, assistant regression. Closed. Do not
  re-break it.

## Retracted (do not reuse)

Collector `to_f64(c)` as the picture. False-admit of i64. Negative / screen-center
signed offsets. Uniform “flat grey” (it is a 2×2).

## Live suspects

1. **CopyIntExp** (new add/mul/From) loses pixel-index bits on **non-negative**
   `origin + k*space` or on iterate.
2. **Accidental f64** on the iterate/`c` path (53 bits → big rectangles). Absolute
   naive is supposed to stay in `T`. `DirectKernel::start_seat` f64-roundtrips `c`
   only if `coords_are_relative`.

16×17 pin `og_copy_intexp1_headed_mag_43_not_all_interior` does not see headed
854×480.

Product code is developer-driven. Docs: `docs/assistant/rca-i64-flat-grey-2026-08-13.md`
(may still contain the retracted signed-center guess — this note wins).
