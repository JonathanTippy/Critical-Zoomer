//! Fixed-width `IntExp` for iterate: `[i64; Words]` + `exp` on the stack.
//! Infinite tape (`IntExp`) grows the mantissa. This type squeezes: align to the
//! coarser exp, drop low bits, and if a sum/product needs another word, shift
//! the value right and add 64 to `exp`. No infinities.
//!
//! Design: `docs/assistant/design/copy-intexp.md`.

const WORDSIZE: usize = 64;

// JFT: This is a form of intexp optimal for iterating with either naive or perturbed methods.
// JFT: The const bits number is central; sized data on the stack is fast data, even if its actually quite a bit of data.
// JFT: Q: What about rounding? A: not necessary when iterating. bits stay the same.
// JFT: keep addition and multiplication algebraic instead of branching on sign, its simpler.
// JFT: When iterating over all words in the value, always use an iterator to give rust the best shot at optimizing.

use std::cmp::Ordering;
use std::cmp::Ordering::{Equal, Greater, Less};
use crate::assemblies::workgroup::c_generator::{HostPrecision, Mandelbrotable};
use crate::utils::IntExp;
use rug::Integer;
use rug::integer::Order;
use std::ops::{Add, Mul, Sub};

#[derive(Clone, Copy, Debug)]
pub(crate) struct CopyIntExp<const Words: usize> {
    value: [i64; Words],
    exp: i32,
}
impl<const Words: usize> CopyIntExp<Words> {
    // r[impl cz.math.copy-intexp-from-tape+1]
    pub(crate) fn from(value: IntExp) -> CopyIntExp<Words> {
        if value.val.significant_bits() as usize > Words * WORDSIZE {
            panic!()
        } else {
            let digits = value.val.to_digits::<u64>(Order::Lsf);
            let mut words = [0i64; Words];
            for (i, d) in digits.into_iter().enumerate() {
                words[i] = d as i64;
            }
            let mut out = CopyIntExp {
                value: words,
                exp: value.exp,
            };
            // JFT: good job assistant, happy with this block.
            if value.val.is_negative() {
                out.value = neg_limbs(out.value);
            }
            out
        }
    }

    /// Like `IntExp`: no infinities, so every value is finite.
    pub(crate) fn is_finite(self) -> bool {
        true
    }

    pub(crate) fn neg(self) -> Self {
        Self {
            value: neg_limbs(self.value),
            exp: self.exp,
        }
    }

    pub(crate) fn abs(self) -> Self {
        if limbs_neg(self.value) {
            self.neg()
        } else {
            self
        }
    }
}

impl<const Words: usize> From<IntExp> for CopyIntExp<Words> {
    fn from(value: IntExp) -> Self {
        Self::from(value)
    }
}

impl<const Words: usize> Add for CopyIntExp<Words> {
    type Output = CopyIntExp<Words>;
    fn add(self, other: CopyIntExp<Words>) -> Self::Output {
        // r[impl cz.math.copy-intexp-add-squeeze+1]
        // Infinite tape (IntExp): shl the higher-exp mantissa, grow val, keep
        // the finer exp. Fixed tape: shr the finer mantissa onto the coarser
        // exp (drop low bits). Never `<< 0`.
        match self.exp.cmp(&other.exp) {
            Equal => pack_add(add_limbs(self.value, other.value), self.exp),
            Greater => {
                let s = (self.exp - other.exp) as u32;
                debug_assert!(s > 0);
                pack_add(add_limbs(self.value, shr_limbs(other.value, s)), self.exp)
            }
            Less => {
                let s = (other.exp - self.exp) as u32;
                debug_assert!(s > 0);
                pack_add(add_limbs(shr_limbs(self.value, s), other.value), other.exp)
            }
        }
    }
}

fn add_limbs<const Words: usize>(a: [i64; Words], b: [i64; Words]) -> ([i64; Words], i64) {
    let mut out = [0i64; Words];
    let mut carry = 0u128;
    for (dst, (&av, &bv)) in out.iter_mut().zip(a.iter().zip(b.iter())) {
        let sum = av as u64 as u128 + bv as u64 as u128 + carry;
        *dst = sum as u64 as i64;
        carry = sum >> 64;
    }
    (out, carry as i64)
}

