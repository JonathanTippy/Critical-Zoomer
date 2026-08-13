# Recontinuation — i64 four-quadrant grey

**Closed** (developer 2026-08-13). See `docs/assistant/rca-i64-flat-grey-2026-08-13.md`.

Loci: `HEADED_I64_GREY_*` and the nearby mag 43/44 points in
`docs/assistant/rca-i64-flat-grey-2026-08-13.md`.

## What landed in lib

- `From`: squeeze to 63 magnitude bits, abs digits, then negate.
- Add: sign-extend the high limb; keep when extra is `0` or `-1`.
- Mul `Words = 1`: signed `i128` product.

## Binding

Admit is correct. Screen seats ≥ 0, UL, +right/+down. Worker does not emit `c`.
Mag-38 black was OG f64; do not re-break it.

## Retracted

Collector `to_f64(c)`. False-admit. Negative / center screen offsets. Accidental
f64 iterate on absolute naive.
