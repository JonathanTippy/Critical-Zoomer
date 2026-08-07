use crate::floatexp::{ComplexFloatExp, FloatExp};
use crate::reference::ReferenceOrbit;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PerturbedOutcome {
    Escapes { n: u32 },
    Repeats { period: u32 },
    Glitch,
    Unfinished,
}

fn near(a: ComplexFloatExp, b: ComplexFloatExp, epsilon: f64) -> bool {
    let epsilon_squared = FloatExp::from(epsilon * epsilon);
    (a - b).norm_squared() <= epsilon_squared
}

/// Iterate a pixel as a delta from an already computed reference orbit.
///
/// A missing reference iterate or detected perturbation glitch is reported
/// honestly; neither is converted into a guessed Mandelbrot answer.
// r[impl cz.depth.perturb-never-wrong+1]
pub fn iterate_pixel(
    reference: &ReferenceOrbit,
    dc: ComplexFloatExp,
    epsilon: f64,
    max_n: u32,
) -> PerturbedOutcome {
    if dc == ComplexFloatExp::ZERO {
        if let Some(period) = reference.period {
            return PerturbedOutcome::Repeats { period };
        }
    }

    let mut dz = ComplexFloatExp::ZERO;
    let mut checkpoint = ComplexFloatExp::ZERO;
    let mut checkpoint_n = 0;

    for n in 0..=max_n {
        let Some(z_reference) = reference.get(n) else {
            return PerturbedOutcome::Unfinished;
        };
        let z = z_reference + dz;

        // At the exact bailout circle, a delta below the stored mantissa can
        // decide which side the target lies on. Floatexp extends range, not
        // mantissa precision, so report the ambiguity instead of guessing.
        if dz != ComplexFloatExp::ZERO && z_reference.norm_squared() == FloatExp::from(4.0) {
            // |Z+δ|² - |Z|² = 2 Re(conj(Z)δ) + |δ|². Evaluate
            // the correction at delta scale so it is not absorbed by 4.0.
            let correction = FloatExp::TWO * (z_reference.re * dz.re + z_reference.im * dz.im)
                + dz.norm_squared();
            if correction > FloatExp::ZERO {
                return PerturbedOutcome::Escapes { n };
            }
            if correction == FloatExp::ZERO {
                return PerturbedOutcome::Glitch;
            }
        }

        if z.norm_squared() > FloatExp::from(4.0) {
            return PerturbedOutcome::Escapes { n };
        }

        if n > 0 && near(z, checkpoint, epsilon) {
            return PerturbedOutcome::Repeats {
                period: n - checkpoint_n,
            };
        }

        // Pauldelbrot glitch criterion: cancellation makes the reconstructed
        // orbit much smaller than its reference, so reference rounding can
        // dominate. Unknown is safer than a corrupted answer.
        if n > 0 && z.norm_squared() < z_reference.norm_squared() * FloatExp::from(1.0e-6) {
            return PerturbedOutcome::Glitch;
        }

        if n >= max_n {
            break;
        }
        if n >= checkpoint_n.saturating_mul(2).max(1) {
            checkpoint = z;
            checkpoint_n = n;
        }

        // δz_{n+1} = 2 Z_n δz_n + δz_n² + δc
        dz = z_reference * dz * ComplexFloatExp::new(FloatExp::TWO, FloatExp::ZERO) + dz * dz + dc;
    }
    PerturbedOutcome::Unfinished
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::IntExp;
    use proptest::prelude::*;
    use rug::Float;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum OracleOutcome {
        Escapes(u32),
        Unfinished,
    }

    fn naive(c: &(IntExp, IntExp), precision: u32, max_n: u32) -> OracleOutcome {
        let to_float = |v: &IntExp| {
            let mut f = Float::with_val(precision, &v.val);
            if v.exp >= 0 {
                f <<= v.exp;
            } else {
                f >>= -v.exp;
            }
            f
        };
        let cr = to_float(&c.0);
        let ci = to_float(&c.1);
        let mut zr = Float::with_val(precision, 0);
        let mut zi = Float::with_val(precision, 0);
        for n in 0..=max_n {
            let norm = Float::with_val(
                precision,
                Float::with_val(precision, &zr * &zr) + Float::with_val(precision, &zi * &zi),
            );
            if norm > 4 {
                return OracleOutcome::Escapes(n);
            }
            let next_re = Float::with_val(
                precision,
                Float::with_val(precision, &zr * &zr) - Float::with_val(precision, &zi * &zi) + &cr,
            );
            let next_im =
                Float::with_val(precision, Float::with_val(precision, &zr * &zi) * 2 + &ci);
            zr = next_re;
            zi = next_im;
        }
        OracleOutcome::Unfinished
    }

    fn doubling_oracle(c: &(IntExp, IntExp), max_n: u32) -> Option<OracleOutcome> {
        // Starting below the dyadic input's own significand can produce two
        // identical *wrong* answers: both precisions merely erase the same
        // low bit. Represent c exactly first, then seek stability by doubling.
        let input_bits = c.0.val.significant_bits().max(c.1.val.significant_bits()) as u32;
        let mut bits = input_bits.saturating_add(32).max(64);
        let mut previous = None;
        for _ in 0..6 {
            let answer = naive(c, bits, max_n);
            if previous == Some(answer) {
                return Some(answer);
            }
            previous = Some(answer);
            bits = bits.saturating_mul(2);
        }
        None
    }

    proptest! {
        // r[verify cz.depth.oracle-doubling+1 cz.depth.perturb-never-wrong+1]
        #[test]
        fn perturbation_matches_precision_doubling_oracle_for_exteriors(
            real_numerator in 17i32..64,
            imag_numerator in -16i32..16,
            depth in 60i32..1500,
        ) {
            // References c=2; targets differ at depths well beyond f64.
            let reference_c = (IntExp::from(2), IntExp::ZERO);
            let dc_objective = (
                IntExp::from(real_numerator).shift(-depth),
                IntExp::from(imag_numerator).shift(-depth),
            );
            let target_c = (
                IntExp {
                    val: (rug::Integer::from(2) << depth as u32) + real_numerator,
                    exp: -depth,
                },
                IntExp {
                    val: rug::Integer::from(imag_numerator),
                    exp: -depth,
                },
            );
            let oracle = doubling_oracle(&target_c, 64);
            prop_assume!(oracle.is_some());

            let reference = ReferenceOrbit::compute(&reference_c, depth as u32 + 64, 64);
            let dc = ComplexFloatExp::new(
                FloatExp::from(dc_objective.0),
                FloatExp::from(dc_objective.1),
            );
            let actual = iterate_pixel(&reference, dc, 1.0e-15, 64);
            match (oracle.unwrap(), actual) {
                (OracleOutcome::Escapes(expected), PerturbedOutcome::Escapes { n }) =>
                    prop_assert_eq!(n, expected),
                (_, PerturbedOutcome::Glitch | PerturbedOutcome::Unfinished) => (),
                (expected, got) => prop_assert!(false,
                    "perturbed answer {:?} disagrees with oracle {:?}", got, expected),
            }
        }
    }

    #[test]
    fn exact_reference_matches_naive_escape_time() {
        let c = (IntExp::from(2), IntExp::ZERO);
        let reference = ReferenceOrbit::compute(&c, 128, 16);
        assert_eq!(
            iterate_pixel(&reference, ComplexFloatExp::ZERO, 1.0e-15, 16),
            PerturbedOutcome::Escapes { n: 2 },
        );
    }

    #[test]
    // r[verify cz.depth.floatexp-range+1 cz.depth.oracle-doubling+1]
    fn deep_delta_runs_without_f64_underflow() {
        let reference_c = (IntExp::from(1), IntExp::ZERO);
        let dc_re = IntExp::from(1).shift(-1500);
        let target = (
            IntExp {
                val: (rug::Integer::from(1) << 1500) + 1,
                exp: -1500,
            },
            IntExp::ZERO,
        );
        let expected = doubling_oracle(&target, 16).unwrap();
        let reference = ReferenceOrbit::compute(&reference_c, 1600, 16);
        let actual = iterate_pixel(
            &reference,
            ComplexFloatExp::new(FloatExp::from(dc_re), FloatExp::ZERO),
            1.0e-15,
            16,
        );
        assert_eq!(expected, OracleOutcome::Escapes(2));
        assert_eq!(actual, PerturbedOutcome::Escapes { n: 2 });
    }

    #[test]
    fn periodic_reference_detects_repeat() {
        let c = (IntExp::from(-1), IntExp::ZERO);
        let reference = ReferenceOrbit::compute(&c, 128, 8);
        assert_eq!(
            iterate_pixel(&reference, ComplexFloatExp::ZERO, 1.0e-15, 8),
            PerturbedOutcome::Repeats { period: 2 },
        );
    }

    #[test]
    fn missing_reference_is_unfinished_not_wrong() {
        let c = (IntExp::from(1).shift(-2), IntExp::from(1).shift(-3));
        let reference = ReferenceOrbit::compute(&c, 128, 2);
        assert_eq!(
            iterate_pixel(&reference, ComplexFloatExp::ZERO, 1.0e-15, 50),
            PerturbedOutcome::Unfinished,
        );
    }
}