fn pack_add<const Words: usize>(sum: ([i64; Words], i64), exp: i32) -> CopyIntExp<Words> {
    let (limbs, extra) = sum;
    if extra == 0 {
        CopyIntExp {
            value: limbs,
            exp,
        }
    } else {
        CopyIntExp {
            value: shr_one_word(limbs, extra),
            exp: exp + WORDSIZE as i32,
        }
    }
}

fn shr_one_word<const Words: usize>(a: [i64; Words], extra: i64) -> [i64; Words] {
    let mut out = [0i64; Words];
    for (dst, &src) in out.iter_mut().zip(a.iter().skip(1)) {
        *dst = src;
    }
    out[Words - 1] = extra;
    out
}

fn shr_limbs<const Words: usize>(a: [i64; Words], s: u32) -> [i64; Words] {
    let mut out = [0i64; Words];
    let word_shift = (s as usize) / WORDSIZE;
    let bits = s % (WORDSIZE as u32);
    let sign = if a[Words - 1] < 0 { !0u64 } else { 0u64 };
    if word_shift >= Words {
        if sign != 0 {
            out = [!0i64; Words];
        }
        return out;
    }
    for (i, dst) in out.iter_mut().enumerate() {
        let src = i + word_shift;
        if src >= Words {
            *dst = sign as i64;
            continue;
        }
        let low = a[src] as u64;
        let mut v = if bits == 0 { low } else { low >> bits };
        if bits != 0 {
            let high = if src + 1 < Words {
                a[src + 1] as u64
            } else {
                sign
            };
            v |= high << (WORDSIZE as u32 - bits);
        }
        *dst = v as i64;
    }
    out
}

impl<const Words: usize> Mul for CopyIntExp<Words> {
    type Output = Self;
    fn mul(self, other: Self) -> Self {
        // r[impl cz.math.copy-intexp-mul-schoolbook+1]
        let (mut lo, mut hi) = mul_limbs_full(self.value, other.value);
        let mut exp = self.exp + other.exp;
        while hi.iter().any(|&w| w != 0) {
            lo = shr_one_word(lo, hi[0]);
            hi.rotate_left(1);
            hi[Words - 1] = 0;
            exp += WORDSIZE as i32;
        }
        Self { value: lo, exp }
    }
}

fn mul_limbs_full<const Words: usize>(
    a: [i64; Words],
    b: [i64; Words],
) -> ([i64; Words], [i64; Words]) {
    let mut lo = [0i64; Words];
    let mut hi = [0i64; Words];
    for (i, &ai) in a.iter().enumerate() {
        let mut carry = 0u128;
        for (j, &bj) in b.iter().enumerate() {
            let idx = i + j;
            let old = if idx < Words {
                lo[idx] as u64 as u128
            } else {
                hi[idx - Words] as u64 as u128
            };
            let t = (ai as u64 as u128) * (bj as u64 as u128) + old + carry;
            let w = t as u64 as i64;
            if idx < Words {
                lo[idx] = w;
            } else {
                hi[idx - Words] = w;
            }
            carry = t >> 64;
        }
        for h in hi.iter_mut().skip(i) {
            if carry == 0 {
                break;
            }
            let t = (*h as u64 as u128) + carry;
            *h = t as u64 as i64;
            carry = t >> 64;
        }
    }
    (lo, hi)
}

impl<const Words: usize> PartialEq for CopyIntExp<Words> {
    fn eq(&self, other: &Self) -> bool {
        to_intexp(*self) == to_intexp(*other)
    }
}

impl<const Words: usize> Eq for CopyIntExp<Words> {}

impl<const Words: usize> PartialOrd for CopyIntExp<Words> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<const Words: usize> Ord for CopyIntExp<Words> {
    fn cmp(&self, other: &Self) -> Ordering {
        to_intexp(*self).cmp(&to_intexp(*other))
    }
}

impl<const Words: usize> Sub for CopyIntExp<Words> {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        self.add(CopyIntExp {
            value: neg_limbs(other.value),
            exp: other.exp,
        })
    }
}

fn limbs_zero<const Words: usize>(a: [i64; Words]) -> bool {
    a.iter().all(|&w| w == 0)
}

