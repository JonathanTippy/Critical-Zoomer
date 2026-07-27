use crate::assemblies::structs::*;
use crate::assemblies::workgroup_new::structs::*;
use crate::assemblies::workgroup_new::structs::mandelbrotable::*;
use crate::assemblies::workgroup_new::workcore::mandelbrot::*;
use crate::assemblies::workgroup_new::workcore::mandelbrot::worker_implementations::periodicity_detector::*;
use crate::range::*;

pub struct NaiveCpuWorker;

#[derive(Clone, Debug)]
pub struct NaiveCpuWorkerState {
    pub bailout_radius_squared: f64
    , pub iterations_per_bout: u32
    , pub stencil: Option<PointStencil>
}

impl Default for NaiveCpuWorkerState {
    fn default() -> Self {
        NaiveCpuWorkerState {
            bailout_radius_squared: 4.0
            , iterations_per_bout: 1000
            , stencil: None
        }
    }
}

impl Worker<f64, CpuPeriodicityDetector> for NaiveCpuWorker {
    type State = NaiveCpuWorkerState;

    fn initialize_batch<const N: usize>(
        worker_state: &Self::State
        , active_tile: &Tile<()>
        , seats: [Option<(usize, usize)>; N]
    ) -> PointBatch<f64, CpuPeriodicityDetector, N> {
        let Some(stencil) = worker_state.stencil.as_ref() else {
            return PointBatch { points: [const { None }; N] };
        };
        let generator = match stencil.get_c_generator::<f64>() {
            Some(g) => g
            , None => {
                return PointBatch { points: [const { None }; N] };
            }
        };
        let mut points: [Option<((usize, usize), ActivePoint<f64, CpuPeriodicityDetector>)>; N] =
            [const { None }; N];
        for i in 0..N {
            if let Some(local) = seats[i] {
                let seat = active_tile.screen_seat(local);
                if seat.0 >= stencil.resolution.0
                    || seat.1 >= stencil.resolution.1
                {
                    continue;
                }
                let c = generator.get_c((
                    seat.0.min(u16::MAX as usize) as u16
                    , seat.1.min(u16::MAX as usize) as u16
                ));
                let z = (f64::ZERO, f64::ZERO);
                let derivative = (f64::ONE, f64::ZERO);
                points[i] = Some((
                    local
                    , ActivePoint {
                        c
                        , z
                        , derivative
                        , real_squared: f64::ZERO
                        , imag_squared: f64::ZERO
                        , real_imag: f64::ZERO
                        , iteration_count: 0
                        , min_magnitude: f64::MAX
                        , min_magnitude_time: 0
                        , periodicity_detector: CpuPeriodicityDetector::init(0, z, derivative)
                        , escaped: false
                        , finished: false
                        , orbit_id: ZERO_ORBIT_ID
                        , seat_linear: 0
                    }
                ));
            }
        }
        PointBatch { points }
    }

    fn workshift_on_batch<const N: usize>(
        worker_state: &mut Self::State
        , active_batch: &mut PointBatch<f64, CpuPeriodicityDetector, N>
    ) -> bool {
        let mut any_incomplete = false;
        for slot in active_batch.points.iter_mut() {
            if let Some((_, point)) = slot {
                if point.finished { continue; }
                any_incomplete = true;
                let epsilon = 1e-12f64.max(point.c.0.abs().max(point.c.1.abs()) * 1e-6);
                iterate_point_bout(
                    point
                    , worker_state.bailout_radius_squared
                    , epsilon
                    , worker_state.iterations_per_bout
                );
            }
        }
        any_incomplete
    }

    fn peek_batch<const N: usize>(
        active_batch: &PointBatch<f64, CpuPeriodicityDetector, N>
        , active_tile: &Tile<()>
    ) -> [Option<((usize, usize), CalibratedAnswer)>; N] {
        let _ = active_tile;
        let mut out: [Option<((usize, usize), CalibratedAnswer)>; N] = [const { None }; N];
        for i in 0..N {
            if let Some((seat, point)) = &active_batch.points[i] {
                if point.finished {
                    out[i] = Some((*seat, point_to_calibrated_answer(point)));
                }
            }
        }
        out
    }

    fn pack_batches<const N: usize, const B: usize>(
        batches: [PointBatch<f64, CpuPeriodicityDetector, N>; B]
    ) -> [Option<PointBatch<f64, CpuPeriodicityDetector, N>>; B] {
        batches.map(Some)
    }
}

