use crate::assemblies::structs::*;
use crate::assemblies::workgroup_new::structs::*;
use crate::assemblies::workgroup_new::structs::mandelbrotable::*;
use crate::assemblies::workgroup_new::workcore::mandelbrot::*;
use crate::assemblies::workgroup_new::workcore::mandelbrot::worker_implementations::periodicity_detector::*;
use crate::constants::*;
use crate::range::*;

pub struct PerturbationCpuWorker;

pub struct PerturbationCpuWorkerState {
    pub bailout_radius_squared: f64
    , pub iterations_per_bout: u32
    , pub stencil: Option<PointStencil>
    , pub references: ReferenceCollection
    , pub seat_orbit_ids: Vec<OrbitId>
    , pub screen_width: usize
    , pub gear: crate::gear::Gear
}

impl Default for PerturbationCpuWorkerState {
    fn default() -> Self {
        PerturbationCpuWorkerState {
            bailout_radius_squared: 4.0
            , iterations_per_bout: 1000
            , stencil: None
            , references: ReferenceCollection::new()
            , seat_orbit_ids: Vec::new()
            , screen_width: 0
            , gear: crate::gear::Gear::F64
        }
    }
}

impl PerturbationCpuWorkerState {
    /// Recompute the gear from the current stencil / reference (D-GEAR-1).
    pub fn refresh_gear(&mut self, gpu_available: bool) {
        let Some(stencil) = self.stencil.as_ref() else {
            self.gear = crate::gear::Gear::F64;
            return;
        };
        let relative = self
            .seat_orbit_ids
            .iter()
            .copied()
            .find(|&id| id != ZERO_ORBIT_ID)
            .and_then(|id| self.references.get(id))
            .map(|orbit| &orbit.big_c);
        self.gear = stencil.select_gear(relative, gpu_available);
    }
}

impl Worker<f64, CpuPeriodicityDetector> for PerturbationCpuWorker {
    type State = PerturbationCpuWorkerState;

