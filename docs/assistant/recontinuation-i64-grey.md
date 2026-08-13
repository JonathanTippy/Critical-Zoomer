# Recontinuation — i64 four-quadrant grey

Paste into a new chat. Do not revive retracted theories.

## Job

Headed **mag 43–44**, `stack:i64` `mode:naive`. Screen worker. Shade is a pipe.
Not declared fixed.

Loci:

- `mag 2^43  -0.1761779392230477 + 1.0870336335448237i` (`HEADED_I64_GREY_*`)
- `mag 2^43  -0.2067325560057166 + 1.1075689870974698i`
- `mag 2^44  -0.104628673854 + 0.959133336119i`

HUD `gear:F64` is the OG DirectKernel stamp. Read `stack:`.

## Proven (lib, 854×480)

1. **`pack_add` sign-extension.** Unsigned limb carry `1` on a negative high
   limb is not a new word. Old code squeezed to `1 × 2^12` = 4096 for every
   imag except row 0 → `ipp:0`, field glued to the window. Live tree keeps the
   carry when it matches the sign bit. Distinct `get_c` imag after that.
2. **`From` still steals the sign bit.** Squeeze to 64 magnitude bits into
   signed `i64`. Headed UL imag IntExp (+) becomes CopyIntExp (−). Real sign
   flips the other way. Pins in `src/copy_intexp.rs`.

The headed “four equal greys” picture was **not** what `get_c` produced: it
was **two** imag values (row 0 vs rest). Do not re-invent a 2×2 of screen
halves.

## Binding

- **Admit is correct.** Significand bits, not exponent range.
- **Screen seats are always ≥ 0.** Origin = **top-left**. +seat = right, +row =
  down. No negative screen offsets.
- **Drag** glued to the window while imag was 4096 for row ≥ 1 (UL row index,
  not a sliding objective-`c` grid).
- `get_c`: `origin + space * from_u32(seat)`, `origin − space * from_u32(row)`.
- WorkUpdate: **f32** `z`, **f64** scalars. Worker must **not** emit `c`.
- Mag-~38 **black** was OG naive **f64**. Closed.

## Retracted

Collector `to_f64(c)`. False-admit of i64. Negative / screen-center signed
offsets. Uniform “flat grey.” Accidental f64 on absolute naive iterate.

Product code is developer-driven. RCA:
`docs/assistant/rca-i64-flat-grey-2026-08-13.md`.