#[inline(always)]
pub fn iterate_point_bout(
    point: &mut ActivePoint<f64, CpuPeriodicityDetector>
    , bailout_radius_squared: f64
    , epsilon: f64
    , bout_iterations: u32
) {
    let mut z_re = point.z.0;
    let mut z_im = point.z.1;
    let mut d_re = point.derivative.0;
    let mut d_im = point.derivative.1;
    let c_re = point.c.0;
    let c_im = point.c.1;
    point.real_squared = z_re * z_re;
    point.imag_squared = z_im * z_im;
    point.real_imag = z_re * z_im;
    for _ in 0..bout_iterations {
        if point.finished { break; }
        let old_re = z_re;
        let old_im = z_im;
        let new_d_re = 2.0 * (old_re * d_re - old_im * d_im);
        let new_d_im = 2.0 * (old_re * d_im + old_im * d_re);
        d_re = new_d_re;
        d_im = new_d_im;
        z_re = old_re * old_re - old_im * old_im + c_re;
        z_im = 2.0 * old_re * old_im + c_im;
        point.iteration_count += 1;
        point.real_squared = z_re * z_re;
        point.imag_squared = z_im * z_im;
        point.real_imag = z_re * z_im;
        let rad = z_re * z_re + z_im * z_im;
        if rad < point.min_magnitude {
            point.min_magnitude = rad;
            point.min_magnitude_time = point.iteration_count;
        }
        if rad > bailout_radius_squared {
            point.z = (z_re, z_im);
            point.derivative = (d_re, d_im);
            point.escaped = true;
            point.finished = true;
            break;
        }
        if point.periodicity_detector.check_periodicity(
            point.c
            , (z_re, z_im)
            , (d_re, d_im)
            , point.iteration_count
            , epsilon
        ).is_some() {
            point.z = (z_re, z_im);
            point.derivative = (d_re, d_im);
            point.finished = true;
            break;
        }
    }
    if !point.finished {
        point.z = (z_re, z_im);
        point.derivative = (d_re, d_im);
    }
}

pub fn point_to_answer(point: &ActivePoint<f64, CpuPeriodicityDetector>) -> Answer {
    if point.escaped {
        Answer {
            result: MandelbrotResult::Outside {
                escape_time_r2: point.iteration_count
                , escape_z: (point.z.0 as f32, point.z.1 as f32)
            }
            , min_magnitude_time: point.min_magnitude_time
            , min_magnitude: point.min_magnitude
        }
    } else {
        Answer {
            result: MandelbrotResult::Inside {
                period: 0
            }
            , min_magnitude_time: point.min_magnitude_time
            , min_magnitude: point.min_magnitude
        }
    }
}

fn exact_range<T: crate::range::Value>(value: T) -> crate::range::Range<T> {
    crate::range::Range { lower_bound: value, upper_bound: value }
}

fn point_to_calibrated_answer(point: &ActivePoint<f64, CpuPeriodicityDetector>) -> CalibratedAnswer {
    let answer = point_to_answer(point);
    match answer.result {
        MandelbrotResult::Outside { escape_time_r2, escape_z } => {
            CalibratedAnswer {
                result: CalibratedMandelbrotResult::Outside {
                    escape_time_r2: exact_range(escape_time_r2)
                    , escape_z: (exact_range(escape_z.0), exact_range(escape_z.1))
                }
                , min_magnitude_time: exact_range(answer.min_magnitude_time)
                , min_magnitude: exact_range(answer.min_magnitude)
                , highlights: CalibratedHighlights {
                    in_filament: exact_range(false)
                    , out_filament: exact_range(false)
                    , small_time_edge: exact_range(false)
                    , node: exact_range(false)
                }
            }
        }
        , MandelbrotResult::Inside { period } => {
            CalibratedAnswer {
                result: CalibratedMandelbrotResult::Inside {
                    period: exact_range(period)
                }
                , min_magnitude_time: exact_range(answer.min_magnitude_time)
                , min_magnitude: exact_range(answer.min_magnitude)
                , highlights: CalibratedHighlights {
                    in_filament: exact_range(false)
                    , out_filament: exact_range(false)
                    , small_time_edge: exact_range(false)
                    , node: exact_range(false)
                }
            }
        }
    }
}

#[cfg(test)]
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

#[cfg(test)]
mod symmetry_tests {
    use super::*;
    use proptest::prelude::*;

    fn fresh_point(c: (f64, f64)) -> ActivePoint<f64, CpuPeriodicityDetector> {
        let z = (0.0, 0.0);
        ActivePoint {
            c,
            z,
            derivative: (1.0, 0.0),
            real_squared: 0.0,
            imag_squared: 0.0,
            real_imag: 0.0,
            iteration_count: 0,
            min_magnitude: f64::MAX,
            min_magnitude_time: 0,
            periodicity_detector: CpuPeriodicityDetector::init(0, z, (1.0, 0.0)),
            escaped: false,
            finished: false,
            orbit_id: ZERO_ORBIT_ID,
            seat_linear: 0,
        }
    }

    fn finish(c: (f64, f64)) -> Answer {
        let mut p = fresh_point(c);
        // Enough bouts for shallow samples used in the property.
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
                },
                MandelbrotResult::Outside {
                    escape_time_r2: eb, ..
                },
            ) => ea == eb,
            (MandelbrotResult::Inside { .. }, MandelbrotResult::Inside { .. }) => true,
            _ => false,
        }
    }

    // r[verify cz.math.mandelbrot-real-axis-symmetry+1]
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]
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

    #[test]
    fn seahorse_valley_conjugates_match() {
        let a = finish((-0.75, 0.1));
        let b = finish((-0.75, -0.1));
        assert!(same_class(&a, &b), "{a:?} vs {b:?}");
    }

    #[test]
    fn cardioid_sample_conjugates_match() {
        let a = finish((-0.5, 0.25));
        let b = finish((-0.5, -0.25));
        assert!(same_class(&a, &b), "{a:?} vs {b:?}");
    }
}
