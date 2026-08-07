use std::cmp::Ordering;
use std::ops::{Add, Mul, Neg, Sub};

use rug::Float;

use crate::assemblies::workgroup::c_generator::Mandelbrotable;
use crate::utils::IntExp;

/// A floating mantissa with an exponent range large enough for deep deltas.
///
/// Nonzero values are normalized to `1 <= |mantissa| < 2`. There are no NaN
/// or infinity values: exponent overflow is checked, and zero is canonical.
// r[impl cz.depth.floatexp-range+1]
#[derive(Clone, Copy, Debug)]
pub struct FloatExp {
    pub mantissa: f64,
    pub exponent: i64,
}

impl FloatExp {
    pub const ZERO: Self = Self {
        mantissa: 0.0,
        exponent: 0,
    };
    pub const ONE: Self = Self {
        mantissa: 1.0,
        exponent: 0,
    };
    pub const TWO: Self = Self {
        mantissa: 1.0,
        exponent: 1,
    };

    pub fn new(mantissa: f64, exponent: i64) -> Self {
        assert!(
            mantissa.is_finite(),
            "FloatExp cannot represent non-finite values"
        );
        if mantissa == 0.0 {
            return Self::ZERO;
        }
        let mut m = mantissa;
        let mut e = exponent;
        while m.abs() >= 2.0 {
            m *= 0.5;
            e = e.checked_add(1).expect("FloatExp exponent overflow");
        }
        while m.abs() < 1.0 {
            m *= 2.0;
            e = e.checked_sub(1).expect("FloatExp exponent underflow");
        }
        Self {
            mantissa: m,
            exponent: e,
        }
    }

    pub fn from_rug(value: &Float) -> Self {
        if value == &0 {
            return Self::ZERO;
        }
        let (mantissa, exponent) = value.to_f64_exp();
        // rug returns mantissa in [0.5, 1); normalize to this type's [1, 2).
        Self::new(mantissa * 2.0, exponent as i64 - 1)
    }

    pub fn to_rug(self, precision: u32) -> Float {
        let mut value = Float::with_val(precision, self.mantissa);
        if self.exponent >= 0 {
            value <<= self.exponent.min(i32::MAX as i64) as i32;
        } else {
            value >>= (-self.exponent).min(i32::MAX as i64) as i32;
        }
        value
    }

    pub fn abs(self) -> Self {
        Self {
            mantissa: self.mantissa.abs(),
            exponent: self.exponent,
        }
    }

    pub fn square(self) -> Self {
        self * self
    }

    pub fn to_f64(self) -> f64 {
        if self.exponent > i32::MAX as i64 {
            return if self.mantissa.is_sign_negative() {
                f64::NEG_INFINITY
            } else {
                f64::INFINITY
            };
        }
        if self.exponent < i32::MIN as i64 {
            return 0.0_f64.copysign(self.mantissa);
        }
        self.mantissa * 2.0f64.powi(self.exponent as i32)
    }
}

impl From<f64> for FloatExp {
    fn from(value: f64) -> Self {
        if value == 0.0 {
            return Self::ZERO;
        }
        assert!(
            value.is_finite(),
            "FloatExp cannot represent non-finite values"
        );
        let exponent = value.abs().log2().floor() as i64;
        Self::new(value / 2.0f64.powi(exponent as i32), exponent)
    }
}

impl From<IntExp> for FloatExp {
    fn from(value: IntExp) -> Self {
        if value.val == 0 {
            return Self::ZERO;
        }
        let (mantissa, integer_exponent) = value.val.to_f64_exp();
        Self::new(
            mantissa * 2.0,
            value.exp as i64 + integer_exponent as i64 - 1,
        )
    }
}

impl PartialEq for FloatExp {
    fn eq(&self, other: &Self) -> bool {
        self.mantissa == other.mantissa && (self.mantissa == 0.0 || self.exponent == other.exponent)
    }
}

impl PartialOrd for FloatExp {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self == other {
            return Some(Ordering::Equal);
        }
        if self.mantissa == 0.0 {
            return Some(if other.mantissa.is_sign_negative() {
                Ordering::Greater
            } else {
                Ordering::Less
            });
        }
        if other.mantissa == 0.0 {
            return Some(if self.mantissa.is_sign_negative() {
                Ordering::Less
            } else {
                Ordering::Greater
            });
        }
        let a_negative = self.mantissa.is_sign_negative();
        let b_negative = other.mantissa.is_sign_negative();
        if a_negative != b_negative {
            return Some(if a_negative {
                Ordering::Less
            } else {
                Ordering::Greater
            });
        }
        let magnitude = self.exponent.cmp(&other.exponent).then_with(|| {
            self.mantissa
                .abs()
                .partial_cmp(&other.mantissa.abs())
                .unwrap()
        });
        Some(if a_negative {
            magnitude.reverse()
        } else {
            magnitude
        })
    }
}

