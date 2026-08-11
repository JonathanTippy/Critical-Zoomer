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

    #[inline(always)]
    pub fn new(mantissa: f64, exponent: i64) -> Self {
        assert!(
            mantissa.is_finite(),
            "FloatExp cannot represent non-finite values"
        );
        if mantissa == 0.0 {
            return Self::ZERO;
        }
        // Extract the binary exponent directly. This is exactly the loop
        // normalization below expressed in O(1): preserve sign/fraction, set
        // the f64 exponent to zero, and carry its old exponent into `exponent`.
        let bits = mantissa.to_bits();
        let biased = ((bits >> 52) & 0x7ff) as i64;
        let (m, adjustment) = if biased == 0 {
            // A nonzero subnormal becomes normal after an exact 2^64 scale.
            let scaled = mantissa * 18_446_744_073_709_551_616.0;
            let scaled_bits = scaled.to_bits();
            let scaled_biased = ((scaled_bits >> 52) & 0x7ff) as i64;
            (
                f64::from_bits(
                    (scaled_bits & !(0x7ffu64 << 52)) | (1023u64 << 52),
                ),
                scaled_biased - 1023 - 64,
            )
        } else {
            (
                f64::from_bits((bits & !(0x7ffu64 << 52)) | (1023u64 << 52)),
                biased - 1023,
            )
        };
        Self {
            mantissa: m,
            exponent: exponent
                .checked_add(adjustment)
                .expect("FloatExp exponent overflow"),
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

    #[inline(always)]
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
        let biased = self.exponent + 1023;
        if biased >= 0x7ff {
            return if self.mantissa.is_sign_negative() {
                f64::NEG_INFINITY
            } else {
                f64::INFINITY
            };
        }
        if biased <= 0 {
            if self.exponent < -1074 {
                return 0.0_f64.copysign(self.mantissa);
            }
            // Keep the scale normal until the final multiply so `powi` cannot
            // underflow before the mantissa participates.
            return (self.mantissa
                * 2.0f64.powi((self.exponent + 1022) as i32))
                * f64::MIN_POSITIVE;
        }
        self.mantissa * f64::from_bits((biased as u64) << 52)
    }
}

