# CopyIntExp — fixed-tape IntExp for iterate

`IntExp` is an unbounded mantissa (`rug::Integer`) plus `exp`. Adding two values
left-shifts the coarser mantissa onto the finer exp and **grows** `val`. That
only works with an infinite tape.

`CopyIntExp<Words>` is the same number, but the mantissa is `[i64; Words]` on
the stack. The tape cannot grow. When a result would need more bits, **squeeze**:
drop low bits, keep high bits, raise `exp`.

Limbs stay `i64` in memory. Multi-limb two’s complement uses **unsigned 64-bit
patterns** on *lower* limbs; the **high** limb is sign-extended into the carry
word. `pack_add` keeps the sum when that high word is `0` or `-1` (fits);
otherwise it shifts one word and adds 64 to `exp`.

`PRECISION.significand_bits` is `Words * 64` (admit). `From` squeezes to
`Words*64 − 1` magnitude bits so bit 63 stays the sign. `min_exponent` is
`i32::MIN`.

## Add

1. Align to the **coarser** exp (the larger one). Right-shift the finer mantissa.
2. Add limbs. High limb is signed; keep the word when the extra high word is
   sign extension (`0` or `-1`). A real extra word still shifts one word right
   and adds 64 to `exp`. Mixed-sign wrap that lands in range stays as-is
   (`a + (-a)` is zero). Unsigned-only carry used to turn `origin − pitch`
   into `1 × 2^12` (headed imag 4096) or collapse every real seat.

Never left-shift to keep extra precision. That would throw away high bits or
need a longer tape.

## Mul

Schoolbook into `2×Words` limbs for `Words > 1`. **`Words = 1` is signed
`i128` product**, then squeeze until it fits an `i64` limb (shift 64, add 64
to `exp`). Unsigned 64×64 on a negative limb is not `z²`. Not Karatsuba.

## Finite

No infinities, same as `IntExp`. Overflow is a scale bump, not NaN/Inf.
`is_finite` is always true.

## From IntExp

Digit copy of the **absolute** mantissa into the limb window. Squeeze until
`significant_bits ≤ Words×64 − 1`, then two’s-complement if the source is
negative. A 64-bit magnitude in a signed `i64` stole the sign (headed UL imag
+ → −). Viewport `IntExp` often keeps extra low bits past the admit count;
panicking there killed the screen worker.

## OG naive (`Words = 1`)

Manual Naive CPU uses `DirectKernel` on `CopyIntExp<1>` when f32 does not admit
and absolute f64 fails the bit-count gate (home ~**42**). One word still covers
through ~**49–52**. HUD host stack label is `i64`. Not a perturbation compute-gear
rung. If f64 is admitted, iterate must work; mag-38 black with `stack:f64` is a
bug, not a reason to bump the host.

Headed 2026-08-13: **black on OG naive f64 was an assistant regression** (CopyIntExp
wire-up / `From` panic / illegal `1e-14` host bump). HUD `gear:F64` reports the
OG naive compute-gear stamp, not the i64 tape — mag 43 `stack:i64` still shows
`gear:F64`. Grey was `From` sign-bit plus unsigned `pack_add`/`mul` on negative
limbs. Pins: `from_64bit_positive_mantissa_stays_positive`,
`headed_mag_43_get_c_unique_count_at_window_res`. Admit is correct. Not
WorkUpdate `c`. Do not treat headed as fixed until the developer says so.
