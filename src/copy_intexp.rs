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
// JFT: When iterating over all words in the value, always use an iterator to give rust the best shot at optimizing.
// JFT: In this code, its just addition, multiplication, and shifts. no slow ops.
// JFT: When branching, first ask if you can avoid it.
// JFT: if you can't, ensure the happy path is immensely more common than the unhappy path.
// JFT: May need to invert the order of some logic, this is ok.

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

/// One-word tape for OG naive after f64 absolute admit fails.
pub(crate) type CopyIntExp1 = CopyIntExp<1>;
impl<const Words: usize> CopyIntExp<Words> {
    // r[impl cz.math.copy-intexp-from-tape+1]
    pub(crate) fn from(value: IntExp) -> CopyIntExp<Words> {
        // Signed two's-complement tape: high bit of the last limb is sign.
        // `Words*64` magnitude bits do not fit (`u64 as i64` steals the sign).
        let cap = (Words * WORDSIZE - 1) as u32;
        let mut value = value;
        while value.val.significant_bits() > cap {
            let extra = (value.val.significant_bits() - cap) as usize;
            value = value.round(extra.max(1));
        }
        let neg = value.val.is_negative();
        let mag = value.val.abs();
        let digits = mag.to_digits::<u64>(Order::Lsf);
        let mut words = [0i64; Words];
        for (i, d) in digits.into_iter().take(Words).enumerate() {
            words[i] = d as i64;
        }
        let mut out = CopyIntExp {
            value: words,
            exp: value.exp,
        };
        // JFT: good job assistant, happy with this block.
        if neg {
            out.value = neg_limbs(out.value);
        }
        out
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
    let mut carry = 0i128;
    for i in 0..Words {
        let last = i + 1 == Words;
        // Lower limbs: unsigned 64-bit digits. High limb: sign-extended.
        let av = if last { a[i] as i128 } else { a[i] as u64 as i128 };
        let bv = if last { b[i] as i128 } else { b[i] as u64 as i128 };
        let sum = av + bv + carry;
        out[i] = sum as i64;
        carry = sum >> 64;
    }
    (out, carry as i64)
}

fn pack_add<const Words: usize>(sum: ([i64; Words], i64), exp: i32) -> CopyIntExp<Words> {
    let (limbs, extra) = sum;
    // High word of a signed 2×width add is 0 or -1 when the sum fits.
    let expected = if limbs[Words - 1] < 0 { -1i64 } else { 0 };
    if extra == expected {
        CopyIntExp {
            value: limbs,
            exp,
        }
    } else {
        CopyIntExp {
            value: shr_one_word(limbs, extra),
            exp: exp.saturating_add(WORDSIZE as i32),
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
    if s == 0 {
        return a;
    }
    let neg = limbs_neg(a);
    let mag = if neg { neg_limbs(a) } else { a };
    let mut out = [0i64; Words];
    let word_shift = (s as usize) / WORDSIZE;
    let bits = s % (WORDSIZE as u32);
    if word_shift >= Words {
        return out;
    }
    for (i, dst) in out.iter_mut().enumerate() {
        let src = i + word_shift;
        if src >= Words {
            *dst = 0;
            continue;
        }
        let low = mag[src] as u64;
        let mut v = if bits == 0 { low } else { low >> bits };
        if bits != 0 {
            let high = if src + 1 < Words {
                mag[src + 1] as u64
            } else {
                0
            };
            v |= high << (WORDSIZE as u32 - bits);
        }
        *dst = v as i64;
    }
    if neg && !limbs_zero(out) {
        neg_limbs(out)
    } else {
        out
    }
}

impl<const Words: usize> Mul for CopyIntExp<Words> {
    type Output = Self;
    fn mul(self, other: Self) -> Self {
        // r[impl cz.math.copy-intexp-mul-schoolbook+1]
        if Words == 1 {
            let mut p = (self.value[0] as i128) * (other.value[0] as i128);
            let mut exp = self.exp.saturating_add(other.exp);
            while p > i64::MAX as i128 || p < i64::MIN as i128 {
                p >>= 1;
                exp = exp.saturating_add(1);
            }
            let mut value = [0i64; Words];
            value[0] = p as i64;
            return Self { value, exp };
        }
        let (lo, hi) = mul_limbs_full(self.value, other.value);
        let exp = self.exp.saturating_add(other.exp);
        if hi.iter().all(|&w| w == 0) {
            return Self { value: lo, exp };
        }
        let mut lo = lo;
        let mut hi = hi;
        let mut exp = exp;
        loop {
            lo = shr_one_word(lo, hi[0]);
            hi.rotate_left(1);
            hi[Words - 1] = 0;
            exp = exp.saturating_add(WORDSIZE as i32);
            if hi.iter().all(|&w| w == 0) {
                return Self { value: lo, exp };
            }
        }
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
            // 64×64 unsigned product is up to 128 bits; i128 only holds 127.
            let t = (ai as u64 as u128) * (bj as u64 as u128) + old + carry;
            let w = t as i64;
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
            *h = t as i64;
            carry = t >> 64;
        }
    }
    (lo, hi)
}

impl<const Words: usize> PartialEq for CopyIntExp<Words> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
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
        // r[impl cz.math.copy-intexp-no-infinity+2]
        // Aligning a sub-unit onto ZERO (exp 0) used to shift the mantissa
        // away and call |c|<1 equal to 0. Period check then marked every
        // interior-looking seat a repeat (headed mag 44 full black, ipp:1).
        let az = limbs_zero(self.value);
        let bz = limbs_zero(other.value);
        match (az, bz) {
            (true, true) => Ordering::Equal,
            (true, false) => {
                if limbs_neg(other.value) {
                    Ordering::Greater
                } else {
                    Ordering::Less
                }
            }
            (false, true) => {
                if limbs_neg(self.value) {
                    Ordering::Less
                } else {
                    Ordering::Greater
                }
            }
            (false, false) => match self.exp.cmp(&other.exp) {
                Equal => cmp_limbs(self.value, other.value),
                Greater => cmp_after_shr(
                    self.value,
                    other.value,
                    (self.exp - other.exp) as u32,
                ),
                Less => cmp_after_shr(
                    other.value,
                    self.value,
                    (other.exp - self.exp) as u32,
                )
                .reverse(),
            },
        }
    }
}