impl From<f64> for FloatExp {
    fn from(value: f64) -> Self {
        Self::new(value, 0)
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
    #[inline(always)]
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
            let scale = f64::from_bits(((1023 - shift) as u64) << 52);
            let sum = self.mantissa + rhs.mantissa * scale;
            // Hot path: avoid FloatExp::new's assert + full bit extract when the
            // sum is already a normal finite mantissa near [1, 2).
            debug_assert!(sum.is_finite());
            if sum == 0.0 {
                return Self::ZERO;
            }
            let abs = sum.abs();
            if abs >= 1.0 && abs < 2.0 {
                return Self {
                    mantissa: sum,
                    exponent: self.exponent,
                };
            }
            if abs >= 2.0 && abs < 4.0 {
                return Self {
                    mantissa: sum * 0.5,
                    exponent: self.exponent.saturating_add(1),
                };
            }
            if abs >= 0.5 && abs < 1.0 {
                return Self {
                    mantissa: sum * 2.0,
                    exponent: self.exponent.saturating_sub(1),
                };
            }
            Self::new(sum, self.exponent)
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
    #[inline(always)]
    fn mul(self, rhs: Self) -> Self {
        if self.mantissa == 0.0 || rhs.mantissa == 0.0 {
            return Self::ZERO;
        }
        let product = self.mantissa * rhs.mantissa;
        // Hot path: mantissa product stays in [1, 4) for normalized inputs, so
        // at most one exponent bump. Saturating add is enough for deep zooms;
        // overflow still surfaces as an extreme exponent rather than wrapping.
        let mut exponent = self.exponent.saturating_add(rhs.exponent);
        let mantissa = if product.abs() >= 2.0 {
            exponent = exponent.saturating_add(1);
            product * 0.5
        } else {
            product
        };
        Self { mantissa, exponent }
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

impl From<f32> for FloatExp {
    fn from(value: f32) -> Self {
        Self::from(value as f64)
    }
}

impl Into<f64> for FloatExp {
    fn into(self) -> f64 {
        self.to_f64()
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
    #[inline(always)]
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
    use crate::utils::IntExp;
    use proptest::prelude::*;
    use std::cmp::Ordering;

    fn slow_normalize(mantissa: f64, exponent: i64) -> FloatExp {
        assert!(mantissa.is_finite());
        if mantissa == 0.0 {
            return FloatExp::ZERO;
        }
        let mut m = mantissa;
        let mut e = exponent;
        while m.abs() >= 2.0 {
            m *= 0.5;
            e += 1;
        }
        while m.abs() < 1.0 {
            m *= 2.0;
            e -= 1;
        }
        FloatExp {
            mantissa: m,
            exponent: e,
        }
    }

    proptest! {
        #[test]
        fn constant_time_normalization_matches_loop(
            mantissa in any::<f64>().prop_filter("finite", |v| v.is_finite()),
            exponent in -10_000i64..10_000,
        ) {
            prop_assert_eq!(FloatExp::new(mantissa, exponent), slow_normalize(mantissa, exponent));
        }

        #[test]
        fn f64_round_trip_is_exact(value in any::<f64>().prop_filter("finite", |v| v.is_finite())) {
            if value == 0.0 {
                prop_assert_eq!(FloatExp::from(value), FloatExp::ZERO);
            } else {
                prop_assert_eq!(FloatExp::from(value).to_f64().to_bits(), value.to_bits());
            }
        }

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

    /// Fast pins for `to_f64` mutants that previously *timed out* under the full suite
    /// (`mutants.out/timeout.txt`). Keep these local and cheap.
    #[test]
    fn to_f64_rejects_constant_and_branch_mutants() {
        assert_eq!(FloatExp::from(2.0).to_f64(), 2.0);
        assert_eq!(FloatExp::from(-3.5).to_f64(), -3.5);
        assert_eq!(FloatExp::ONE.to_f64(), 1.0);
        assert_ne!(FloatExp::from(2.0).to_f64(), 0.0);
        assert_ne!(FloatExp::from(2.0).to_f64(), 1.0);
        assert_ne!(FloatExp::from(2.0).to_f64(), -1.0);

        // Overflow / underflow branch direction (kill exponent bound flips).
        assert!(FloatExp::new(1.0, (i32::MAX as i64) + 10).to_f64().is_infinite());
        assert!(FloatExp::new(-1.0, (i32::MAX as i64) + 10).to_f64().is_sign_negative());
        assert_eq!(FloatExp::new(1.5, (i32::MIN as i64) - 10).to_f64(), 0.0);
        assert!(FloatExp::new(-1.5, (i32::MIN as i64) - 10)
            .to_f64()
            .is_sign_negative());

        // Subnormal path uses mantissa * 2^(e+1022) * MIN_POSITIVE — not +.
        let sub = FloatExp::new(1.0, -1070);
        let got = sub.to_f64();
        assert!(got > 0.0 && got.is_finite(), "subnormal got {got}");
        assert_ne!(got, 1.0 + 2.0f64.powi((-1070 + 1022) as i32) * f64::MIN_POSITIVE);

        // Normal path: mantissa * from_bits(biased << 52).
        let normal = FloatExp::new(1.5, 0);
        assert_eq!(normal.to_f64(), 1.5);
        let bits_mutant = 1.5 * f64::from_bits((1023u64) >> 52);
        assert_ne!(normal.to_f64(), bits_mutant);
    }

    /// Thought-killed pins for dense `floatexp.rs` caught mutants (add/mul/cmp/complex).
    #[test]
    fn add_mul_eq_ord_kill_operator_mutants() {
        let a = FloatExp::from(1.5);
        let b = FloatExp::from(0.75);
        let sum = a + b;
        assert!((sum.to_f64() - 2.25).abs() < 1e-12, "sum={}", sum.to_f64());
        // Zero identity / short-circuit.
        assert_eq!(a + FloatExp::ZERO, a);
        assert_eq!(FloatExp::ZERO + b, b);
        // Large exponent gap: smaller addend dropped (shift > 54).
        let huge = FloatExp::new(1.0, 100);
        let tiny = FloatExp::new(1.0, 0);
        assert_eq!(huge + tiny, huge);
        // Exponent alignment uses >= then scale — not flipped comparisons alone.
        let c = FloatExp::new(1.0, 5);
        let d = FloatExp::new(1.0, 3);
        let aligned = c + d;
        assert!((aligned.to_f64() - (32.0 + 8.0)).abs() < 1e-9, "{}", aligned.to_f64());
        // Both orderings (Greater-branch and recursive Less-branch).
        let aligned_rev = d + c;
        assert!((aligned_rev.to_f64() - aligned.to_f64()).abs() < 1e-9);

        let prod = a * b;
        assert!((prod.to_f64() - 1.125).abs() < 1e-12, "{}", prod.to_f64());
        assert_eq!(a * FloatExp::ZERO, FloatExp::ZERO);
        assert_eq!(FloatExp::ZERO * b, FloatExp::ZERO);
        // 1.5 * 1.5 = 2.25 → renormalize to mantissa 1.125 exp+1
        let sq = FloatExp::from(1.5).square();
        assert!((sq.mantissa - 1.125).abs() < 1e-12);
        assert_eq!(sq.exponent, 1);
        // *→+ / *→/ on mul.
        assert_ne!((a * b).to_f64(), 1.5 + 0.75);
        assert_ne!((a * b).to_f64(), 1.5 / 0.75);

        let neg = -a;
        assert_eq!(neg.mantissa, -1.5);
        assert_eq!((a - b).to_f64(), (a + (-b)).to_f64());

        // Eq: zero ignores exponent; nonzero requires both fields.
        assert_eq!(FloatExp::new(0.0, 99), FloatExp::ZERO);
        assert_ne!(FloatExp::new(1.0, 0), FloatExp::new(1.0, 1));
        assert_eq!(FloatExp::new(1.0, 0), FloatExp::ONE);

        assert!(FloatExp::from(-2.0) < FloatExp::ZERO);
        assert!(FloatExp::ZERO < FloatExp::from(2.0));
        assert!(FloatExp::from(-3.0) < FloatExp::from(-1.0));
        assert!(FloatExp::new(1.0, 10) > FloatExp::new(1.5, 9));
        assert_eq!(
            FloatExp::from(1.0).partial_cmp(&FloatExp::from(1.0)),
            Some(Ordering::Equal)
        );

        // Hot-path renormalize after add: 1.5+1.5 at same exp → mantissa 1.5, exp+1
        // (* 0.5, not + 0.5 / *→/).
        let twin = FloatExp::from(1.5) + FloatExp::from(1.5);
        assert!((twin.mantissa - 1.5).abs() < 1e-12, "m={}", twin.mantissa);
        assert_eq!(twin.exponent, 1);
        assert_ne!(twin.mantissa, 3.0 + 0.5);
        assert_ne!(twin.mantissa, 3.0 / 0.5);
    }

    /// Filter-friendly name for scoped `cargo mutants -- --lib mutant_kill`.
    #[test]
    fn mutant_kill_floatexp_add_mul_to_f64_complex() {
        to_f64_rejects_constant_and_branch_mutants();
        add_mul_eq_ord_kill_operator_mutants();
        complex_ops_and_intexp_conversion();
        // Extra *→/ pin on to_f64 normal path.
        let x = FloatExp::new(1.25, 3);
        assert!((x.to_f64() - 10.0).abs() < 1e-12, "got {}", x.to_f64());
        assert_ne!(x.to_f64(), 1.25 / 8.0);
        assert_ne!(x.to_f64(), 1.25 + 8.0);
        // Timeout-class: mantissa * from_bits(biased<<52) vs + / >>.
        let y = FloatExp::new(1.5, 4); // 24.0
        assert_eq!(y.to_f64(), 24.0);
        let biased = (4i64 + 1023) as u64;
        assert_ne!(y.to_f64(), 1.5 + f64::from_bits(biased << 52));
        assert_ne!(y.to_f64(), 1.5 * f64::from_bits(biased >> 52));
        let tiny_neg = FloatExp::new(-1.0, (i32::MIN as i64) - 100);
        assert_eq!(tiny_neg.to_f64(), -0.0);
        assert!(tiny_neg.to_f64().is_sign_negative());
    }

    #[test]
    fn complex_ops_and_intexp_conversion() {
        let z = ComplexFloatExp::new(FloatExp::from(1.0), FloatExp::from(2.0));
        let w = ComplexFloatExp::new(FloatExp::from(3.0), FloatExp::from(4.0));
        let sum = z + w;
        assert!((sum.re.to_f64() - 4.0).abs() < 1e-12);
        assert!((sum.im.to_f64() - 6.0).abs() < 1e-12);
        let diff = w - z;
        assert!((diff.re.to_f64() - 2.0).abs() < 1e-12);
        assert!((diff.im.to_f64() - 2.0).abs() < 1e-12);
        // (1+2i)(3+4i) = -5+10i
        let prod = z * w;
        assert!((prod.re.to_f64() + 5.0).abs() < 1e-12, "re={}", prod.re.to_f64());
        assert!((prod.im.to_f64() - 10.0).abs() < 1e-12, "im={}", prod.im.to_f64());
        // Kill *→+ on complex mul components.
        assert_ne!(prod.re.to_f64(), 1.0 * 3.0 + 2.0 * 4.0);

        let n2 = z.norm_squared();
        assert!((n2.to_f64() - 5.0).abs() < 1e-12);

        let ie = IntExp {
            val: rug::Integer::from(3),
            exp: 2,
        }; // 3 * 2^2 = 12
        let fe = FloatExp::from(ie);
        assert!((fe.to_f64() - 12.0).abs() < 1e-9, "{}", fe.to_f64());
        assert_eq!(FloatExp::from(IntExp::ZERO), FloatExp::ZERO);
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(128))]

        #[test]
        fn complex_mul_agrees_with_schoolbook(
            a0 in -1e2f64..1e2,
            a1 in -1e2f64..1e2,
            b0 in -1e2f64..1e2,
            b1 in -1e2f64..1e2,
        ) {
            let z = ComplexFloatExp::new(FloatExp::from(a0), FloatExp::from(a1));
            let w = ComplexFloatExp::new(FloatExp::from(b0), FloatExp::from(b1));
            let p = z * w;
            let expect_re = a0 * b0 - a1 * b1;
            let expect_im = a0 * b1 + a1 * b0;
            prop_assert!((p.re.to_f64() - expect_re).abs() < 1e-6 * (1.0 + expect_re.abs()));
            prop_assert!((p.im.to_f64() - expect_im).abs() < 1e-6 * (1.0 + expect_im.abs()));
        }

        #[test]
        fn sub_is_add_of_negation(a in -1e50f64..1e50, b in -1e50f64..1e50) {
            let fa = FloatExp::from(a);
            let fb = FloatExp::from(b);
            prop_assert_eq!(fa - fb, fa + (-fb));
        }
    }
}
