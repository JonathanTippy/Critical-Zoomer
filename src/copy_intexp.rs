const WORDSIZE: usize = 64;

// This is a form of intexp optimal for iterating with either naive or perturbed methods.
// The const bits number is central; sized data on the stack is fast data, even if its actually quite a bit of data.

use std::cmp::Ordering;
use std::cmp::Ordering::{Equal, Greater, Less};
use crate::utils::IntExp;
use rug::integer::Order;
use std::ops::{Add, Mul, Sub};

#[derive(Clone, Copy)]
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
            CopyIntExp {
                value: words,
                exp: value.exp,
            }
            // good job assistant, happy with this block.
        }
    }

    /// Like `IntExp`: no infinities, so every value is finite.
    pub(crate) fn is_finite(self) -> bool {
        true
    }
}

impl<const Words: usize> Add for CopyIntExp<Words> {
    type Output = CopyIntExp<Words>;
    fn add(self, other: CopyIntExp<Words>) -> Self::Output {
        // Only the higher-exp mantissa is shifted — never `<< 0` (which is
        // observationally identical to `>> 0` and hid a cargo-mutants survivor).
        match self.exp.cmp(&other.exp) {
            Equal => Self {
                value: add_limbs(self.value, other.value),
                exp: self.exp,
            },
            Greater => {
                let s = (self.exp - other.exp) as u32;
                debug_assert!(s > 0);
                Self {
                    value: add_limbs(shl_limbs(self.value, s), other.value),
                    exp: other.exp,
                }
            }
            Less => {
                let s = (other.exp - self.exp) as u32;
                debug_assert!(s > 0);
                Self {
                    value: add_limbs(self.value, shl_limbs(other.value, s)),
                    exp: self.exp,
                }
            }
        }
    }
}

fn add_limbs<const Words: usize>(a: [i64; Words], b: [i64; Words]) -> [i64; Words] {
    let mut out = [0i64; Words];
    let mut carry = 0i128;
    for i in 0..Words {
        let sum = a[i] as i128 + b[i] as i128 + carry;
        out[i] = sum as i64;
        carry = sum >> 64;
    }
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
        Self {
            value: mul_limbs(self.value, other.value),
            exp: self.exp + other.exp,
        }
    }
}

fn mul_limbs<const Words: usize>(a: [i64; Words], b: [i64; Words]) -> [i64; Words] {
    let mut out = [0i64; Words];
    for i in 0..Words {
        let mut carry = 0i128;
        for j in 0..(Words - i) {
            let t = (a[i] as i128) * (b[j] as i128) + (out[i + j] as i128) + carry;
            out[i + j] = t as i64;
            carry = t >> 64;
        }
    }
    out
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
