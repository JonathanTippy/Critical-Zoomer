# CopyIntExp — fixed-tape IntExp for iterate

`IntExp` is an unbounded mantissa (`rug::Integer`) plus `exp`. Adding two values
left-shifts the coarser mantissa onto the finer exp and **grows** `val`. That
only works with an infinite tape.

`CopyIntExp<Words>` is the same number, but the mantissa is `[i64; Words]` on
the stack. The tape cannot grow. When a result would need more bits, **squeeze**:
drop low bits, keep high bits, raise `exp`.

## Add

1. Align to the **coarser** exp (the larger one). Right-shift the finer mantissa.
2. Add as two’s-complement limbs (unsigned carry). If same-sign overflow produces
   an extra carry word, shift one word right and add 64 to `exp`. Mixed-sign wrap
   that lands in range stays as-is (`a + (-a)` is zero).

Never left-shift to keep extra precision. That would throw away high bits or
need a longer tape.

## Mul

Schoolbook into `2×Words` limbs (unsigned magnitude, then sign). While the high
half is used, shift one word right and add 64 to `exp`. Same squeeze as add;
not Karatsuba.

## Finite

No infinities, same as `IntExp`. Overflow is a scale bump, not NaN/Inf.
`is_finite` is always true.

## From IntExp

Digit copy into the limb window (panic if it does not fit), then two’s-complement
if the source is negative.
