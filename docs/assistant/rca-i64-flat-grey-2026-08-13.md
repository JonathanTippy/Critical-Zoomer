# RCA: i64 four-quadrant grey

**Locus.** Mag 43–44, `type:i64` `mode:naive`. **Headed fixed** (developer
2026-08-13).

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
3. **`mul`** unsigned 64×64 on a negative limb is not `z²`. `Words = 1` is
   signed `i128` product.
4. **Relative naive `start_seat`** rebuilt `c` as f64(anchor)+f64(δc). Mag 44
   pitch is one ulp of `|c|~1` → one plane `c`. Headed logs: `uniq_c` then 5
   after T-add.
5. **`Words = 1` mul squeezed with `>> 64`** whenever the product missed
   `i64`. After a few `z²` the mantissa was a few bits (`n=6` val=-14, exp=-6).
   Every seat escaped at **7**. Mag 43 f64 still `it0=159`. Fix: shift **one
   bit at a time** until the product fits.

Admit was never the bug. Not WorkUpdate `c`. Not collector. Not center-δ.

Pins: `from_64bit_positive_mantissa_stays_positive`,
`headed_mag_43_get_c_unique_count_at_window_res`,
`add_negative_plus_small_positive_keeps_word`,
`add_two_negatives_keeps_word_and_exp`,
`relative_copy_intexp1_mag_44_does_not_f64_collapse_c`,
`copy_intexp1_mandel_orbit_tracks_f64_at_headed_c`.
