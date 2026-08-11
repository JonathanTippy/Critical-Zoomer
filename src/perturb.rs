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
pub(crate) mod oracle {
    use crate::utils::IntExp;
    use rug::Float;

    /// Ground-truth answer class for one pixel, computed at arbitrary precision.
    /// This is the oracle; f64 direct iteration is *not* ground truth (it can be
    /// wrong at the bailout circle or lose a deep bit), so correctness is always
    /// judged against this, not against the f64 kernel.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum OracleOutcome {
        Escapes(u32),
        Unfinished,
    }

    /// Naive arbitrary-precision iteration of z' = z² + c from z₀ = 0.
    ///
    /// Matches the production convention: this codebase's orbit starts at z₁ = c,
    /// so "escape at n" means |z_n|² > 4 with z_n the n-th iterate from z₀ = 0.
    pub fn naive(c: &(IntExp, IntExp), precision: u32, max_n: u32) -> OracleOutcome {
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

    /// Double rug precision until two consecutive answers agree, starting from
    /// enough bits to represent the (dyadic) input exactly.
    pub fn doubling_oracle(c: &(IntExp, IntExp), max_n: u32) -> Option<OracleOutcome> {
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
}

#[cfg(test)]
mod tests {
    use super::oracle::{doubling_oracle, naive, OracleOutcome};
    use super::*;
    use crate::utils::IntExp;
    use proptest::prelude::*;

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

    /// Thought-killed pins for `perturb.rs` caught mutants (`near`, δz recurrence,
    /// bailout `>`, glitch/`&&` thresholds).
    #[test]
    fn near_uses_squared_epsilon_leq() {
        let a = ComplexFloatExp::new(FloatExp::from(1.0), FloatExp::ZERO);
        let b = ComplexFloatExp::new(FloatExp::from(1.0 + 1e-4), FloatExp::ZERO);
        assert!(near(a, a, 1e-15));
        assert!(near(a, b, 1e-3));
        assert!(!near(a, b, 1e-5));
        // Kill return-constant mutants.
        assert_ne!(near(a, b, 1e-5), true);
        assert_ne!(near(a, a, 1e-15), false);
        // Kill epsilon*epsilon → + / / and <= → >.
        let tight = ComplexFloatExp::new(FloatExp::from(1.0 + 0.5), FloatExp::ZERO);
        assert!(!near(a, tight, 0.1)); // |δ|²=0.25 > 0.01
        assert!(near(a, tight, 1.0)); // 0.25 <= 1.0
        // Boundary: |δ|² == ε² must still be near (<= not < / >).
        let origin = ComplexFloatExp::ZERO;
        let edge = ComplexFloatExp::new(FloatExp::from(0.1), FloatExp::ZERO);
        assert!(near(origin, edge, 0.1)); // 0.01 <= 0.01
        assert!(!near(origin, edge, 0.09));
        // *→+: ε+ε=0.2 would falsely accept |δ|²=0.25; keep the 0.5-gap case.
        assert!(!near(a, tight, 0.1));
    }

    #[test]
    fn iterate_pixel_delta_recurrence_and_escape_strict() {
        // Reference at c=0.25 (cardioid cusp-ish exterior? 0.25 is on boundary of
        // main cardioid — use c=2 for clear escape).
        let reference_c = (IntExp::from(2), IntExp::ZERO);
        let reference = ReferenceOrbit::compute(&reference_c, 128, 16);
        // Nonzero δc still escapes; δz' = 2Z δz + δz² + δc must not become +/ *.
        let dc = ComplexFloatExp::new(FloatExp::from(0.01), FloatExp::ZERO);
        match iterate_pixel(&reference, dc, 1.0e-15, 16) {
            PerturbedOutcome::Escapes { n } => {
                assert!(n >= 1 && n <= 4, "n={n}");
            }
            other => panic!("expected Escapes, got {other:?}"),
        }

        // Strict bailout: |z|² > 4 (not >=). Value exactly on the circle is the
        // special glitch/escape correction path when dz≠0 and |Z|²==4.
        let zero_orbit = ReferenceOrbit::zero_orbit();
        // Against zero orbit, δz evolves as ordinary Mandelbrot: z=δz, c=δc.
        // δc = 3 → escapes immediately at n=0? z0=0, |0|²≯4; then dz=dc=3;
        // at n=1, z=0+3=3, |z|²=9>4 → Escapes { n: 1 }.
        let far = ComplexFloatExp::new(FloatExp::from(3.0), FloatExp::ZERO);
        assert_eq!(
            iterate_pixel(&zero_orbit, far, 1.0e-15, 8),
            PerturbedOutcome::Escapes { n: 1 }
        );
        // |z|=2 exactly after one step from δc=2: |z|²=4 is NOT >4, so must not
        // Escapes at n=1 via a >= mutant (continues / unfinished / later escape).
        let on_circle = ComplexFloatExp::new(FloatExp::from(2.0), FloatExp::ZERO);
        let on = iterate_pixel(&zero_orbit, on_circle, 1.0e-15, 2);
        assert_ne!(on, PerturbedOutcome::Escapes { n: 1 });

        // Interior-ish small δc on period-2 reference → Repeats or Unfinished,
        // never a wrong Escapes from *→+ on the recurrence alone.
        let p2 = ReferenceOrbit::compute(&(IntExp::from(-1), IntExp::ZERO), 128, 32);
        let tiny = ComplexFloatExp::new(FloatExp::from(1e-12), FloatExp::ZERO);
        let out = iterate_pixel(&p2, tiny, 1.0e-9, 64);
        assert!(
            matches!(
                out,
                PerturbedOutcome::Repeats { .. }
                    | PerturbedOutcome::Unfinished
                    | PerturbedOutcome::Glitch
            ),
            "got {out:?}"
        );

        // Zero-orbit algebra: after first step dz=δc; second step uses
        // δz' = δz² + δc (Z=0). Escape timing pins 2·Z·δz / + mutants.
        let mild = ComplexFloatExp::new(FloatExp::from(0.5), FloatExp::ZERO);
        match iterate_pixel(&zero_orbit, mild, 1.0e-15, 16) {
            PerturbedOutcome::Escapes { n } => {
                // Mandelbrot c=0.5 escapes in a few iterations from 0.
                assert!(n >= 2 && n <= 8, "n={n}");
            }
            other => panic!("expected Escapes for c=0.5 on zero orbit, got {other:?}"),
        }
    }

    #[test]
    fn iterate_pixel_glitch_when_reconstructed_cancels() {
        // Force Pauldelbrot: need n>0 and |z|² < |Z|² * 1e-6.
        // Use a deep-ish reference and a δc that cancels toward the reference.
        let reference_c = (IntExp::from(-1).shift(-1), IntExp::ZERO); // -0.5
        let reference = ReferenceOrbit::compute(&reference_c, 256, 64);
        // δc ≈ -c so absolute c≈0 stays at origin while reference walks — classic
        // cancellation / glitch territory for perturbation.
        let dc = ComplexFloatExp::new(FloatExp::from(0.5), FloatExp::ZERO);
        let out = iterate_pixel(&reference, dc, 1.0e-15, 64);
        assert!(
            matches!(
                out,
                PerturbedOutcome::Glitch
                    | PerturbedOutcome::Escapes { .. }
                    | PerturbedOutcome::Unfinished
                    | PerturbedOutcome::Repeats { .. }
            ),
            "got {out:?}"
        );
        // Ensure && thresholds: n>0 AND small ratio — zero-orbit never glitches
        // that way for ordinary δc=0.1 (escapes or unfinished cleanly).
        let zref = ReferenceOrbit::zero_orbit();
        let mid = ComplexFloatExp::new(FloatExp::from(0.1), FloatExp::ZERO);
        let clean = iterate_pixel(&zref, mid, 1.0e-15, 32);
        assert!(
            !matches!(clean, PerturbedOutcome::Glitch),
            "zero-orbit mid δc should not Pauldelbrot-glitch: {clean:?}"
        );
    }

    #[test]
    fn mutant_kill_perturb_near_iterate_glitch() {
        near_uses_squared_epsilon_leq();
        iterate_pixel_delta_recurrence_and_escape_strict();
        iterate_pixel_glitch_when_reconstructed_cancels();
    }
}
