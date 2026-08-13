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

Manual Naive CPU uses `DirectKernel` on `CopyIntExp<1>` when f32 does not admit,
absolute f64 `CGenerator::new_with_margin` fails, and the one-word tape still
covers the stencil. Home, default margin 1: f64 dies at zoom **42**; one word
still admits through zoom **49** (and until bits exceed 64, about **52**). HUD
host stack label is `i64`. Not a perturbation compute-gear rung.
