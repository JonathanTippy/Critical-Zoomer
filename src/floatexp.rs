use std::cmp::*;
use std::ops::*;
use rug::Float;
use crate::assemblies::workgroup::structs::mandelbrotable::*;
use crate::intexp::*;

#[derive(Clone, Copy, Debug)]
pub struct FloatExp {
    pub mantissa: f64
    , pub exp: i32
}

pub type RugReferenceFloat = Float;

impl FloatExp {
    pub const ZERO: Self = Self { mantissa: 0.0, exp: 0 };
    pub const ONE: Self = Self { mantissa: 0.5, exp: 1 };
    pub const TWO: Self = Self { mantissa: 0.5, exp: 2 };

    pub fn new(mantissa: f64, exp: i32) -> Self {
        if mantissa == 0.0 {
            return Self::ZERO;
        }
        let (m, e) = frexp(mantissa);
        Self {
            mantissa: m
            , exp: exp.saturating_add(e)
        }
    }

    pub fn from_f64(value: f64) -> Self {
        Self::new(value, 0)
    }

    pub fn to_f64(self) -> f64 {
        if self.mantissa == 0.0 {
            return 0.0;
        }
        if self.exp > 1023 {
            return f64::INFINITY.copysign(self.mantissa);
        }
        if self.exp < -1022 {
            return 0.0;
        }
        self.mantissa * 2.0f64.powi(self.exp)
    }

    pub fn to_f32(self) -> f32 {
        self.to_f64() as f32
    }
}

fn frexp(value: f64) -> (f64, i32) {
    if value == 0.0 {
        return (0.0, 0);
    }
    let bits = value.to_bits();
    let raw_exp = ((bits >> 52) & 0x7ff) as i32;
    if raw_exp == 0 {
        let (m, e) = frexp(value * 2.0f64.powi(54));
        return (m, e - 54);
    }
    if raw_exp == 0x7ff {
        return (value, 0);
    }
    let mantissa_bits = (bits & ((1u64 << 52) - 1)) | (1u64 << 52);
    let sign_bit = bits & (1u64 << 63);
    let mantissa = f64::from_bits(sign_bit | ((1022u64) << 52) | (mantissa_bits & ((1u64 << 52) - 1)));
    (mantissa, raw_exp - 1022)
}

impl From<i32> for FloatExp {
    fn from(value: i32) -> Self {
        Self::from_f64(value as f64)
    }
}

impl From<IntExp> for FloatExp {
    fn from(value: IntExp) -> Self {
        Self::new(value.val.to_f64(), value.exp)
    }
}

impl PartialEq for FloatExp {
    fn eq(&self, other: &Self) -> bool {
        self.mantissa == other.mantissa && self.exp == other.exp
            || (self.mantissa == 0.0 && other.mantissa == 0.0)
    }
}

impl PartialOrd for FloatExp {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.mantissa == 0.0 && other.mantissa == 0.0 {
            return Some(Ordering::Equal);
        }
        if self.mantissa == 0.0 {
            return Some(if other.mantissa > 0.0 { Ordering::Less } else { Ordering::Greater });
        }
        if other.mantissa == 0.0 {
            return Some(if self.mantissa > 0.0 { Ordering::Greater } else { Ordering::Less });
        }
        let s = self.mantissa.signum();
        let o = other.mantissa.signum();
        if s != o {
            return s.partial_cmp(&o);
        }
        match self.exp.cmp(&other.exp) {
            Ordering::Equal => self.mantissa.partial_cmp(&other.mantissa)
            , ord => {
                if s > 0.0 {
                    Some(ord)
                } else {
                    Some(ord.reverse())
                }
            }
        }
    }
}

impl Add for FloatExp {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        if self.mantissa == 0.0 {
            return other;
        }
        if other.mantissa == 0.0 {
            return self;
        }
        match self.exp.cmp(&other.exp) {
            Ordering::Equal => Self::new(self.mantissa + other.mantissa, self.exp)
            , Ordering::Greater => {
                let delta = self.exp - other.exp;
                if delta > 60 {
                    self
                } else {
                    Self::new(self.mantissa + other.mantissa * 2.0f64.powi(-delta), self.exp)
                }
            }
            , Ordering::Less => {
                let delta = other.exp - self.exp;
                if delta > 60 {
                    other
                } else {
                    Self::new(other.mantissa + self.mantissa * 2.0f64.powi(-delta), other.exp)
                }
            }
        }
    }
}

impl Sub for FloatExp {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        self + Self {
            mantissa: -other.mantissa
            , exp: other.exp
        }
    }
}

impl Mul for FloatExp {
    type Output = Self;

    fn mul(self, other: Self) -> Self {
        if self.mantissa == 0.0 || other.mantissa == 0.0 {
            return Self::ZERO;
        }
        Self::new(
            self.mantissa * other.mantissa
            , self.exp.saturating_add(other.exp)
        )
    }
}

impl Mandelbrotable for FloatExp {
    const ZERO: Self = FloatExp::ZERO;
    const ONE: Self = FloatExp::ONE;
    const TWO: Self = FloatExp::TWO;

    fn from_u16(value: u16) -> Self {
        Self::from_f64(value as f64)
    }

    fn from_f64(value: f64) -> Self {
        FloatExp::from_f64(value)
    }

    fn to_f32(self) -> f32 {
        FloatExp::to_f32(self)
    }

    fn to_f64(self) -> f64 {
        FloatExp::to_f64(self)
    }

    fn abs(self) -> Self {
        Self {
            mantissa: self.mantissa.abs()
            , exp: self.exp
        }
    }

    fn neg(self) -> Self {
        Self {
            mantissa: -self.mantissa
            , exp: self.exp
        }
    }

    fn max_value() -> Self {
        // Large finite sentinel for min-magnitude init (not IEEE inf).
        Self::new(0.5, 1023)
    }
}

#[cfg(test)]
mod floatexp_tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn floatexp_add_mul_basic() {
        let a = FloatExp::from_f64(2.0);
        let b = FloatExp::from_f64(3.0);
        assert!(( (a + b).to_f64() - 5.0 ).abs() < 1e-12);
        assert!(( (a * b).to_f64() - 6.0 ).abs() < 1e-12);
    }

    #[test]
    fn floatexp_mandelbrotable_constants() {
        assert_eq!(<FloatExp as Mandelbrotable>::ZERO.to_f64(), 0.0);
        assert!((<FloatExp as Mandelbrotable>::ONE.to_f64() - 1.0).abs() < 1e-12);
        assert!((<FloatExp as Mandelbrotable>::TWO.to_f64() - 2.0).abs() < 1e-12);
    }

    #[test]
    fn floatexp_mul_one_is_identity() {
        let a = FloatExp::from_f64(12.5);
        let one = FloatExp::ONE;
        assert!(((a * one).to_f64() - a.to_f64()).abs() < 1e-12);
    }

    // Property: Finite round-trip preserves value within relative tolerance.
    proptest! {
        #[test]
        fn floatexp_from_f64_roundtrip(
            v in prop_oneof![
                Just(0.0f64),
                -1e6f64..1e6f64,
            ]
        ) {
            let back = FloatExp::from_f64(v).to_f64();
            if v == 0.0 {
                prop_assert_eq!(back, 0.0);
            } else {
                let scale = v.abs().max(1.0);
                prop_assert!((back - v).abs() / scale < 1e-12);
            }
        }
    }
}
