# RCA: i64 four-quadrant grey

**Locus.** Mag 43–44, `stack:i64` `mode:naive`. Lib pins green. Headed not
declared fixed.

## Measured

On `HEADED_I64_GREY_*` at 854×480, `CGenerator<CopyIntExp<1>>` after the fix:

- Distinct `get_c` real along row 0: **854**
- Distinct `get_c` imag along seat 0: **480**
- UL imag `From` matches IntExp sign (+) and f64 magnitude

## Causes (all in `CopyIntExp<1>`)

1. **`From`** squeezed to 64 magnitude bits into a signed `i64` (`u64 as i64`).
   Bit 63 became the sign. Headed UL imag + → −; real − → +.
2. **`pack_add`** used unsigned carry as “new word.” Subtracting pitch from a
   signed origin became `1 × 2^12` (4096) or collapsed every seat. Window-glue
   and `ipp:0`. High limb must be **sign-extended**; keep when extra is `0` or
   `-1`.
3. **`mul`** unsigned 64×64 on a negative limb is not `z²` (debug overflow in
   the squeeze loop). `Words = 1` is signed `i128` product.

Admit was never the bug. Not WorkUpdate `c`. Not collector. Not center-δ.

Pins: `from_64bit_positive_mantissa_stays_positive`,
`headed_mag_43_get_c_unique_count_at_window_res`,
`add_negative_plus_small_positive_keeps_word`,
`add_two_negatives_keeps_word_and_exp`.