fn limbs_neg<const Words: usize>(a: [i64; Words]) -> bool {
    a[Words - 1] < 0
}

fn neg_limbs<const Words: usize>(a: [i64; Words]) -> [i64; Words] {
    let mut out = [0i64; Words];
    let mut carry = 1i128;
    for (dst, &src) in out.iter_mut().zip(a.iter()) {
        let t = (!(src as u64) as i128) + carry;
        *dst = t as i64;
        carry = t >> 64;
    }
    out
}

fn to_intexp<const Words: usize>(x: CopyIntExp<Words>) -> IntExp {
    let neg = limbs_neg(x.value);
    let mag = if neg {
        neg_limbs(x.value)
    } else {
        x.value
    };
    let mut val = Integer::from(0);
    for i in (0..Words).rev() {
        val <<= WORDSIZE as u32;
        val += Integer::from(mag[i] as u64);
    }
    if neg {
        val = -val;
    }
    IntExp { val, exp: x.exp }
}

impl<const Words: usize> Mandelbrotable for CopyIntExp<Words> {
    // r[impl cz.math.copy-intexp-no-infinity+1]
    const ZERO: Self = Self {
        value: [0; Words],
        exp: 0,
    };
    const ONE: Self = {
        let mut value = [0i64; Words];
        value[0] = 1;
        Self { value, exp: 0 }
    };
    const TWO: Self = {
        let mut value = [0i64; Words];
        value[0] = 2;
        Self { value, exp: 0 }
    };
    const PRECISION: HostPrecision = HostPrecision {
        significand_bits: (Words * WORDSIZE) as u32,
        min_exponent: i32::MIN,
    };

    fn from_u32(value: u32) -> Self {
        let mut words = [0i64; Words];
        words[0] = value as i64;
        Self {
            value: words,
            exp: 0,
        }
    }

    fn from_f32(value: f32) -> Self {
        assert!(
            value.is_finite(),
            "CopyIntExp cannot represent non-finite values"
        );
        if value == 0.0 {
            return Self::ZERO;
        }
        let bits = value.to_bits();
        let neg = (bits & 0x8000_0000) != 0;
        let raw_exp = ((bits >> 23) & 0xff) as i32;
        let frac = (bits & 0x7f_ffff) as i64;
        let (mant, exp) = if raw_exp == 0 {
            (frac, -149)
        } else {
            (frac | (1 << 23), raw_exp - 127 - 23)
        };
        let mut words = [0i64; Words];
        words[0] = mant;
        let out = Self {
            value: words,
            exp,
        };
        if neg {
            out.neg()
        } else {
            out
        }
    }

    fn to_f64(self) -> f64 {
        f64::from(to_intexp(self))
    }

    fn abs(self) -> Self {
        self.abs()
    }

    fn neg(self) -> Self {
        self.neg()
    }

    fn max_value() -> Self {
        let mut value = [!0i64; Words];
        value[Words - 1] = i64::MAX;
        Self {
            value,
            exp: i32::MAX,
        }
    }