fn cmp_after_shr<const Words: usize>(
    coarse: [i64; Words],
    fine: [i64; Words],
    s: u32,
) -> Ordering {
    let shifted = shr_limbs(fine, s);
    if limbs_zero(shifted) {
        cmp_limbs(coarse, [0; Words])
    } else {
        cmp_limbs(coarse, shifted)
    }
}

fn cmp_limbs<const Words: usize>(a: [i64; Words], b: [i64; Words]) -> Ordering {
    let mut limbs = a.iter().rev().zip(b.iter().rev());
    let Some((&ha, &hb)) = limbs.next() else {
        return Ordering::Equal;
    };
    match ha.cmp(&hb) {
        Ordering::Equal => {}
        o => return o,
    }
    for (&x, &y) in limbs {
        match (x as u64).cmp(&(y as u64)) {
            Ordering::Equal => {}
            o => return o,
        }
    }
    Ordering::Equal
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
    // r[impl cz.math.copy-intexp-no-infinity+2]
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
    // r[verify cz.math.copy-intexp-mul-schoolbook+1]
    fn mul_high_half_shifts_exp() {
        let a = CopyIntExp::<1> {
            value: [1i64 << 32],
            exp: 0,
        };
        let p = a * a;
        // 2^64 does not fit in a signed limb: shift 2 bits, not a whole word.
        assert_eq!(p.exp, 2);
        assert_eq!(p.value[0], 1i64 << 62);
        assert_eq!(p.to_f64(), 2.0_f64.powi(64));
    }

    #[test]
    // r[verify cz.math.copy-intexp-mul-schoolbook+1]
    fn copy_intexp1_mandel_orbit_tracks_f64_at_mag_44_black_locus() {
        let cre = -0.6487374290236704_f64;
        let cim = 0.374687166530634_f64;
        let c = (
            CopyIntExp1::from(crate::assemblies::headgroup::window::coords::f64_to_intexp(cre)),
            CopyIntExp1::from(crate::assemblies::headgroup::window::coords::f64_to_intexp(cim)),
        );
        let r2 = CopyIntExp1::from_f32(4.0);
        let mut z = c;
        let mut zf = (cre, cim);
        let mut t_esc = None;
        let mut f_esc = None;
        for n in 1..=4000u32 {
            let re2 = z.0 * z.0;
            let im2 = z.1 * z.1;
            let ri = z.0 * z.1;
            z = (re2 - im2 + c.0, CopyIntExp1::TWO * ri + c.1);
            zf = (zf.0 * zf.0 - zf.1 * zf.1 + cre, 2.0 * zf.0 * zf.1 + cim);
            let rad_t = (z.0 * z.0 + z.1 * z.1).to_f64();
            let rad_f = zf.0 * zf.0 + zf.1 * zf.1;
            if t_esc.is_none() && rad_t > r2.to_f64() {
                t_esc = Some(n);
            }
            if f_esc.is_none() && rad_f > 4.0 {
                f_esc = Some(n);
            }
            if n <= 80 {
                let zt = (z.0.to_f64(), z.1.to_f64());
                assert!(
                    (zt.0 - zf.0).abs() < 1e-6 && (zt.1 - zf.1).abs() < 1e-6,
                    "n={n} zT=({:.8},{:.8}) zf=({:.8},{:.8})",
                    zt.0,
                    zt.1,
                    zf.0,
                    zf.1
                );
            }
            if t_esc.is_some() && f_esc.is_some() {
                break;
            }
        }
        assert!(
            t_esc.is_some() && f_esc.is_some(),
            "i64 escape {t_esc:?} vs f64 {f_esc:?} (high-IPP black if i64 never escapes)"
        );
    }

    #[test]
    // r[verify cz.math.copy-intexp-mul-schoolbook+1]
    fn copy_intexp1_mandel_orbit_tracks_f64_at_headed_c() {
        let cre = -0.1146689120911964_f64;
        let cim = 0.9695042757337979_f64;
        let c = (
            CopyIntExp1::from(crate::assemblies::headgroup::window::coords::f64_to_intexp(cre)),
            CopyIntExp1::from(crate::assemblies::headgroup::window::coords::f64_to_intexp(cim)),
        );
        let mut z = c;
        let mut zf = (cre, cim);
        for n in 1..=40u32 {
            let re2 = z.0 * z.0;
            let im2 = z.1 * z.1;
            let ri = z.0 * z.1;
            z = (re2 - im2 + c.0, CopyIntExp1::TWO * ri + c.1);
            zf = (zf.0 * zf.0 - zf.1 * zf.1 + cre, 2.0 * zf.0 * zf.1 + cim);
            let zt = (z.0.to_f64(), z.1.to_f64());
            assert!(
                (zt.0 - zf.0).abs() < 1e-8 && (zt.1 - zf.1).abs() < 1e-8,
                "n={n} zT=({:.6},{:.6}) zf=({:.6},{:.6})",
                zt.0,
                zt.1,
                zf.0,
                zf.1
            );
        }
    }

    #[test]
    // r[verify cz.math.copy-intexp-from-tape+1]
    fn from_squeezes_mantissa_wider_than_tape() {
        let src = IntExp {
            val: (Integer::from(1) << 90) + Integer::from(123),
            exp: -80,
        };
        let c = CopyIntExp::<1>::from(src);
        assert!(c.is_finite());
        assert!(c.value[0] != 0 || c.exp != -80);
    }

    #[test]
    // r[verify cz.depth.c-generator-fails-closed+1]
    // r[verify cz.math.copy-intexp-add-squeeze+1]
    fn headed_mag_43_get_c_unique_count_at_window_res() {
        use crate::assemblies::headgroup::window::coords::{decimal_str_to_intexp, ul_for_center};
        use crate::assemblies::workgroup::c_generator::CGenerator;
        use crate::constants::{
            DEFAULT_WINDOW_RES, HEADED_I64_GREY_IM, HEADED_I64_GREY_MAG, HEADED_I64_GREY_RE,
        };
        let loc = ul_for_center(
            decimal_str_to_intexp(HEADED_I64_GREY_RE).unwrap(),
            decimal_str_to_intexp(HEADED_I64_GREY_IM).unwrap(),
            HEADED_I64_GREY_MAG,
            DEFAULT_WINDOW_RES,
        );
        let compute = (
            loc.pos.0.clone(),
            crate::utils::IntExp::ZERO - loc.pos.1.clone(),
        );
        let gen = CGenerator::<CopyIntExp1>::new_with_margin(
            &compute,
            HEADED_I64_GREY_MAG as i64,
            DEFAULT_WINDOW_RES,
            1,
        )
        .expect("i64 admits mag 43 window");
        let w = DEFAULT_WINDOW_RES.0;
        let h = DEFAULT_WINDOW_RES.1;
        let mut eq_re = 1u32;
        let mut last = gen.get_c((0, 0)).0;
        for seat in 1..w {
            let v = gen.get_c((seat, 0)).0;
            if v != last {
                eq_re += 1;
                last = v;
            }
        }
        let mut eq_im = 1u32;
        let mut last = gen.get_c((0, 0)).1;
        for row in 1..h {
            let v = gen.get_c((0, row)).1;
            if v != last {
                eq_im += 1;
                last = v;
            }
        }
        assert_eq!(eq_re, w, "pixel re must stay distinct");
        assert_eq!(eq_im, h, "pixel im must stay distinct (pack_add sign-ext)");
        let compute_im_f = f64::from(compute.1.clone());
        let from_im_f = CopyIntExp1::from(compute.1.clone()).to_f64();
        assert!(
            compute_im_f > 0.0,
            "UL imag IntExp is + (~1.09), got {compute_im_f}"
        );
        assert!(
            from_im_f > 0.0,
            "From must keep headed UL imag positive ({from_im_f})"
        );
        let rel = (from_im_f - compute_im_f).abs() / compute_im_f.abs();
        assert!(
            rel < 1e-12,
            "From imag {from_im_f} vs IntExp {compute_im_f} rel={rel}"
        );
    }

    #[test]
    // r[verify cz.math.copy-intexp-from-tape+1]
    fn from_64bit_positive_mantissa_stays_positive() {
        let src = IntExp {
            val: Integer::from(1) << 63,
            exp: -63,
        };
        assert!(!src.val.is_negative());
        assert_eq!(src.val.significant_bits(), 64);
        assert!((f64::from(src.clone()) - 1.0).abs() < 1e-15);
        let c = CopyIntExp::<1>::from(src);
        assert!(c.value[0] > 0, "2^63 must squeeze into a positive limb ({})", c.value[0]);
        assert!((c.to_f64() - 1.0).abs() < 1e-15, "+1.0 From became {}", c.to_f64());
        let neg = IntExp {
            val: {
                let mut v = Integer::from(1);
                v <<= 63;
                -v
            },
            exp: -63,
        };
        let n = CopyIntExp::<1>::from(neg);
        assert!(n.value[0] < 0);
        assert!((n.to_f64() + 1.0).abs() < 1e-15);
    }

    #[test]
    // r[verify cz.math.copy-intexp-add-squeeze+1]
    fn add_two_negatives_keeps_word_and_exp() {
        let a = CopyIntExp::<1> {
            value: [-100],
            exp: -52,
        };
        let b = CopyIntExp::<1> {
            value: [-1],
            exp: -52,
        };
        let s = a + b;
        assert_eq!(s.exp, -52);
        assert_eq!(s.value[0], -101);
    }

    #[test]
    // r[verify cz.math.copy-intexp-add-squeeze+1]
    fn add_negative_plus_small_positive_keeps_word() {
        let a = CopyIntExp::<1> {
            value: [i64::MIN / 2],
            exp: -60,
        };
        let b = CopyIntExp::<1> {
            value: [1],
            exp: -52,
        };
        let s0 = a;
        let s1 = a + b;
        assert_ne!(s0, s1);
        assert!(s1.exp < 0);
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
    // r[verify cz.math.copy-intexp-no-infinity+2]
    fn never_infinite() {
        assert!(C2::ZERO.is_finite());
        assert!(C2::max_value().is_finite());
        assert!(C2::from_f32(-0.0).is_finite());
        assert!(C2::from_f32(1.25).is_finite());
    }

    #[test]
    // r[verify cz.math.copy-intexp-no-infinity+2]
    fn sub_unit_is_not_zero() {
        let c = CopyIntExp::<1> {
            value: [3 << 50],
            exp: -52,
        };
        assert!(c > CopyIntExp1::ZERO);
        assert_ne!(c, CopyIntExp1::ZERO);
        let tiny = CopyIntExp::<1> {
            value: [1],
            exp: -61,
        };
        assert!(tiny > CopyIntExp1::ZERO);
        assert!(c > tiny);
        let vanished = CopyIntExp1::ZERO - tiny;
        assert_eq!(vanished, CopyIntExp1::ZERO);
        assert!(!(c <= vanished));
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

        // r[verify cz.math.copy-intexp-no-infinity+2]
        #[test]
        fn every_value_is_finite(a in arb_c2()) {
            prop_assert!(a.is_finite());
            prop_assert!(<C2 as Mandelbrotable>::is_finite(a));
        }

        // r[verify cz.math.copy-intexp-no-infinity+2]
        #[test]
        fn ord_is_total(a in arb_c2(), b in arb_c2()) {
            prop_assert_eq!(a.cmp(&b), b.cmp(&a).reverse());
            if a == b {
                prop_assert_eq!(a.cmp(&b), Ordering::Equal);
            }
        }
    }
}