impl Add for FloatExp {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        if self.mantissa == 0.0 {
            return rhs;
        }
        if rhs.mantissa == 0.0 {
            return self;
        }
        if self.exponent >= rhs.exponent {
            let shift = self.exponent - rhs.exponent;
            if shift > 54 {
                return self;
            }
            Self::new(
                self.mantissa + rhs.mantissa * 2.0f64.powi(-(shift as i32)),
                self.exponent,
            )
        } else {
            rhs + self
        }
    }
}

impl Neg for FloatExp {
    type Output = Self;
    fn neg(self) -> Self {
        Self {
            mantissa: -self.mantissa,
            exponent: self.exponent,
        }
    }
}

impl Sub for FloatExp {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        self + (-rhs)
    }
}

impl Mul for FloatExp {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        if self.mantissa == 0.0 || rhs.mantissa == 0.0 {
            return Self::ZERO;
        }
        Self::new(
            self.mantissa * rhs.mantissa,
            self.exponent
                .checked_add(rhs.exponent)
                .expect("FloatExp exponent overflow"),
        )
    }
}

impl Mandelbrotable for FloatExp {
    const ZERO: Self = Self::ZERO;
    const ONE: Self = Self::ONE;
    const TWO: Self = Self::TWO;
    fn from_u32(value: u32) -> Self {
        Self::from(value as f64)
    }
    fn to_f64(self) -> f64 {
        self.to_f64()
    }
    fn abs(self) -> Self {
        self.abs()
    }
    fn neg(self) -> Self {
        -self
    }
    fn max_value() -> Self {
        Self {
            mantissa: 1.9999999999999998,
            exponent: i64::MAX,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ComplexFloatExp {
    pub re: FloatExp,
    pub im: FloatExp,
}

impl ComplexFloatExp {
    pub const ZERO: Self = Self {
        re: FloatExp::ZERO,
        im: FloatExp::ZERO,
    };
    pub fn new(re: FloatExp, im: FloatExp) -> Self {
        Self { re, im }
    }
    pub fn norm_squared(self) -> FloatExp {
        self.re.square() + self.im.square()
    }
}

impl Add for ComplexFloatExp {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self::new(self.re + rhs.re, self.im + rhs.im)
    }
}

impl Sub for ComplexFloatExp {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self::new(self.re - rhs.re, self.im - rhs.im)
    }
}

impl Mul for ComplexFloatExp {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        Self::new(
            self.re * rhs.re - self.im * rhs.im,
            self.re * rhs.im + self.im * rhs.re,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        // r[verify cz.depth.floatexp-range+1]
        #[test]
        fn add_and_multiply_agree_with_rug(
            a in -1.0e100f64..1.0e100,
            b in -1.0e100f64..1.0e100,
        ) {
            let fa = FloatExp::from(a);
            let fb = FloatExp::from(b);
            let rug_a = Float::with_val(256, a);
            let rug_b = Float::with_val(256, b);
            let expected_add = FloatExp::from_rug(&Float::with_val(256, &rug_a + &rug_b));
            let expected_mul = FloatExp::from_rug(&Float::with_val(256, &rug_a * &rug_b));
            prop_assert_eq!(fa + fb, expected_add);
            prop_assert_eq!(fa * fb, expected_mul);
        }
    }

    #[test]
    // r[verify cz.depth.floatexp-range+1]
    fn does_not_underflow_far_beyond_f64() {
        let tiny = FloatExp::new(1.25, -5000);
        assert_ne!(tiny, FloatExp::ZERO);
        assert_eq!(tiny * FloatExp::TWO, FloatExp::new(1.25, -4999));
        assert_eq!(tiny.to_f64(), 0.0);
    }

    #[test]
    fn zero_is_canonical_and_exact() {
        assert_eq!(FloatExp::new(0.0, 1234), FloatExp::ZERO);
        assert_eq!(FloatExp::ONE - FloatExp::ONE, FloatExp::ZERO);
        assert_eq!(FloatExp::ZERO * FloatExp::new(1.0, -5000), FloatExp::ZERO);
        assert!(FloatExp::new(1.0, -5000) > FloatExp::ZERO);
        assert!(FloatExp::new(-1.0, -5000) < FloatExp::ZERO);
    }
}
