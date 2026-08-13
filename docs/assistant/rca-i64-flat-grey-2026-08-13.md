# RCA: i64 naive flat grey (2026-08-13)

Headed observation (developer):

```
mag 2^43  -0.1761779392230477 + 1.0870336335448237i
```

Picture: **flat grey**, not Mandelbrot. HUD: `stack:i64`, `mode:naive`,
`gear:F64`, `ipp:0`, pps high. Constants: `HEADED_I64_GREY_*`.

**Not fixed.** Do not treat this as closed.

This is **not** the mag-~38 black. That was OG naive on **f64** (`stack:f64`),
an assistant regression while wiring `CopyIntExp` (`From` panic, then an illegal
`1e-14` host bump). Developer: black is closed. HUD `gear:F64` on naive is the
OG DirectKernel / ComputeGear stamp, including when the host is i64. Read
`stack:`, not `gear:`. Assistant mistake: treating `gear:F64` as “this is f64
iterate / the i64 kernel.”

---

## What is still iterating

At mag 43, absolute f64 fails the bit-count admit. Manual Naive CPU uses
`DirectKernel` on `CopyIntExp<1>` (`stack:i64`).

The pin `og_copy_intexp1_headed_mag_43_not_all_interior` shows:

- seats **do** escape (not all-interior black)
- `view_ipp() > 0` (tape is iterating)
- **more than one** escape time (neighbors are not the same orbit)

So the i64 iterate is not “everyone Dummy 100.” Headed `ipp:0` is a **HUD /
publish lie**, not “no work.”

---

## Why shade is flat grey

The collector still takes `WorkUpdate<f64>`. Completions from the i64 tape are
narrowed with `to_f64` on `c` (and `z`) before color.

At this location `|c| ~ 1`. Pixel pitch at mag 43 is far below 1 ulp of f64
near 1. Neighbor seats that differ on the i64 tape **round to the same f64
`c`**. The pin: unique `to_f64(c)` bit-pairs **< seat count**.

Colorer / dummy-smallness then see a field of collapsed `c`/`z`. Dummy 100 on
that collapsed field is the same flat grey.

High pps + `ipp:0` on the headed HUD is this interlayer (f64 publish), not an
idle worker.

---

## What this is not

- Not location-HUD decimal rounding (display only).
- Not the closed fat-mantissa `From<IntExp>` panic.
- Not “bump CopyIntExp earlier” (that re-broke OG f64 naive).
- Not `gear:F64` meaning the wrong kernel.

---

## What a fix would have to change (not done)

Keep answers at a width that still separates neighbors through collector →
shade, **or** stop coloring from f64 `c` at this depth. Product change is
developer-driven. Pin stays; grey stays open until you say otherwise.

Links: `docs/assistant/issue-stack.md` (true bugs),
`docs/assistant/design/copy-intexp.md`,
`src/assemblies/workgroup/screen_worker/workshift.rs`
(`og_copy_intexp1_headed_mag_43_not_all_interior`).
