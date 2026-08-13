# CopyIntExp — fixed-tape IntExp for iterate

`IntExp` is an unbounded mantissa (`rug::Integer`) plus `exp`. Adding two values
left-shifts the coarser mantissa onto the finer exp and **grows** `val`. That
only works with an infinite tape.

`CopyIntExp<Words>` is the same number, but the mantissa is `[i64; Words]` on
the stack. The tape cannot grow. When a result would need more bits, **squeeze**:
drop low bits, keep high bits, raise `exp`.

Limbs stay `i64` in memory. Multi-limb two’s complement uses **unsigned 64-bit
patterns** and a widening carry. Sign-extending a limb into `i128` wrecks carry.
Add widens with `as u64 as i128`. Mul schoolbook uses `u128` for 64×64 products
(`(2^64-1)²` does not fit in `i128`). No per-sign abs/restore on the iterate path.

`PRECISION.significand_bits` is `Words * 64`. `min_exponent` is `i32::MIN`.

## Add

1. Align to the **coarser** exp (the larger one). Right-shift the finer mantissa.
2. Add limbs with unsigned carry. If the extra carry word is used, shift one
   word right and add 64 to `exp`. Mixed-sign wrap that lands in range stays
   as-is (`a + (-a)` is zero).

Never left-shift to keep extra precision. That would throw away high bits or
need a longer tape.

## Mul

Schoolbook into `2×Words` limbs (unsigned limb products, algebraic). While the
high half is used, shift one word right and add 64 to `exp`. Same squeeze as
add; not Karatsuba.

## Finite

No infinities, same as `IntExp`. Overflow is a scale bump, not NaN/Inf.
`is_finite` is always true.

## From IntExp

Digit copy into the limb window. If the source mantissa is wider than the tape,
squeeze (drop low bits, raise `exp`) until it fits — same as add/mul. Then
two’s-complement if the source is negative. Viewport `IntExp` often keeps extra
low bits past the admit count; panicking there killed the screen worker.

## OG naive (`Words = 1`)

Manual Naive CPU uses `DirectKernel` on `CopyIntExp<1>` when f32 does not admit
and absolute f64 fails the bit-count gate (home ~**42**). One word still covers
through ~**49–52**. HUD host stack label is `i64`. Not a perturbation compute-gear
rung. If f64 is admitted, iterate must work; mag-38 black with `stack:f64` is a
bug, not a reason to bump the host.

Headed 2026-08-13: **black on OG naive f64 was an assistant regression** (CopyIntExp
wire-up / `From` panic / illegal `1e-14` host bump). HUD `gear:F64` reports the
OG naive compute-gear stamp, not the i64 tape — mag 43 `stack:i64` still shows
`gear:F64`. Remaining product bug is **four-quadrant grey** (`HEADED_I64_GREY_*`,
HUD `ipp:0`): drag-locked to **screen center**. CopyIntExp signed screen-δ
(or f64 of it). Admit is correct. Not WorkUpdate `c`.
RCA: `docs/assistant/rca-i64-flat-grey-2026-08-13.md`. Pin
`og_copy_intexp1_headed_mag_43_not_all_interior` is 16×17 only.
