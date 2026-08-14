# Foundation Tracey rules (assistant-owned; not authoritative product spec)

Atomic rules for bottom-up units: IntExp/homothety, Range/calibration bias, NORES, Mandelbrot symmetry, off-screen geometry. Normative product text remains in `docs/authoritative/requirements.md` / design; these tags exist for Tracey linkage only.

> **2026-08-06 revert note.** Codebase is now v0.0.9 (e6a0560). Checked boxes earned on the tile machine must be re-earned against v0.0.9 symbols; per-rule notes below where the cited symbol changed. The product intent of every rule in this file still stands — these are foundation-level truths the golden design already embodies.

r[cz.math.intexp-add-commutative+1]

**Normative summary.** `IntExp` addition is commutative. `IntExp` has no infinities.

**Acceptance criteria.**
- [x] For arbitrary `a`, `b`, `a + b == b + a`.

r[cz.math.intexp-mul-associative+1]

**Normative summary.** `IntExp` multiplication is associative in typical coordinate ranges. `IntExp` has no infinities.

**Acceptance criteria.**
- [x] For arbitrary `a`, `b`, `c` within test bounds, `(a * b) * c == a * (b * c)`.

Defined in requirements.md: r[cz.math.mandelbrot-real-axis-symmetry+1]

**Normative summary.** Mandelbrot membership / escape classification is symmetric across the real axis for conjugate `c`.

**Acceptance criteria.**
- [x] For sampled `c`, conjugating imag preserves inside/outside and escape-time when finished.
- [x] After a published reference with series, a real-axis-centered package keeps conjugate
  seat pairs equal on classification, escape/period, and small_time
  (`home_package_with_live_series_obeys_real_axis_symmetry`).

r[cz.range.guess-biased-nearest+1]

**Normative summary.** Tile-era `Range::guess_biased` (clamp a bias into a proven range) is **not** a v0.0.9 product path. The live bar is PPS through whole-snapshot publish, not a calibrated-answer biasing synthesizer.

**Acceptance criteria.**
- [ ] Do **not** recreate `guess_biased` as a standing requirement. Unfinished seats stay `Dummy` (outside-looking), never invented Inside.
- Revert note: `src/range.rs` at v0.0.9 has `Range` but no `guess_biased`. That method was tile-machine. Skip it: bias would feel better on a slow pipe, but the benefit is too small to justify the complexity; when PPS is flowing it would not be noticed.

Defined in requirements.md: r[cz.display.nores-when-no-proximate+1]

**Normative summary.** Missing proximate work must surface as an outside-flavored
placeholder (escape after ~1 iteration, infinitely far), never as flat set-black.

**Acceptance criteria.**
- [x] Placeholder packs/paints as Outside, not missing/Inside.
- Revert note: the tile-era `NORES_ANSWER` constant is gone. The v0.0.9 symbol is
  `CompletedPoint::Dummy{}` (`workgroup/screen_worker/workshift.rs`), which the escaper
  (`shadergroup/escaper.rs`) paints as outside-looking. Principle unchanged; see
  `docs/assistant/collected-wisdom.md` ("Unknown is NORES, never black").

Defined in requirements.md: r[cz.display.window-default-800x480+1]

**Normative summary.** App defaults to 800×480 on startup and does not restore a customized size on launch.

**Acceptance criteria.**
- [ ] `DEFAULT_WINDOW_RES` matches the product default.
- Revert note: **discrepancy flagged** — v0.0.9 `constants.rs` has `(854, 480)`, this rule
  says 800×480. Resolve with developer (code change vs rule text change) before re-checking.

Defined in requirements.md: r[cz.display.offscreen-r2-circle+1]

**Normative summary.** Off-screen guidance uses the r=2 circle: fully off, mostly off (within 10% of fully off), too small (≤1px), mostly too small (<10% of screen).

**Acceptance criteria.**
- [ ] Classifier returns the four states from stencil geometry vs unit circle radius 2.
- Revert note: **feature absent at v0.0.9** — the classifier (`headgroup/window/offscreen.rs`)
  was tile-era. The product rule stands; the implementation needs porting to the restored
  headgroup.