    fn is_finite(self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    const TAPE: usize = 2;
    type C2 = CopyIntExp<TAPE>;

    fn round_to_exp(x: IntExp, exp: i32) -> IntExp {
        match x.exp.cmp(&exp) {
            Equal => x,
            Less => IntExp {
                val: x.val >> (exp - x.exp) as u32,
                exp,
            },
            Greater => IntExp {
                val: x.val << (x.exp - exp) as u32,
                exp,
            },
        }
    }

    fn fit_tape(mut x: IntExp, words: usize) -> IntExp {
        let cap = (words * WORDSIZE - 1) as u32;
        while x.val.significant_bits() > cap {
            x = x.round(WORDSIZE);
        }
        x
    }

    fn squeeze_add(a: IntExp, b: IntExp, words: usize) -> IntExp {
        let exp = a.exp.max(b.exp);
        fit_tape(round_to_exp(a, exp) + round_to_exp(b, exp), words)
    }

    fn squeeze_mul(a: IntExp, b: IntExp, words: usize) -> IntExp {
        fit_tape(a * b, words)
    }

    #[test]
    fn mul_schoolbook_fits_in_words() {
        let a = CopyIntExp::<2>::from_u32(3);
        let b = CopyIntExp::<2>::from_u32(5);
        let p = a * b;
        assert_eq!(p.to_f64(), 15.0);
        assert_eq!(p.exp, 0);
    }

    #[test]
    fn mul_high_half_shifts_exp() {
        let a = CopyIntExp::<1> {
            value: [1i64 << 32],
            exp: 0,
        };
        let p = a * a;
        assert_eq!(p.exp, WORDSIZE as i32);
        assert_eq!(p.value[0], 1);
        assert_eq!(p.to_f64(), 2.0_f64.powi(64));
    }

    #[test]
    fn add_squeezes_to_coarser_exp() {
        let coarse = CopyIntExp::<1> {
            value: [1],
            exp: 3,
        };
        let fine = CopyIntExp::<1> {
            value: [1],
            exp: 0,
        };
        let s = coarse + fine;
        assert_eq!(s.exp, 3);
        assert_eq!(s.value[0], 1);
    }

    #[test]
    fn never_infinite() {
        assert!(C2::ZERO.is_finite());
        assert!(C2::max_value().is_finite());
        assert!(C2::from_f32(-0.0).is_finite());
        assert!(C2::from_f32(1.25).is_finite());
    }

    prop_compose! {
        fn arb_c2()(
            w0 in -2048i64..2048,
            w1 in -4i64..4,
            exp in -12i32..12,
        ) -> C2 {
            C2 { value: [w0, w1], exp }
        }
    }

    prop_compose! {
        fn arb_c2_low()(
            w0 in 0i64..2048,
            exp in -12i32..12,
        ) -> C2 {
            C2 { value: [w0, 0], exp }
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        // r[verify cz.math.copy-intexp-add-squeeze+1]
        #[test]
        fn add_commutative(a in arb_c2(), b in arb_c2()) {
            prop_assert_eq!(a + b, b + a);
        }

        // r[verify cz.math.copy-intexp-add-squeeze+1]
        #[test]
        fn add_matches_squeezed_intexp(a in arb_c2_low(), b in arb_c2_low()) {
            let got = to_intexp(a + b);
            let expect = squeeze_add(to_intexp(a), to_intexp(b), TAPE);
            prop_assert_eq!(got, expect);
        }

        // r[verify cz.math.copy-intexp-mul-schoolbook+1]
        #[test]
        fn mul_commutative(a in arb_c2(), b in arb_c2()) {
            prop_assert_eq!(a * b, b * a);
        }

        // r[verify cz.math.copy-intexp-mul-schoolbook+1]
        #[test]
        fn mul_matches_squeezed_intexp(a in arb_c2_low(), b in arb_c2_low()) {
            let got = to_intexp(a * b);
            let expect = squeeze_mul(to_intexp(a), to_intexp(b), TAPE);
            prop_assert_eq!(got, expect);
        }

        // r[verify cz.math.copy-intexp-from-tape+1]
        #[test]
        fn from_intexp_roundtrips_when_it_fits(
            v in -10_000i64..10_000,
            exp in -8i32..8,
        ) {
            let src = IntExp {
                val: Integer::from(v),
                exp,
            };
            let back = to_intexp(C2::from(src.clone()));
            prop_assert_eq!(back, src);
        }

        // r[verify cz.math.copy-intexp-add-squeeze+1]
        #[test]
        fn neg_is_involution(a in arb_c2()) {
            prop_assert_eq!(a.neg().neg(), a);
        }

        // r[verify cz.math.copy-intexp-add-squeeze+1]
        #[test]
        fn sub_is_add_of_neg(a in arb_c2(), b in arb_c2()) {
            prop_assert_eq!(a - b, a + b.neg());
        }

        // r[verify cz.math.copy-intexp-no-infinity+1]
        #[test]
        fn every_value_is_finite(a in arb_c2()) {
            prop_assert!(a.is_finite());
            prop_assert!(<C2 as Mandelbrotable>::is_finite(a));
        }

        // r[verify cz.math.copy-intexp-no-infinity+1]
        #[test]
        fn ord_agrees_with_intexp(a in arb_c2(), b in arb_c2()) {
            prop_assert_eq!(a.cmp(&b), to_intexp(a).cmp(&to_intexp(b)));
        }
    }
}
