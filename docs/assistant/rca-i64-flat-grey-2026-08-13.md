# RCA: i64 four-quadrant grey

**Locus.** Mag 43–44, `stack:i64` `mode:naive`. Open until the developer
says headed is fixed. Handoff: `docs/assistant/recontinuation-i64-grey.md`.

## What was measured (not guessed)

On `HEADED_I64_GREY_*` at `DEFAULT_WINDOW_RES` (854×480), `CGenerator<CopyIntExp<1>>`:

| Quantity | Before `pack_add` sign-ext guard | After (live tree) |
|---|---|---|
| Distinct `get_c` real along row 0 | 854 | 854 |
| Distinct `get_c` imag along seat 0 | **2** (row 0 vs row ≥ 1) | 480 |
| `get_c(0,1).im` | `1 × 2^12` = **4096** | pitch-step from origin |

Pins: `headed_mag_43_get_c_unique_count_at_window_res`,
`add_two_negatives_keeps_word_and_exp`,
`rca_from_64bit_positive_mantissa_sets_sign_bit`.

IntExp (infinite tape) neighbors at this locus are distinct. Admit of i64 is
still correct (54 bits needed, 64 claimed). Screen seats ≥ 0, UL origin. No
collector `c`. No signed center-δ.

## Cause 1 — `pack_add` treated sign-extension as a new word

`Add` is unsigned limb add. A negative `i64` limb plus a small pitch has
**carry 1** that is two’s-complement sign extension, not “needs another word.”

Old `if extra == 0` then `shr_one_word` + `exp += 64`:

- Origin imag after `From` had bit 63 set (signed negative).
- `origin.im − space × row` for `row ≥ 1` aligned to `exp = -52`.
- `pack_add` kept `value = 1`, `exp = -52 + 64 = 12` → **4096**.
- Every row except 0 used that imag. `|c| ≈ 4096` bails before iterate → HUD
  `ipp:0`. Drag: row index is from the window UL, so the field **stays on the
  window**.

That is the window-glue + `ipp:0` mechanism. It is **two** imag values (top
scanline vs the rest), not four equal quadrants. Live `pack_add` now accepts
`extra == sign_ext` (`copy_intexp.rs`). Do not call headed fixed.

## Cause 2 — `From<IntExp>` still writes 64 magnitude bits into a signed limb

`PRECISION.significand_bits` is `Words * 64`. `From` squeezes until
`significant_bits ≤ 64`, then `u64 as i64`. Bit 63 is the **sign**.

Witness: `+1.0` as `2^63 × 2^{-63}` becomes a negative `CopyIntExp`
(`rca_from_64bit_positive_mantissa_sets_sign_bit`).

Headed UL imag IntExp is **positive** (~1.09). After `From` it is **negative**.
UL real IntExp is negative (~−0.176); after `From` it is positive (~+0.074).
The 16×17 interior pin never required origin to match IntExp, only “some
escapes.”

This is still live. It does **not** by itself glue tiles to the window (origin
still tracks the view). It still paints the **wrong** `c`.

## Retracted

Collector `to_f64(c)`. False-admit. Negative / center screen offsets. “Uniform
flat grey.” Accidental f64 iterate on absolute naive (`DirectKernel::start_seat`
f64-roundtrips only if `coords_are_relative`).
