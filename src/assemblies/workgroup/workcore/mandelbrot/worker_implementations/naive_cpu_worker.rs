//! Test-only naive (non-perturbation) Mandelbrot iterate.
//!
//! Live tile work always uses perturbation (+ zero-orbit fallback). This module
//! exists solely under `cfg(test)` for symmetry / oracle property tests.
#![cfg(test)]

use crate::assemblies::structs::*;
use crate::assemblies::workgroup::structs::*;
use crate::assemblies::workgroup::structs::mandelbrotable::*;
use crate::assemblies::workgroup::workcore::mandelbrot::*;
use crate::assemblies::workgroup::workcore::mandelbrot::worker_implementations::periodicity_detector::*;
use crate::range::*;

/// Generic naive bout — test oracle only (not a live Worker).
#[inline(always)]
// r[impl cz.math.mandelbrot-real-axis-symmetry+1]
pub fn iterate_point_bout<T, P>(
    point: &mut ActivePoint<T, P>
    , bailout_radius_squared: T
    , epsilon: T
    , bout_iterations: u32
)
where
    T: Mandelbrotable
    , P: PeriodicityDetector<T>
{
    let mut z = point.z;
    let mut d = point.derivative;
    point.real_squared = z.0 * z.0;
    point.imag_squared = z.1 * z.1;
    point.real_imag = z.0 * z.1;
    for _ in 0..bout_iterations {
        if point.finished {
            break;
        }
        advance_orbit_step(&mut z, &mut d, point.c);
        point.iteration_count += 1;
        point.real_squared = z.0 * z.0;
        point.imag_squared = z.1 * z.1;
        point.real_imag = z.0 * z.1;
        let rad = point.real_squared + point.imag_squared;
        if rad < point.min_magnitude {
            point.min_magnitude = rad;
            point.min_magnitude_time = point.iteration_count;
        }
        if rad > bailout_radius_squared {
            point.z = z;
            point.derivative = d;
            point.escaped = true;
            point.finished = true;
            break;
        }
        if point
            .periodicity_detector
            .check_periodicity(point.c, z, d, point.iteration_count, epsilon)
            .is_some()
        {
            point.z = z;
            point.derivative = d;
            point.finished = true;
            break;
        }
    }
    if !point.finished {
        point.z = z;
        point.derivative = d;
    }
}

fn derivative_magnitude_angle<T: Mandelbrotable>(d: (T, T)) -> u8 {
    let a = d.1.to_f64().atan2(d.0.to_f64());
    let turns = a / (2.0 * std::f64::consts::PI);
    let u = (turns.rem_euclid(1.0) * 256.0).floor() as i32;
    (u.clamp(0, 255)) as u8
}

pub fn point_to_answer<T, P>(point: &ActivePoint<T, P>) -> Answer
where
    T: Mandelbrotable
    , P: PeriodicityDetector<T>
{
    let escape_time_angle = derivative_magnitude_angle(point.derivative);
    let min_magnitude_angle = escape_time_angle;
    if point.escaped {
        Answer {
            result: MandelbrotResult::Outside {
                escape_time_r2: point.iteration_count
                , escape_z: (point.z.0.to_f32(), point.z.1.to_f32())
            }
            , min_magnitude_time: point.min_magnitude_time
            , min_magnitude: point.min_magnitude.to_f64()
            , escape_time_angle
            , min_magnitude_angle
        }
    } else {
        Answer {
            result: MandelbrotResult::Inside { period: 0 }
            , min_magnitude_time: point.min_magnitude_time
            , min_magnitude: point.min_magnitude.to_f64()
            , escape_time_angle
            , min_magnitude_angle
        }
    }
}

mod b_per_2_tests {
    use super::*;

    #[test]
    fn regular_inside_answer_emits_unknown_period_zero() {
        let z = (0.0, 0.0);
        let point = ActivePoint {
            c: (-0.5, 0.0)
            , z
            , derivative: (1.0, 0.0)
            , real_squared: 0.0
            , imag_squared: 0.0
            , real_imag: 0.0
            , iteration_count: 100
            , min_magnitude: 0.0
            , min_magnitude_time: 1
            , periodicity_detector: CpuPeriodicityDetector::init(0, z, (1.0, 0.0))
            , escaped: false
            , finished: true
            , orbit_id: ZERO_ORBIT_ID
            , seat_linear: 0
        };
        let answer = point_to_answer(&point);
        match answer.result {
            MandelbrotResult::Inside { period } => {
                assert_eq!(period, 0, "regular iterate must not emit a period number");
            }
            other => panic!("expected Inside, got {other:?}"),
        }
    }
}

mod symmetry_tests {
    use super::*;
    use proptest::prelude::*;

    fn fresh_point(c: (f64, f64)) -> ActivePoint<f64, CpuPeriodicityDetector> {
        let z = (0.0, 0.0);
        ActivePoint {
            c
            , z
            , derivative: (1.0, 0.0)
            , real_squared: 0.0
            , imag_squared: 0.0
            , real_imag: 0.0
            , iteration_count: 0
            , min_magnitude: f64::MAX
            , min_magnitude_time: 0
            , periodicity_detector: CpuPeriodicityDetector::init(0, z, (1.0, 0.0))
            , escaped: false
            , finished: false
            , orbit_id: ZERO_ORBIT_ID
            , seat_linear: 0
        }
    }

    fn finish(c: (f64, f64)) -> Answer {
        let mut p = fresh_point(c);
        for _ in 0..50 {
            if p.finished {
                break;
            }
            iterate_point_bout(&mut p, 4.0, 1e-14, 200);
        }
        point_to_answer(&p)
    }

    fn same_class(a: &Answer, b: &Answer) -> bool {
        match (&a.result, &b.result) {
            (
                MandelbrotResult::Outside {
                    escape_time_r2: ea, ..
                }
                , MandelbrotResult::Outside {
                    escape_time_r2: eb, ..
                }
            ) => ea == eb
            , (MandelbrotResult::Inside { .. }, MandelbrotResult::Inside { .. }) => true
            , _ => false
        }
    }

    // r[verify cz.math.mandelbrot-real-axis-symmetry+1]
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]
        #[test]
        fn mandelbrot_conjugate_symmetry(
            re in prop_oneof![
                3 => -2.0f64..0.5,
                1 => -0.75f64..-0.7,
            ],
            im in prop_oneof![
                3 => -1.2f64..1.2,
                1 => -0.1f64..0.1,
            ],
        ) {
            let a = finish((re, im));
            let b = finish((re, -im));
            prop_assert!(
                same_class(&a, &b),
                "asymmetry at c=({re},{im}): {a:?} vs {b:?}"
            );
        }
    }

    // r[verify cz.math.mandelbrot-real-axis-symmetry+1]
    #[test]
    fn seahorse_valley_conjugates_match() {
        let a = finish((-0.75, 0.1));
        let b = finish((-0.75, -0.1));
        assert!(same_class(&a, &b), "{a:?} vs {b:?}");
    }

    // r[verify cz.math.mandelbrot-real-axis-symmetry+1]
    #[test]
    fn cardioid_sample_conjugates_match() {
        let a = finish((-0.5, 0.25));
        let b = finish((-0.5, -0.25));
        assert!(same_class(&a, &b), "{a:?} vs {b:?}");
    }
}