r[cz.math.copy-intexp-add-squeeze+1]

**Normative summary.** `CopyIntExp` add cannot grow the mantissa. It aligns to the
coarser exp by right-shifting the finer operand (low bits dropped), then if an
extra carry word is used, shifts one word right and adds 64 to `exp`. Commutative.
No infinities.

**Acceptance criteria.**
- [x] `a + b == b + a`.
- [x] Result matches unbounded `IntExp` add after the same squeeze (round to coarser
  exp, then round 64-bit words until the value fits `Words` limbs).
- [x] Finer + coarser keeps the coarser exp.
- [x] Carry 1 with a negative high limb is sign extension, not a new word.

**Implementation.** `src/copy_intexp.rs` `Add`.
**Verification.** `add_commutative`, `add_matches_squeezed_intexp`,
`add_squeezes_to_coarser_exp`, `sub_is_add_of_neg`, `neg_is_involution`,
`add_two_negatives_keeps_word_and_exp`,
`add_negative_plus_small_positive_keeps_word`,
`headed_mag_43_get_c_unique_count_at_window_res`.

r[cz.math.copy-intexp-mul-schoolbook+1]

**Normative summary.** `Words = 1` mul is a signed `i128` product, then
minimum bit-shifts until the product fits a signed `i64` limb (not a 64-bit
dump). Wider tapes stay schoolbook into `2×Words` limbs. Commutative. Not
recursive.

**Acceptance criteria.**
- [x] `a * b == b * a`.
- [x] Result matches unbounded `IntExp` mul after fitting to `Words×64` bits.
- [x] A product that needs another word on a wide tape raises `exp` by 64 and
  keeps the high limb. `Words = 1` raises `exp` by the bit count that fits
  the signed limb.
- [x] A Mandelbrot orbit at the headed mag-44 sample tracks f64 for tens of
  iterates (`copy_intexp1_mandel_orbit_tracks_f64_at_headed_c`).

**Implementation.** `src/copy_intexp.rs` `Mul`.
**Verification.** `mul_commutative`, `mul_matches_squeezed_intexp`,
`mul_schoolbook_fits_in_words`, `mul_high_half_shifts_exp`,
`copy_intexp1_mandel_orbit_tracks_f64_at_headed_c`.

r[cz.math.copy-intexp-from-tape+1]

**Normative summary.** `From<IntExp>` copies the absolute mantissa into the
fixed window. Squeeze until `significant_bits ≤ Words×64 − 1` (signed limb),
then two’s-complement if the source is negative. Never panic on a too-wide
source. A 64-bit magnitude must not be stored with `u64 as i64`.

**Acceptance criteria.**
- [x] Values that fit in 63 magnitude bits round-trip through `CopyIntExp`.
- [x] A mantissa wider than `Words×64` bits still converts (squeeze, no panic).
- [x] 64-bit positive magnitude stays a positive limb (`2^63 × 2^{-63}` → `+1`).

**Implementation.** `CopyIntExp::from`.
**Verification.** `from_intexp_roundtrips_when_it_fits`,
`from_squeezes_mantissa_wider_than_tape`,
`from_64bit_positive_mantissa_stays_positive`,
`headed_mag_43_get_c_unique_count_at_window_res`.

r[cz.math.copy-intexp-no-infinity+1]

**Normative summary.** `CopyIntExp` cannot represent infinities. Every value is
finite. `Mandelbrotable::is_finite` is constantly true. Squeeze never produces Inf.

**Acceptance criteria.**
- [x] Arbitrary values, `ZERO`, `max_value`, and finite `from_f32` are finite.
- [x] `Ord` agrees with the sign of `a - b` after squeeze subtract.

**Implementation.** `Mandelbrotable` for `CopyIntExp`.
**Verification.** `every_value_is_finite`, `never_infinite`, `ord_is_total`.

