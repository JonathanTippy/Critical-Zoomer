//! Stacked i32 significand with a shared exponent (docs/design/tile_worker.md).
//!
//! Limbs are native `i32` (not i64), and multiply is schoolbook — no rug in the
//! hot path. Limb counts 1..=8 are the GPU-capable stacked gears.
// r[impl cz.seamless.gpu-preferred+1]

use std::cmp::*;
use std::ops::*;
use rug::Integer;
use crate::assemblies::workgroup::structs::mandelbrotable::Mandelbrotable;
use crate::constants::*;
use crate::intexp::*;

pub type DefaultStackedIntExp = StackedIntExp<STACKED_INTEXP_STACKS>;

#[derive(Clone, Copy, Debug, Eq)]
pub struct StackedIntExp<const STACKS: usize> {
    pub limbs: [i32; STACKS]
    , pub exp: i32
}

impl<const STACKS: usize> StackedIntExp<STACKS> {
    pub const ZERO: Self = Self {
        limbs: [0; STACKS]
        , exp: 0
    };

    pub const fn from_i32(value: i32) -> Self {
        let mut limbs = [0i32; STACKS];
        if STACKS > 0 {
            limbs[0] = value;
            let fill = if value < 0 { -1i32 } else { 0 };
            let mut i = 1;
            while i < STACKS {
                limbs[i] = fill;
                i += 1;
            }
        }
        Self { limbs, exp: 0 }
    }

    pub fn shift(self, bits: i32) -> Self {
        if bits >= 0 {
            self << bits as u32
        } else {
            self >> (-bits) as u32
        }
    }

    pub fn to_intexp(self) -> IntExp {
        IntExp::from(self)
    }

    fn is_zero_limbs(limbs: &[i32; STACKS]) -> bool {
        limbs.iter().all(|&limb| limb == 0)
    }

    fn is_negative(limbs: &[i32; STACKS]) -> bool {
        STACKS > 0 && limbs[STACKS - 1] < 0
    }

    fn add_limbs(a: [i32; STACKS], b: [i32; STACKS]) -> [i32; STACKS] {
        let mut out = [0i32; STACKS];
        let mut carry: u64 = 0;
        for i in 0..STACKS {
            let sum = (a[i] as u32 as u64) + (b[i] as u32 as u64) + carry;
            out[i] = sum as u32 as i32;
            carry = sum >> 32;
        }
        let _ = carry;
        out
    }

    fn neg_limbs(a: [i32; STACKS]) -> [i32; STACKS] {
        let mut out = [0i32; STACKS];
        let mut carry: u64 = 1;
        for i in 0..STACKS {
            let sum = (!(a[i] as u32) as u64) + carry;
            out[i] = sum as u32 as i32;
            carry = sum >> 32;
        }
        out
    }

    fn abs_limbs(a: [i32; STACKS]) -> ([i32; STACKS], bool) {
        if Self::is_negative(&a) {
            (Self::neg_limbs(a), true)
        } else {
            (a, false)
        }
    }

    /// Schoolbook multiply of two magnitudes; keep the low STACKS limbs
    /// (same truncation as packing an IntExp product back into fixed stacks).
    fn mul_limbs_schoolbook(a: [i32; STACKS], b: [i32; STACKS]) -> [i32; STACKS] {
        let mut wide = [0i64; 16];
        debug_assert!(STACKS <= 8);
        for i in 0..STACKS {
            let ai = a[i] as u32 as i64;
            for j in 0..STACKS {
                wide[i + j] += ai * (b[j] as u32 as i64);
            }
        }
        let mut carry: i64 = 0;
        for slot in wide.iter_mut().take(STACKS * 2) {
            let v = *slot + carry;
            *slot = v & 0xffff_ffff;
            carry = v >> 32;
        }
        let mut out = [0i32; STACKS];
        for i in 0..STACKS {
            out[i] = wide[i] as u32 as i32;
        }
        out
    }

