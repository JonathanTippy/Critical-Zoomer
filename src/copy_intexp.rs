const WORDSIZE: usize = 64;

// This is a form of intexp optimal for iterating with either naive or perturbed methods.
// The const bits number is central; sized data on the stack is fast data, even if its actually quite a bit of data.

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
            // good job assistant, happy with this block.
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
        // Only the higher-exp mantissa is shifted — never `<< 0` (which is
        // observationally identical to `>> 0` and hid a cargo-mutants survivor).
        match self.exp.cmp(&other.exp) {
            Equal => pack_add(add_limbs(self.value, other.value), self.exp),
            Greater => {
                let s = (self.exp - other.exp) as u32;
                debug_assert!(s > 0);
                pack_add(add_limbs(shl_limbs(self.value, s), other.value), other.exp)
            }
            Less => {
                let s = (other.exp - self.exp) as u32;
                debug_assert!(s > 0);
                pack_add(add_limbs(self.value, shl_limbs(other.value, s)), self.exp)
            }
        }
    }
}

fn add_limbs<const Words: usize>(a: [i64; Words], b: [i64; Words]) -> ([i64; Words], i64) {
    let mut out = [0i64; Words];
    let mut carry = 0i128;
    for i in 0..Words {
        let sum = a[i] as i128 + b[i] as i128 + carry;
        out[i] = sum as i64;
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
    for i in 0..Words - 1 {
        out[i] = a[i + 1];
    }
    out[Words - 1] = extra;
    out
}

fn shl_limbs<const Words: usize>(a: [i64; Words], s: u32) -> [i64; Words] {
    let mut out = [0i64; Words];
    let word_shift = (s as usize) / WORDSIZE;
    let bits = s % (WORDSIZE as u32);
    if word_shift >= Words {
        return out;
    }
    for (i, dst) in out.iter_mut().enumerate().rev() {
        let Some(src) = i.checked_sub(word_shift) else {
            continue;
        };
        let low = a[src] as u64;
        let mut v = if bits == 0 { low } else { low << bits };
        if bits != 0 && src > 0 {
            v |= (a[src - 1] as u64) >> (WORDSIZE as u32 - bits);
        }
        *dst = v as i64;
    }
    out
}

impl<const Words: usize> Mul for CopyIntExp<Words> {
    type Output = Self;
    fn mul(self, other: Self) -> Self {
        let sa = limbs_neg(self.value);
        let sb = limbs_neg(other.value);
        let a = if sa {
            neg_limbs(self.value)
        } else {
            self.value
        };
        let b = if sb {
            neg_limbs(other.value)
        } else {
            other.value
        };
        let (mut lo, mut hi) = mul_limbs_full(a, b);
        let mut exp = self.exp + other.exp;
        while !limbs_zero(hi) {
            lo = shr_one_word(lo, hi[0]);
            for i in 0..Words - 1 {
                hi[i] = hi[i + 1];
            }
            hi[Words - 1] = 0;
            exp += WORDSIZE as i32;
        }
        let value = if sa != sb { neg_limbs(lo) } else { lo };
        Self { value, exp }
    }
}

fn mul_limbs_full<const Words: usize>(
    a: [i64; Words],
    b: [i64; Words],
) -> ([i64; Words], [i64; Words]) {
    let mut lo = [0i64; Words];
    let mut hi = [0i64; Words];
    for i in 0..Words {
        let mut carry = 0u128;
        for j in 0..Words {
            let idx = i + j;
            let old = if idx < Words {
                lo[idx] as u64 as u128
            } else {
                hi[idx - Words] as u64 as u128
            };
            let t = (a[i] as u64 as u128) * (b[j] as u64 as u128) + old + carry;
            let w = t as u64 as i64;
            if idx < Words {
                lo[idx] = w;
            } else {
                hi[idx - Words] = w;
            }
            carry = t >> 64;
        }
        let mut k = i;
        while carry != 0 && k < Words {
            let t = (hi[k] as u64 as u128) + carry;
            hi[k] = t as u64 as i64;
            carry = t >> 64;
            k += 1;
        }
    }
    (lo, hi)
}

/// Q: What about rounding? A: not necessary when iterating. bits stay the same.
impl<const Words: usize> PartialEq for CopyIntExp<Words> {
    fn eq(&self, other: &Self) -> bool {
        limbs_zero((self.sub(*other)).value)
    }
}

impl<const Words: usize> Eq for CopyIntExp<Words> {}

impl<const Words: usize> PartialOrd for CopyIntExp<Words> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
    fn lt(&self, other: &Self) -> bool {
        limbs_neg((self.sub(*other)).value)
    }
    fn gt(&self, other: &Self) -> bool {
        let d = (self.sub(*other)).value;
        !limbs_neg(d) && !limbs_zero(d)
    }
    fn le(&self, other: &Self) -> bool {
        !self.gt(other)
    }
    fn ge(&self, other: &Self) -> bool {
        !self.lt(other)
    }
}

impl<const Words: usize> Ord for CopyIntExp<Words> {
    fn cmp(&self, other: &Self) -> Ordering {
        let d = (self.sub(*other)).value;
        if limbs_neg(d) {
            Ordering::Less
        } else if limbs_zero(d) {
            Ordering::Equal
        } else {
            Ordering::Greater
        }
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
    for i in 0..Words {
        let t = (!(a[i] as u64) as i128) + carry;
        out[i] = t as i64;
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
}