    fn initialize_batch<const N: usize>(
        worker_state: &Self::State
        , active_tile: &Tile<()>
        , seats: [Option<(usize, usize)>; N]
    ) -> PointBatch<f64, CpuPeriodicityDetector, N> {
        let Some(stencil) = worker_state.stencil.as_ref() else {
            return PointBatch { points: [const { None }; N] };
        };
        let mut points: [Option<((usize, usize), ActivePoint<f64, CpuPeriodicityDetector>)>; N] =
            [const { None }; N];
        for i in 0..N {
            let Some(local) = seats[i] else { continue };
            let seat = active_tile.screen_seat(local);
            if seat.0 >= stencil.resolution.0 || seat.1 >= stencil.resolution.1 {
                continue;
            }
            let seat_u16 = (
                seat.0.min(u16::MAX as usize) as u16
                , seat.1.min(u16::MAX as usize) as u16
            );
            let seat_linear = seat.1 * worker_state.screen_width + seat.0;
            let orbit_id = ReferenceCollection::seat_orbit_id(
                &worker_state.seat_orbit_ids
                , seat_linear
            );
            let Some(orbit) = worker_state.references.get(orbit_id) else {
                continue;
            };
            let Some(delta_c) = delta_c_for_seat(stencil, orbit, seat_u16) else {
                continue;
            };
            let (delta_z, iteration_count) = series_skip(orbit, delta_c);
            let z_ref = orbit.f64[iteration_count];
            let z_full = (z_ref.0 + delta_z.0, z_ref.1 + delta_z.1);
            let derivative = if iteration_count > 0 {
                derivative_after_series(orbit, delta_c, iteration_count)
            } else {
                (f64::ONE, f64::ZERO)
            };
            let min_magnitude = {
                let m = z_full.0 * z_full.0 + z_full.1 * z_full.1;
                if iteration_count == 0 { f64::MAX } else { m }
            };
            points[i] = Some((
                local
                , ActivePoint {
                    c: delta_c
                    , z: delta_z
                    , derivative
                    , real_squared: delta_z.0 * delta_z.0
                    , imag_squared: delta_z.1 * delta_z.1
                    , real_imag: delta_z.0 * delta_z.1
                    , iteration_count
                    , min_magnitude
                    , min_magnitude_time: if iteration_count == 0 { 0 } else { iteration_count }
                    , periodicity_detector: CpuPeriodicityDetector::init(
                        iteration_count
                        , z_full
                        , derivative
                    )
                    , escaped: false
                    , finished: false
                    , orbit_id
                    , seat_linear
                }
            ));
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
                let c_abs = point.c.0.abs().max(point.c.1.abs());
                let epsilon = 1e-12f64.max(c_abs * 1e-6);
                iterate_perturbation_bout(worker_state, point, epsilon);
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

fn delta_c_for_seat(
    stencil: &PointStencil
    , orbit: &ReferenceOrbit
    , seat_u16: (u16, u16)
) -> Option<(f64, f64)> {
    if let Some(gen) = stencil.get_relative_c_generator::<f64>(&orbit.big_c) {
        return Some(gen.get_c(seat_u16));
    }
    let gen = stencil.get_c_generator::<f64>()?;
    let c = gen.get_c(seat_u16);
    let cref = (
        orbit.big_c.0.clone().to_f64()
        , orbit.big_c.1.clone().to_f64()
    );
    Some((c.0 - cref.0, c.1 - cref.1))
}

fn series_skip(orbit: &ReferenceOrbit, delta_c: (f64, f64)) -> ((f64, f64), u64) {
    let series = &orbit.f64.series;
    if series.len() < 2 {
        return ((0.0, 0.0), 0);
    }
    let dc_mag2 = delta_c.0 * delta_c.0 + delta_c.1 * delta_c.1;
    let absorb = 1e-12f64.max(dc_mag2 * 1e-6);
    let mut best_n = 0u64;
    let mut best_dz = (0.0f64, 0.0f64);
    for n in 1..series.len() {
        let a = series[n];
        let dz = (
            a.0 * delta_c.0 - a.1 * delta_c.1
            , a.0 * delta_c.1 + a.1 * delta_c.0
        );
        let mag2 = dz.0 * dz.0 + dz.1 * dz.1;
        if mag2 > absorb {
            break;
        }
        best_n = n as u64;
        best_dz = dz;
    }
    (best_dz, best_n)
}

fn derivative_after_series(
    orbit: &ReferenceOrbit
    , delta_c: (f64, f64)
    , iteration_count: u64
) -> (f64, f64) {
    let mut d = (0.0f64, 0.0f64);
    let mut dz = (0.0f64, 0.0f64);
    for n in 0..iteration_count {
        let z_ref = orbit.f64[n];
        let z_full = (z_ref.0 + dz.0, z_ref.1 + dz.1);
        let new_d = (
            2.0 * (z_full.0 * d.0 - z_full.1 * d.1)
            , 2.0 * (z_full.0 * d.1 + z_full.1 * d.0)
        );
        d = new_d;
        let dz2 = (
            dz.0 * dz.0 - dz.1 * dz.1
            , 2.0 * dz.0 * dz.1
        );
        dz = (
            2.0 * (z_ref.0 * dz.0 - z_ref.1 * dz.1) + dz2.0 + delta_c.0
            , 2.0 * (z_ref.0 * dz.1 + z_ref.1 * dz.0) + dz2.1 + delta_c.1
        );
    }
    d
}

pub fn iterate_perturbation_bout(
    worker_state: &mut PerturbationCpuWorkerState
    , point: &mut ActivePoint<f64, CpuPeriodicityDetector>
    , epsilon: f64
) {
    let bailout = worker_state.bailout_radius_squared;
    let bout = worker_state.iterations_per_bout;
    // Cap so one Inside seat cannot monopolize headed workshifts forever
    // (periodicity usually finishes earlier; this is a safety valve).
    const MAX_PERTURB_ITERS: u64 = 50_000;
    for _ in 0..bout {
        if point.finished { break; }
        if point.iteration_count >= MAX_PERTURB_ITERS {
            point.finished = true;
            break;
        }
        let Some(orbit) = worker_state.references.get(point.orbit_id) else {
            rebind_to_zero_orbit(worker_state, point);
            continue;
        };
        let n = point.iteration_count;
        let z_ref = orbit.f64[n];
        let mut dz_re = point.z.0;
        let mut dz_im = point.z.1;
        let z_full_re = z_ref.0 + dz_re;
        let z_full_im = z_ref.1 + dz_im;
        let z_ref_mag2 = z_ref.0 * z_ref.0 + z_ref.1 * z_ref.1;
        let z_full_mag2 = z_full_re * z_full_re + z_full_im * z_full_im;
        if point.orbit_id != ZERO_ORBIT_ID
            && z_ref_mag2 > 1e-30
            && z_full_mag2 < GLITCH_THRESHOLD * z_ref_mag2
        {
            rebind_to_zero_orbit(worker_state, point);
            continue;
        }
        let mut d_re = point.derivative.0;
        let mut d_im = point.derivative.1;
        let new_d_re = 2.0 * (z_full_re * d_re - z_full_im * d_im);
        let new_d_im = 2.0 * (z_full_re * d_im + z_full_im * d_re);
        d_re = new_d_re;
        d_im = new_d_im;
        let dz2_re = dz_re * dz_re - dz_im * dz_im;
        let dz2_im = 2.0 * dz_re * dz_im;
        let new_dz_re = 2.0 * (z_ref.0 * dz_re - z_ref.1 * dz_im) + dz2_re + point.c.0;
        let new_dz_im = 2.0 * (z_ref.0 * dz_im + z_ref.1 * dz_re) + dz2_im + point.c.1;
        dz_re = new_dz_re;
        dz_im = new_dz_im;
        point.iteration_count += 1;
        let z_ref_next = orbit.f64[point.iteration_count];
        let z_full_re = z_ref_next.0 + dz_re;
        let z_full_im = z_ref_next.1 + dz_im;
        point.z = (dz_re, dz_im);
        point.derivative = (d_re, d_im);
        point.real_squared = dz_re * dz_re;
        point.imag_squared = dz_im * dz_im;
        point.real_imag = dz_re * dz_im;
        let rad = z_full_re * z_full_re + z_full_im * z_full_im;
        if rad < point.min_magnitude {
            point.min_magnitude = rad;
            point.min_magnitude_time = point.iteration_count;
        }
        if rad > bailout {
            point.z = (z_full_re, z_full_im);
            point.escaped = true;
            point.finished = true;
            break;
        }
        let cref = (
            orbit.big_c.0.clone().to_f64()
            , orbit.big_c.1.clone().to_f64()
        );
        let c_full = (cref.0 + point.c.0, cref.1 + point.c.1);
        if point.periodicity_detector.check_periodicity(
            c_full
            , (z_full_re, z_full_im)
            , (d_re, d_im)
            , point.iteration_count
            , epsilon
        ).is_some() {
            point.z = (z_full_re, z_full_im);
            point.finished = true;
            break;
        }
    }
}

fn rebind_to_zero_orbit(
    worker_state: &mut PerturbationCpuWorkerState
    , point: &mut ActivePoint<f64, CpuPeriodicityDetector>
) {
    let cref = worker_state
        .references
        .get(point.orbit_id)
        .map(|o| {
            (
                o.big_c.0.clone().to_f64()
                , o.big_c.1.clone().to_f64()
            )
        })
        .unwrap_or((0.0, 0.0));
    let c_pixel = (cref.0 + point.c.0, cref.1 + point.c.1);
    ReferenceCollection::bind_seat(
        &mut worker_state.seat_orbit_ids
        , point.seat_linear
        , ZERO_ORBIT_ID
    );
    let z = (f64::ZERO, f64::ZERO);
    let derivative = (f64::ONE, f64::ZERO);
    point.orbit_id = ZERO_ORBIT_ID;
    point.c = c_pixel;
    point.z = z;
    point.derivative = derivative;
    point.real_squared = f64::ZERO;
    point.imag_squared = f64::ZERO;
    point.real_imag = f64::ZERO;
    point.iteration_count = 0;
    point.min_magnitude = f64::MAX;
    point.min_magnitude_time = 0;
    point.periodicity_detector = CpuPeriodicityDetector::init(0, z, derivative);
    point.escaped = false;
    point.finished = false;
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
mod phase4_tests {
    use super::*;
    use crate::assemblies::workgroup_new::workcore::mandelbrot::worker_implementations::naive_cpu_worker::{
        iterate_point_bout
        , point_to_answer as naive_point_to_answer
    };
    use crate::intexp::*;

    fn naive_finish(c: (f64, f64), max_iters: u32) -> Answer {
        let z = (0.0, 0.0);
        let derivative = (1.0, 0.0);
        let mut point = ActivePoint {
            c
            , z
            , derivative
            , real_squared: 0.0
            , imag_squared: 0.0
            , real_imag: 0.0
            , iteration_count: 0
            , min_magnitude: f64::MAX
            , min_magnitude_time: 0
            , periodicity_detector: CpuPeriodicityDetector::init(0, z, derivative)
            , escaped: false
            , finished: false
            , orbit_id: ZERO_ORBIT_ID
            , seat_linear: 0
        };
        let epsilon = 1e-12f64.max(c.0.abs().max(c.1.abs()) * 1e-6);
        let mut left = max_iters;
        while !point.finished && left > 0 {
            let bout = left.min(1000);
            iterate_point_bout(&mut point, 4.0, epsilon, bout);
            left = left.saturating_sub(bout);
        }
        naive_point_to_answer(&point)
    }

    fn perturb_finish_zero(c: (f64, f64), max_iters: u32) -> Answer {
        let z = (0.0, 0.0);
        let derivative = (1.0, 0.0);
        let mut state = PerturbationCpuWorkerState::default();
        state.seat_orbit_ids = vec![ZERO_ORBIT_ID];
        state.iterations_per_bout = 1000;
        let mut point = ActivePoint {
            c
            , z
            , derivative
            , real_squared: 0.0
            , imag_squared: 0.0
            , real_imag: 0.0
            , iteration_count: 0
            , min_magnitude: f64::MAX
            , min_magnitude_time: 0
            , periodicity_detector: CpuPeriodicityDetector::init(0, z, derivative)
            , escaped: false
            , finished: false
            , orbit_id: ZERO_ORBIT_ID
            , seat_linear: 0
        };
        let mut left = max_iters;
        while !point.finished && left > 0 {
            state.iterations_per_bout = left.min(1000);
            let epsilon = 1e-12f64.max(c.0.abs().max(c.1.abs()) * 1e-6);
            iterate_perturbation_bout(&mut state, &mut point, epsilon);
            left = left.saturating_sub(state.iterations_per_bout);
        }
        point_to_answer(&point)
    }

    fn same_membership(a: &Answer, b: &Answer) -> bool {
        match (&a.result, &b.result) {
            (
                MandelbrotResult::Outside { escape_time_r2: ea, .. }
                , MandelbrotResult::Outside { escape_time_r2: eb, .. }
            ) => ea == eb
            , (MandelbrotResult::Inside { period: pa }, MandelbrotResult::Inside { period: pb }) => {
                *pa == 0 && *pb == 0
            }
            _ => false
        }
    }

    #[test]
    fn shallow_naive_matches_perturb_zero_orbit_cardioid_and_exterior() {
        let samples = [
            cardioid_c_from_mu((0.0, 0.0))
            , cardioid_c_from_mu((0.25, 0.0))
            , cardioid_c_from_mu((0.0, 0.25))
            , (-0.75, 0.1)
            , (0.5, 0.5)
            , (2.0, 0.0)
        ];
        for c in samples {
            let naive = naive_finish(c, 50_000);
            let perturb = perturb_finish_zero(c, 50_000);
            assert!(
                same_membership(&naive, &perturb)
                , "mismatch at c={c:?}"
            );
        }
    }

    #[test]
    fn glitch_rebinds_seat_to_zero_orbit() {
        let mut state = PerturbationCpuWorkerState::default();
        let id = state.references.try_add_nucleus_at_f64((-1.0, 0.0));
        assert_ne!(id, ZERO_ORBIT_ID, "period-2 nucleus should add");
        state.seat_orbit_ids = vec![id];
        state.screen_width = 1;
        state.iterations_per_bout = 64;
        let orbit = state.references.get(id).expect("orbit");
        let z_ref = orbit.f64[1.min(orbit.length as u64 - 1)];
        let z = (0.0, 0.0);
        let derivative = (1.0, 0.0);
        let mut point = ActivePoint {
            c: (3.0, 0.0)
            , z: (-z_ref.0, -z_ref.1)
            , derivative
            , real_squared: 0.0
            , imag_squared: 0.0
            , real_imag: 0.0
            , iteration_count: 1
            , min_magnitude: f64::MAX
            , min_magnitude_time: 0
            , periodicity_detector: CpuPeriodicityDetector::init(1, z, derivative)
            , escaped: false
            , finished: false
            , orbit_id: id
            , seat_linear: 0
        };
        let epsilon = 1e-12;
        iterate_perturbation_bout(&mut state, &mut point, epsilon);
        assert_eq!(
            ReferenceCollection::seat_orbit_id(&state.seat_orbit_ids, 0)
            , ZERO_ORBIT_ID
        );
        assert_eq!(point.orbit_id, ZERO_ORBIT_ID);
    }

    // r[verify cz.seamless.perturbation-always-on+1]
    #[test]
    fn glitch_fires_just_under_threshold_ratio() {
        // z_full_mag2 < GLITCH_THRESHOLD * z_ref_mag2 with nonzero reference.
        let mut state = PerturbationCpuWorkerState::default();
        let id = state.references.try_add_nucleus_at_f64((-1.0, 0.0));
        assert_ne!(id, ZERO_ORBIT_ID);
        state.seat_orbit_ids = vec![id];
        state.screen_width = 1;
        state.iterations_per_bout = 1;
        let orbit = state.references.get(id).expect("orbit");
        let z_ref = orbit.f64[1.min(orbit.length as u64 - 1)];
        let z_ref_mag2 = z_ref.0 * z_ref.0 + z_ref.1 * z_ref.1;
        assert!(z_ref_mag2 > 1e-30);
        // Choose dz so z_full = z_ref + dz has mag2 just under the threshold.
        let target_full_mag2 = GLITCH_THRESHOLD * z_ref_mag2 * 0.5;
        let scale = (target_full_mag2 / z_ref_mag2).sqrt();
        let z_full = (z_ref.0 * scale, z_ref.1 * scale);
        let dz = (z_full.0 - z_ref.0, z_full.1 - z_ref.1);
        let mut point = ActivePoint {
            c: (3.0, 0.0)
            , z: dz
            , derivative: (1.0, 0.0)
            , real_squared: 0.0
            , imag_squared: 0.0
            , real_imag: 0.0
            , iteration_count: 1
            , min_magnitude: f64::MAX
            , min_magnitude_time: 0
            , periodicity_detector: CpuPeriodicityDetector::init(1, (0.0, 0.0), (1.0, 0.0))
            , escaped: false
            , finished: false
            , orbit_id: id
            , seat_linear: 0
        };
        iterate_perturbation_bout(&mut state, &mut point, 1e-12);
        assert_eq!(point.orbit_id, ZERO_ORBIT_ID, "just-under threshold must glitch");
    }

    // r[verify cz.seamless.perturbation-always-on+1]
    #[test]
    fn glitch_does_not_fire_just_over_threshold_ratio() {
        let mut state = PerturbationCpuWorkerState::default();
        let id = state.references.try_add_nucleus_at_f64((-1.0, 0.0));
        assert_ne!(id, ZERO_ORBIT_ID);
        state.seat_orbit_ids = vec![id];
        state.screen_width = 1;
        state.iterations_per_bout = 1;
        let orbit = state.references.get(id).expect("orbit");
        let z_ref = orbit.f64[1.min(orbit.length as u64 - 1)];
        let z_ref_mag2 = z_ref.0 * z_ref.0 + z_ref.1 * z_ref.1;
        assert!(z_ref_mag2 > 1e-30);
        // z_full ≈ z_ref (ratio ~1 >> GLITCH_THRESHOLD) — healthy perturbation step.
        let dz = (0.0, 0.0);
        let mut point = ActivePoint {
            c: (-1.0, 0.0)
            , z: dz
            , derivative: (1.0, 0.0)
            , real_squared: 0.0
            , imag_squared: 0.0
            , real_imag: 0.0
            , iteration_count: 1
            , min_magnitude: f64::MAX
            , min_magnitude_time: 0
            , periodicity_detector: CpuPeriodicityDetector::init(1, (0.0, 0.0), (1.0, 0.0))
            , escaped: false
            , finished: false
            , orbit_id: id
            , seat_linear: 0
        };
        iterate_perturbation_bout(&mut state, &mut point, 1e-12);
        assert_eq!(
            point.orbit_id
            , id
            , "healthy ratio must keep the nonzero reference orbit"
        );
    }

    // r[verify cz.seamless.perturbation-always-on+1]
    #[test]
    fn after_glitch_rebind_membership_matches_zero_orbit() {
        // Point.c is delta from the reference nucleus until rebind converts it
        // to absolute pixel c (see rebind_to_zero_orbit).
        let abs_c = (0.5, 0.5);
        let ref_c = (-1.0, 0.0);
        let delta_c = (abs_c.0 - ref_c.0, abs_c.1 - ref_c.1);
        let naive = naive_finish(abs_c, 50_000);
        let mut state = PerturbationCpuWorkerState::default();
        let id = state.references.try_add_nucleus_at_f64(ref_c);
        assert_ne!(id, ZERO_ORBIT_ID);
        state.seat_orbit_ids = vec![id];
        state.screen_width = 1;
        state.iterations_per_bout = 64;
        let orbit = state.references.get(id).expect("orbit");
        let z_ref = orbit.f64[1.min(orbit.length as u64 - 1)];
        let mut point = ActivePoint {
            c: delta_c
            , z: (-z_ref.0, -z_ref.1)
            , derivative: (1.0, 0.0)
            , real_squared: 0.0
            , imag_squared: 0.0
            , real_imag: 0.0
            , iteration_count: 1
            , min_magnitude: f64::MAX
            , min_magnitude_time: 0
            , periodicity_detector: CpuPeriodicityDetector::init(1, (0.0, 0.0), (1.0, 0.0))
            , escaped: false
            , finished: false
            , orbit_id: id
            , seat_linear: 0
        };
        let mut left = 50_000u32;
        while !point.finished && left > 0 {
            state.iterations_per_bout = left.min(1000);
            iterate_perturbation_bout(&mut state, &mut point, 1e-12);
            left = left.saturating_sub(state.iterations_per_bout);
        }
        assert_eq!(point.orbit_id, ZERO_ORBIT_ID);
        assert!(
            (point.c.0 - abs_c.0).abs() < 1e-9 && (point.c.1 - abs_c.1).abs() < 1e-9
            , "rebind must promote delta-c to absolute pixel c"
        );
        let answer = point_to_answer(&point);
        assert!(
            same_membership(&answer, &naive)
            , "post-glitch answer must match naive at c={abs_c:?}"
        );
    }

    #[test]
    fn inside_answer_period_unknown_zero_allowed() {
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
                assert_eq!(period, 0);
            }
            other => panic!("expected Inside, got {other:?}"),
        }
    }

    #[test]
    fn seahorse_valley_fixture_matches_naive_on_zero_orbit() {
        let c = (
            -0.7436438870371587
            , 0.13182590420531197
        );
        let naive = naive_finish(c, 100_000);
        let perturb = perturb_finish_zero(c, 100_000);
        assert!(
            same_membership(&naive, &perturb)
            , "seahorse fixture mismatch"
        );
    }

    #[test]
    fn series_populated_for_nonzero_nucleus() {
        let mut collection = ReferenceCollection::new();
        let id = collection.try_add_nucleus_at_f64((-1.0, 0.0));
        assert_ne!(id, ZERO_ORBIT_ID);
        let orbit = collection.get(id).expect("orbit");
        assert!(orbit.f64.series.len() >= 2);
        assert_eq!(orbit.f64.series[1], (1.0, 0.0));
    }

    #[test]
    fn worker_batch_zero_orbit_matches_via_stencil() {
        let res = (8usize, 8usize);
        let stencil = PointStencil {
            homothety: (
                IntExp::from(-2)
                , IntExp::from(2)
                , 0
            )
            , resolution: res
            , serial_number: 0
            , focus: None
            , hover: None,
            mag_velocity: 0.0
        }.correct_precision();
        let mut state = PerturbationCpuWorkerState {
            bailout_radius_squared: 4.0
            , iterations_per_bout: 2000
            , stencil: Some(stencil.clone())
            , references: ReferenceCollection::new()
            , seat_orbit_ids: vec![ZERO_ORBIT_ID; res.0 * res.1]
            , screen_width: res.0
            , gear: crate::gear::Gear::F64
        };
        let tile = Tile::new((0, 0), 0);
        let seats: [Option<(usize, usize)>; 4] = [
            Some((0, 0))
            , Some((7, 0))
            , Some((0, 7))
            , Some((3, 3))
        ];
        let mut batch = PerturbationCpuWorker::initialize_batch(&state, &tile, seats);
        let mut guard = 0;
        while PerturbationCpuWorker::workshift_on_batch(&mut state, &mut batch) {
            guard += 1;
            assert!(guard < 200, "bout runaway");
        }
        let peeked = PerturbationCpuWorker::peek_batch(&batch, &tile);
        let gen = stencil.get_c_generator::<f64>().expect("c gen");
        for i in 0..4 {
            let Some(local) = seats[i] else { continue };
            let Some((_, calib)) = peeked[i] else {
                panic!("seat {local:?} unfinished");
            };
            let seat = tile.screen_seat(local);
            let c = gen.get_c((seat.0 as u16, seat.1 as u16));
            let naive = naive_finish(c, 50_000);
            let perturb = match calib.result {
                CalibratedMandelbrotResult::Outside { escape_time_r2, escape_z } => {
                    Answer {
                        result: MandelbrotResult::Outside {
                            escape_time_r2: escape_time_r2.lower_bound
                            , escape_z: (
                                escape_z.0.lower_bound
                                , escape_z.1.lower_bound
                            )
                        }
                        , min_magnitude_time: calib.min_magnitude_time.lower_bound
                        , min_magnitude: calib.min_magnitude.lower_bound
                    }
                }
                , CalibratedMandelbrotResult::Inside { period } => {
                    Answer {
                        result: MandelbrotResult::Inside {
                            period: period.lower_bound
                        }
                        , min_magnitude_time: calib.min_magnitude_time.lower_bound
                        , min_magnitude: calib.min_magnitude.lower_bound
                    }
                }
                , CalibratedMandelbrotResult::Agnostic { period, .. } => {
                    Answer {
                        result: MandelbrotResult::Inside {
                            period: period.lower_bound
                        }
                        , min_magnitude_time: calib.min_magnitude_time.lower_bound
                        , min_magnitude: calib.min_magnitude.lower_bound
                    }
                }
            };
            assert!(
                same_membership(&naive, &perturb)
                , "stencil seat {local:?} mismatch"
            );
        }
    }
}