    fn align_exponents(mut a: Self, mut b: Self) -> (Self, Self) {
        if a.exp == b.exp {
            return (a, b);
        }
        // Match IntExp: align to the smaller exponent by left-shifting significands.
        if a.exp > b.exp {
            let shift = (a.exp - b.exp) as u32;
            a.limbs = Self::shl_limbs_bits(a.limbs, shift);
            a.exp = b.exp;
        } else {
            let shift = (b.exp - a.exp) as u32;
            b.limbs = Self::shl_limbs_bits(b.limbs, shift);
            b.exp = a.exp;
        }
        (a, b)
    }

    fn shl_limbs_bits(mut limbs: [i32; STACKS], bits: u32) -> [i32; STACKS] {
        if bits == 0 || Self::is_zero_limbs(&limbs) {
            return limbs;
        }
        let mut remaining = bits;
        while remaining > 0 {
            let step = remaining.min(31);
            let mut carry: u32 = 0;
            for i in 0..STACKS {
                let cur = limbs[i] as u32;
                let shifted = (cur << step) | carry;
                carry = cur >> (32 - step);
                limbs[i] = shifted as i32;
            }
            remaining -= step;
        }
        limbs
    }

    fn shr_limbs_bits(mut limbs: [i32; STACKS], bits: u32) -> [i32; STACKS] {
        if bits == 0 || Self::is_zero_limbs(&limbs) {
            return limbs;
        }
        let negative = Self::is_negative(&limbs);
        let mut remaining = bits;
        while remaining > 0 {
            let step = remaining.min(31);
            let mut carry: u32 = if negative { u32::MAX } else { 0 };
            for i in (0..STACKS).rev() {
                let cur = limbs[i] as u32;
                let shifted = (cur >> step) | (carry << (32 - step));
                carry = cur;
                limbs[i] = shifted as i32;
            }
            remaining -= step;
        }
        limbs
    }
}

impl<const STACKS: usize> From<i32> for StackedIntExp<STACKS> {
    fn from(value: i32) -> Self {
        Self::from_i32(value)
    }
}

impl<const STACKS: usize> From<IntExp> for StackedIntExp<STACKS> {
    fn from(value: IntExp) -> Self {
        // Pack the Integer into i32 limbs, LSB first.
        let mut limbs = [0i32; STACKS];
        let mut n = value.val.clone();
        let negative = n < 0;
        if negative {
            n = -n;
        }
        for i in 0..STACKS {
            let limb = n.to_u32_wrapping();
            limbs[i] = limb as i32;
            n >>= 32;
            if n == 0 {
                break;
            }
        }
        if negative {
            limbs = Self::neg_limbs(limbs);
        }
        Self {
            limbs
            , exp: value.exp
        }
    }
}

impl<const STACKS: usize> From<StackedIntExp<STACKS>> for IntExp {
    fn from(value: StackedIntExp<STACKS>) -> Self {
        if StackedIntExp::<STACKS>::is_zero_limbs(&value.limbs) {
            return IntExp::ZERO;
        }
        let (mag, negative) = StackedIntExp::<STACKS>::abs_limbs(value.limbs);
        let mut n = Integer::from(0);
        for i in (0..STACKS).rev() {
            n <<= 32;
            n += mag[i] as u32;
        }
        if negative {
            n = -n;
        }
        IntExp {
            val: n
            , exp: value.exp
        }
    }
}

impl<const STACKS: usize> PartialEq for StackedIntExp<STACKS> {
    fn eq(&self, other: &Self) -> bool {
        IntExp::from(*self) == IntExp::from(*other)
    }
}

impl<const STACKS: usize> PartialOrd for StackedIntExp<STACKS> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        IntExp::from(*self).partial_cmp(&IntExp::from(*other))
    }
}

impl<const STACKS: usize> Add for StackedIntExp<STACKS> {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        let (a, b) = Self::align_exponents(self, other);
        Self {
            limbs: Self::add_limbs(a.limbs, b.limbs)
            , exp: a.exp
        }
    }
}

