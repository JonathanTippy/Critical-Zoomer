use std::cmp::*;
use std::ops::*;
use rug::*;
use crate::constants::*;
use crate::intexp::*;

pub type DefaultStackedIntExp = StackedIntExp<STACKED_INTEXP_STACKS>;

#[derive(Clone, Copy, Debug, Eq)]
pub struct StackedIntExp<const STACKS: usize> {
    pub limbs: [i64; STACKS]
    , pub exp: i32
}

impl<const STACKS: usize> StackedIntExp<STACKS> {
    pub const ZERO: Self = Self {
        limbs: [0; STACKS]
        , exp: 0
    };

    pub fn from_i64(value: i64) -> Self {
        let mut limbs = [0i64; STACKS];
        if STACKS > 0 {
            limbs[0] = value;
            let fill = if value < 0 { -1i64 } else { 0 };
            for i in 1..STACKS {
                limbs[i] = fill;
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

    fn is_zero_limbs(limbs: &[i64; STACKS]) -> bool {
        limbs.iter().all(|&limb| limb == 0)
    }

    fn sign_extend_carry(high: i64) -> i128 {
        if high < 0 { -1 } else { 0 }
    }

    fn add_limbs(a: [i64; STACKS], b: [i64; STACKS]) -> [i64; STACKS] {
        let mut out = [0i64; STACKS];
        let mut carry: u128 = 0;
        for i in 0..STACKS {
            let sum = a[i] as u64 as u128 + b[i] as u64 as u128 + carry;
            out[i] = sum as u64 as i64;
            carry = sum >> 64;
        }
        let _ = carry;
        out
    }

    fn neg_limbs(a: [i64; STACKS]) -> [i64; STACKS] {
        let mut out = [0i64; STACKS];
        let mut carry: u128 = 1;
        for i in 0..STACKS {
            let sum = (!a[i]) as u64 as u128 + carry;
            out[i] = sum as u64 as i64;
            carry = sum >> 64;
        }
        out
    }

    fn shr_limbs_bits(mut limbs: [i64; STACKS], bits: u32) -> [i64; STACKS] {
        if bits == 0 || Self::is_zero_limbs(&limbs) {
            return limbs;
        }
        let mut remaining = bits;
        while remaining >= 64 {
            let sign = if limbs[STACKS - 1] < 0 { -1i64 } else { 0 };
            for i in 0..STACKS - 1 {
                limbs[i] = limbs[i + 1];
            }
            limbs[STACKS - 1] = sign;
            remaining -= 64;
        }
        if remaining == 0 {
            return limbs;
        }
        let r = remaining;
        let mask_shift = 64 - r;
        let mut prev_ext = if limbs[STACKS - 1] < 0 { u64::MAX } else { 0u64 };
        for i in (0..STACKS).rev() {
            let cur = limbs[i] as u64;
            let next_ext = cur;
            limbs[i] = ((cur >> r) | (prev_ext << mask_shift)) as i64;
            prev_ext = next_ext;
        }
        limbs
    }

    fn shl_limbs_bits(mut limbs: [i64; STACKS], bits: u32) -> ([i64; STACKS], i128) {
        if bits == 0 {
            return (limbs, Self::sign_extend_carry(limbs[STACKS - 1]));
        }
        let mut remaining = bits;
        let mut spilled: i128 = Self::sign_extend_carry(limbs[STACKS - 1]);
        while remaining >= 64 {
            spilled = limbs[STACKS - 1] as i128;
            for i in (1..STACKS).rev() {
                limbs[i] = limbs[i - 1];
            }
            limbs[0] = 0;
            remaining -= 64;
        }
        if remaining == 0 {
            return (limbs, spilled);
        }
        let r = remaining;
        let mut carry_bits = 0u64;
        for i in 0..STACKS {
            let cur = limbs[i] as u64;
            let next_carry = cur >> (64 - r);
            limbs[i] = ((cur << r) | carry_bits) as i64;
            carry_bits = next_carry;
        }
        spilled = (spilled << r) | carry_bits as i128;
        (limbs, spilled)
    }

    fn absorb_high(mut limbs: [i64; STACKS], mut high: i128, mut exp: i32) -> Self {
        loop {
            let expected = Self::sign_extend_carry(limbs[STACKS - 1]);
            if high == expected {
                if Self::is_zero_limbs(&limbs) {
                    return Self::ZERO;
                }
                return Self { limbs, exp };
            }
            let mut next = [0i64; STACKS];
            for i in 0..STACKS - 1 {
                next[i] = limbs[i + 1];
            }
            next[STACKS - 1] = high as i64;
            high >>= 64;
            limbs = next;
            exp = exp.saturating_add(64);
        }
    }

    fn align_pair(self, other: Self) -> (Self, Self) {
        match self.exp.cmp(&other.exp) {
            Ordering::Equal => (self, other)
            , Ordering::Less => {
                let delta = (other.exp - self.exp) as u32;
                let (limbs, high) = Self::shl_limbs_bits(other.limbs, delta);
                let aligned = Self::absorb_high(limbs, high, self.exp);
                (self, aligned)
            }
            , Ordering::Greater => {
                let delta = (self.exp - other.exp) as u32;
                let (limbs, high) = Self::shl_limbs_bits(self.limbs, delta);
                let aligned = Self::absorb_high(limbs, high, other.exp);
                (aligned, other)
            }
        }
    }
}

impl<const STACKS: usize> From<i32> for StackedIntExp<STACKS> {
    fn from(value: i32) -> Self {
        Self::from_i64(value as i64)
    }
}

impl<const STACKS: usize> From<IntExp> for StackedIntExp<STACKS> {
    fn from(value: IntExp) -> Self {
        let mut limbs = [0i64; STACKS];
        let mut remaining = value.val;
        for i in 0..STACKS {
            let limb = Integer::from(&remaining & Integer::from(u64::MAX));
            limbs[i] = limb.to_i64_wrapping();
            remaining >>= 64;
        }
        let high = if remaining == 0 {
            0i128
        } else if remaining == -1 {
            -1i128
        } else {
            remaining.to_i64().map(|v| v as i128).unwrap_or_else(|| {
                if remaining < 0 { i128::MIN } else { i128::MAX }
            })
        };
        Self::absorb_high(limbs, high, value.exp)
    }
}

impl<const STACKS: usize> From<StackedIntExp<STACKS>> for IntExp {
    fn from(value: StackedIntExp<STACKS>) -> Self {
        if StackedIntExp::<STACKS>::is_zero_limbs(&value.limbs) {
            return IntExp::ZERO;
        }
        let mut acc = Integer::from(0);
        for i in (0..STACKS).rev() {
            acc <<= 64;
            acc += Integer::from(value.limbs[i] as u64);
        }
        if value.limbs[STACKS - 1] < 0 {
            let mut modulus = Integer::from(1);
            modulus <<= (STACKS * 64) as u32;
            acc -= modulus;
        }
        IntExp {
            val: acc
            , exp: value.exp
        }
    }
}

impl<const STACKS: usize> PartialEq for StackedIntExp<STACKS> {
    fn eq(&self, other: &Self) -> bool {
        (*self - *other).limbs.iter().all(|&limb| limb == 0)
    }
}

impl<const STACKS: usize> PartialOrd for StackedIntExp<STACKS> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let diff = *self - *other;
        if diff.limbs.iter().all(|&limb| limb == 0) {
            Some(Ordering::Equal)
        } else if diff.limbs[STACKS - 1] < 0 {
            Some(Ordering::Less)
        } else {
            Some(Ordering::Greater)
        }
    }
}

impl<const STACKS: usize> Add for StackedIntExp<STACKS> {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        let (a, b) = self.align_pair(other);
        let limbs = Self::add_limbs(a.limbs, b.limbs);
        if Self::is_zero_limbs(&limbs) {
            Self::ZERO
        } else {
            Self { limbs, exp: a.exp }
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
        let product = IntExp::from(self) * IntExp::from(other);
        Self::from(product)
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
}
