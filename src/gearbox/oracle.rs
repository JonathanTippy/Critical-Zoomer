//! Test-only Oracle gear: absolute FloatExp (“slidy”) naive Mandelbrot.
//!
//! Never selected by production `workshift` dispatch. Used to judge answer
//! quality of other gears at arbitrary depth where f64 DirectKernel is not
//! a membership oracle.
//!
//! r[impl cz.depth.oracle-gear+1]

use crate::floatexp::FloatExp;

/// One concluded Oracle answer (escape or interior repeat signal).
#[derive(Clone, Debug, PartialEq)]
pub enum OracleAnswer {
    Escapes {
        escape_time: u32,
    },
    /// Period unknown / not verified here — Oracle only signals repeat vs escape.
    Repeats {
        iterations: u32,
    },
    /// Hit bout cap without concluding (caller must continue).
    Unfinished {
        iterations: u32,
        z: (FloatExp, FloatExp),
    },
}

/// Stateless absolute FloatExp iterator (no reference, no series).
#[derive(Clone, Copy, Debug, Default)]
pub struct OracleKernel;

/// Iterate absolute `z ← z² + c` in FloatExp for up to `max_iters` steps.
///
/// `r_squared` is bailout radius squared (typically 4). `epsilon` is the
/// loop-detect magnitude of `z − z_prev` in FloatExp (use a tiny value).
pub fn iterate_oracle_bout(
    c: (FloatExp, FloatExp),
    mut z: (FloatExp, FloatExp),
    r_squared: FloatExp,
    epsilon: FloatExp,
    max_iters: u32,
    start_iterations: u32,
) -> OracleAnswer {
    let mut iterations = start_iterations;
    let mut prev = z;
    for _ in 0..max_iters {
        let zz_re = z.0 * z.0 - z.1 * z.1 + c.0;
        let zz_im = FloatExp::TWO * z.0 * z.1 + c.1;
        z = (zz_re, zz_im);
        iterations = iterations.saturating_add(1);

        let mag_sq = z.0 * z.0 + z.1 * z.1;
        if mag_sq > r_squared {
            return OracleAnswer::Escapes {
                escape_time: iterations,
            };
        }

        let d_re = z.0 - prev.0;
        let d_im = z.1 - prev.1;
        let d_sq = d_re * d_re + d_im * d_im;
        if d_sq < epsilon * epsilon && iterations > 2 {
            return OracleAnswer::Repeats { iterations };
        }
        prev = z;
    }
    OracleAnswer::Unfinished {
        iterations,
        z,
    }
}

impl OracleKernel {
    /// Run until escape/repeat or `max_iters` from `z = c`.
    pub fn conclude(
        &self,
        c: (FloatExp, FloatExp),
        r_squared: FloatExp,
        epsilon: FloatExp,
        max_iters: u32,
    ) -> OracleAnswer {
        iterate_oracle_bout(c, c, r_squared, epsilon, max_iters, 0)
    }
}

/// Convert a finite f64 pair into FloatExp absolute coordinates.
pub fn c_from_f64(c: (f64, f64)) -> (FloatExp, FloatExp) {
    (FloatExp::from(c.0), FloatExp::from(c.1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::floatexp::FloatExp;

    #[test]
    // r[verify cz.depth.oracle-gear+1]
    fn oracle_escapes_far_exterior() {
        let c = c_from_f64((3.0, 0.0));
        let ans = OracleKernel.conclude(
            c,
            FloatExp::from(4.0),
            FloatExp::from(1e-15),
            64,
        );
        match ans {
            OracleAnswer::Escapes { escape_time } => assert_eq!(escape_time, 1),
            other => panic!("expected escape, got {other:?}"),
        }
    }

    #[test]
    // r[verify cz.depth.oracle-gear+1]
    fn oracle_marks_cardioid_center_repeat() {
        let c = c_from_f64((0.0, 0.0));
        let ans = OracleKernel.conclude(
            c,
            FloatExp::from(4.0),
            FloatExp::from(1e-20),
            256,
        );
        assert!(
            matches!(ans, OracleAnswer::Repeats { .. }),
            "0+0i must repeat under Oracle, got {ans:?}"
        );
    }

    #[test]
    // r[verify cz.depth.oracle-gear+1]
    fn oracle_matches_direct_escape_time_on_shallow_sample() {
        // Mirror DirectKernel production convention: escape_time counts iterations.
        let samples = [(3.0, 0.0), (2.0, 2.0), (-2.5, 0.5), (1.5, 1.5)];
        for c64 in samples {
            let mut z = c64;
            let mut et = 0u32;
            for _ in 0..64 {
                let zr = z.0 * z.0 - z.1 * z.1 + c64.0;
                let zi = 2.0 * z.0 * z.1 + c64.1;
                z = (zr, zi);
                et += 1;
                if z.0 * z.0 + z.1 * z.1 > 4.0 {
                    break;
                }
            }
            let ans = OracleKernel.conclude(
                c_from_f64(c64),
                FloatExp::from(4.0),
                FloatExp::from(1e-30),
                64,
            );
            match ans {
                OracleAnswer::Escapes { escape_time } => {
                    assert_eq!(
                        escape_time, et,
                        "Oracle vs f64 naive escape_time for {c64:?}"
                    );
                }
                other => panic!("expected escape for {c64:?}, got {other:?}"),
            }
        }
    }
}
