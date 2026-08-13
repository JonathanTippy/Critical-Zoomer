# RCA: i64 naive four-quadrant grey (2026-08-13)

Headed screenshots:

- `mag 2^43  -0.1761779392230477 + 1.0870336335448237i`
- `mag 2^43  -0.2067325560057166 + 1.1075689870974698i`
- `mag 2^44  -0.104628673854 + 0.959133336119i`

Picture: **four axis-aligned grey rectangles, one slightly lighter.** Sharp
H/V cuts. Not Mandelbrot. HUD `stack:i64` `mode:naive` `gear:F64` (OG stamp)
`ipp:0` pps high.

**Not fixed.** Screen worker only. Shade is a pipe.

---

## Admit is not the bug

C-generator admit counts **significand bits** against |c| down to pixel pitch
(plus margin). It does not treat exponent range as precision. At this locus
that count is ~54; `CopyIntExp<1>` is 64. **Admitting the type is correct.**
This is not “false-admit of a shallow type.”

---

## Retracted

- Mag-~38 **black** was OG naive f64, assistant regression. Read `stack:`.
- Collector `to_f64(c)` / WorkUpdate type param. `c` is not a render input.
  Publish is f32 `z` + f64 scalars.
- “Admit should have refused i64.” No.
- Uniform flat grey. The picture is a **2×2**.

---

## What the 2×2 means

The patch does not cross `c = 0`. Screen cuts are **pixel-index** cuts: only
a couple of distinct answers across 854×480. Same look as “not enough
mantissa for the stencil” — either the tape does not keep the bits admit
counted, or something **narrows to f64 (53 bits)** on the way.

Two places that can do that, both in the screen worker, both after a correct
admit:

### 1. CopyIntExp itself (likely)

The type is new. Iterate is `z ← z² + c` on `CopyIntExp<1>` add/mul
(`iterate_with_c`). `get_c` is `origin + space * seat` on the same add/mul.
Squeeze-align **drops the finer mantissa** when exps differ. Admit counted
64 bits; those ops can still throw away the pixel delta on the first square
or the first `origin + pitch`. Neighbors become a handful of orbits → 2×2
grey. Same class of bug whether it hits in `get_c` or in iterate.

### 2. Accidental f64 on the iterate/`c` path (same picture)

53 bits at mag 43, |c|~1, is not enough. Big rectangular blocks.

On the **absolute** naive i64 path (`from_stencil_with_margin(..., false)`),
`ensure_started` copies `get_c` in `T` with no f64. Iterate stays in `T`.

f64 that **does** exist in this actor and would explain the picture if it
actually runs:

- `DirectKernel::start_seat`: if `coords_are_relative`, `c` is
  `delta_c.to_f64()` → `c_from_delta_c_f64` → `T::from(f64_to_intexp)`.
  Naive i64 is supposed to be absolute. If
  `rebuild_generator_for_reference` (cie Replace still installs
  `pending_reference`) flips the shell to relative, this cast is live.
- `work_update_to_f64` / `completed_to_f64` after the seat is done — too
  late for a 2×2 of **answers**, and `c` is not a color input.
- `update_point_results` compares smallness via `to_f64` — scalars, not
  neighbor `c`.
- `direct_completion` period check uses f64 `c` **after** `repeats` — not
  the escape-time field.

---

## How to tell 1 vs 2 (not done)

- Distinct `get_c` in `CopyIntExp` at headed res, this mag: if **few** unique
  `c`, the tape already lost index in add/mul (`1`). If **many** unique `c`
  but answers are still 2×2, iterate lost them (`1`) or a mid-bout f64 (`2`).
- `coords_are_relative` on headed `cie_live`: if true, suspect `2` first.

`og_copy_intexp1_headed_mag_43_not_all_interior` is 16×17. It does not see
headed 854×480.

Do not refuse the type. Do not re-break OG f64 naive. Do not put `c` on
WorkUpdate to paint this.