impl<const STACKS: usize> Sub for StackedIntExp<STACKS> {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        self + Self {
            limbs: Self::neg_limbs(other.limbs)
            , exp: other.exp
        }
    }
}

impl<const STACKS: usize> Mul for StackedIntExp<STACKS> {
    type Output = Self;

    fn mul(self, other: Self) -> Self {
        if Self::is_zero_limbs(&self.limbs) || Self::is_zero_limbs(&other.limbs) {
            return Self::ZERO;
        }
        let (a_mag, a_neg) = Self::abs_limbs(self.limbs);
        let (b_mag, b_neg) = Self::abs_limbs(other.limbs);
        let mut limbs = Self::mul_limbs_schoolbook(a_mag, b_mag);
        if a_neg ^ b_neg {
            limbs = Self::neg_limbs(limbs);
        }
        Self {
            limbs
            , exp: self.exp.saturating_add(other.exp)
        }
    }
}

impl<const STACKS: usize> Shl<u32> for StackedIntExp<STACKS> {
    type Output = Self;

    fn shl(self, rhs: u32) -> Self {
        Self {
            limbs: self.limbs
            , exp: self.exp.saturating_add(rhs as i32)
        }
    }
}

impl<const STACKS: usize> Shr<u32> for StackedIntExp<STACKS> {
    type Output = Self;

    fn shr(self, rhs: u32) -> Self {
        Self {
            limbs: self.limbs
            , exp: self.exp.saturating_sub(rhs as i32)
        }
    }
}

impl<const STACKS: usize> Mandelbrotable for StackedIntExp<STACKS> {
    const ZERO: Self = Self::ZERO;
    const ONE: Self = Self::from_i32(1);
    const TWO: Self = Self::from_i32(2);

    fn from_u16(value: u16) -> Self {
        Self::from_i32(value as i32)
    }

    fn to_f32(self) -> f32 {
        IntExp::from(self).to_f64() as f32
    }

    fn to_f64(self) -> f64 {
        IntExp::from(self).to_f64()
    }
}

#[cfg(test)]
mod stacked_intexp_tests {
    use super::*;

    #[test]
    fn stacked_add_matches_intexp() {
        let a = StackedIntExp::<4>::from(3);
        let b = StackedIntExp::<4>::from(5);
        let sum = a + b;
        assert_eq!(IntExp::from(sum), IntExp::from(3) + IntExp::from(5));
    }

    #[test]
    fn stacked_sub_matches_intexp() {
        let a = StackedIntExp::<4>::from(11);
        let b = StackedIntExp::<4>::from(4);
        assert_eq!(IntExp::from(a - b), IntExp::from(11) - IntExp::from(4));
    }

    #[test]
    fn stacked_mul_matches_intexp() {
        let a = StackedIntExp::<4>::from(-7);
        let b = StackedIntExp::<4>::from(9);
        assert_eq!(IntExp::from(a * b), IntExp::from(-7) * IntExp::from(9));
    }

    #[test]
    fn stacked_shifted_add_matches_intexp() {
        let a = StackedIntExp::<4>::from(1) << 10;
        let b = StackedIntExp::<4>::from(3);
        let got = IntExp::from(a + b);
        let expected = (IntExp::from(1) << 10) + IntExp::from(3);
        assert_eq!(got, expected);
    }

    #[test]
    fn stacked_roundtrip_intexp() {
        let original = (IntExp::from(12345) << 20) - IntExp::from(99);
        let stacked = StackedIntExp::<4>::from(original.clone());
        assert_eq!(IntExp::from(stacked), original);
    }

    #[test]
    fn default_stacks_is_four() {
        assert_eq!(STACKED_INTEXP_STACKS, 4);
        let _v: DefaultStackedIntExp = StackedIntExp::from(1);
    }

    #[test]
    fn one_through_eight_limb_gears_construct() {
        let _a: StackedIntExp<1> = StackedIntExp::from(1);
        let _b: StackedIntExp<8> = StackedIntExp::from(1);
    }
}
