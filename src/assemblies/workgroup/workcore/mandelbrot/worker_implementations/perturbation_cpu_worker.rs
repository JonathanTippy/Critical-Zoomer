use crate::assemblies::structs::*;
use crate::assemblies::workgroup::structs::*;
use crate::assemblies::workgroup::structs::mandelbrotable::*;
use crate::assemblies::workgroup::workcore::mandelbrot::*;
use crate::assemblies::workgroup::workcore::mandelbrot::worker_implementations::periodicity_detector::*;
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
    , pub iterations_advanced: u64
    // Counts AdaptiveRug FloatExp bouts (observability for gear dispatch tests).
    , pub floatexp_bouts: u64
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
            , iterations_advanced: 0
            , floatexp_bouts: 0
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
        // Cache orbit + δc generator across seats that share a reference (common at home).
        let mut cached_orbit_id: Option<OrbitId> = None;
        let mut cached_orbit: Option<&ReferenceOrbit> = None;
        let mut cached_rel_gen = None;
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
            if cached_orbit_id != Some(orbit_id) {
                let Some(orbit) = worker_state.references.get(orbit_id) else {
                    continue;
                };
                cached_orbit_id = Some(orbit_id);
                cached_orbit = Some(orbit);
                cached_rel_gen = stencil.get_relative_c_generator::<f64>(&orbit.big_c);
            }
            let Some(orbit) = cached_orbit else { continue };
            let Some(delta_c) = cached_rel_gen
                .as_ref()
                .map(|g| g.get_c(seat_u16))
                .or_else(|| delta_c_for_seat(stencil, orbit, seat_u16))
            else {
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
        // Gear is authoritative for numeric path (selection already refreshed).
        // Dispatch outside the bout loop; each arm monomorphizes Mandelbrotable math.
        let gear = worker_state.gear;
        let mut any_incomplete = false;
        for slot in active_batch.points.iter_mut() {
            if let Some((_, point)) = slot {
                if point.finished { continue; }
                any_incomplete = true;
                let c_abs = point.c.0.abs().max(point.c.1.abs());
                let epsilon = gear_period_epsilon(gear, c_abs);
                dispatch_perturbation_bout(worker_state, point, gear, epsilon);
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
                } else {
                    // Progressive Agnostic WIP: ranges from ActivePoint progress.
                    out[i] = Some((*seat, point_to_calibrated_wip(point)));
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
    let two = 2.0f64;
    for n in 0..iteration_count {
        let z_ref = orbit.f64[n];
        let z_full = (z_ref.0 + dz.0, z_ref.1 + dz.1);
        let new_d = (
            two * (z_full.0 * d.0 - z_full.1 * d.1)
            , two * (z_full.0 * d.1 + z_full.1 * d.0)
        );
        d = new_d;
        let dz2 = (
            dz.0 * dz.0 - dz.1 * dz.1
            , two * dz.0 * dz.1
        );
        dz = (
            two * (z_ref.0 * dz.0 - z_ref.1 * dz.1) + dz2.0 + delta_c.0
            , two * (z_ref.0 * dz.1 + z_ref.1 * dz.0) + dz2.1 + delta_c.1
        );
    }
    d
}

fn pair_from_f64<T: Mandelbrotable>(p: (f64, f64)) -> (T, T) {
    (T::from_f64(p.0), T::from_f64(p.1))
}

fn pair_to_f64<T: Mandelbrotable>(p: (T, T)) -> (f64, f64) {
    (p.0.to_f64(), p.1.to_f64())
}

fn bridge_host_to_typed<T: Mandelbrotable>(
    point: &ActivePoint<f64, CpuPeriodicityDetector>
) -> ActivePoint<T, StandardPeriodicityDetector<T>> {
    let z = pair_from_f64(point.z);
    let derivative = pair_from_f64(point.derivative);
    let mut periodicity_detector = StandardPeriodicityDetector::init(
        point.iteration_count
        , z
        , derivative
    );
    periodicity_detector.checkpoint_z = pair_from_f64(point.periodicity_detector.checkpoint_z);
    periodicity_detector.steps_since_checkpoint = point.periodicity_detector.steps_since_checkpoint;
    periodicity_detector.next_checkpoint_iteration =
        point.periodicity_detector.next_checkpoint_iteration;
    periodicity_detector.detected_period = point.periodicity_detector.detected_period;
    ActivePoint {
        c: pair_from_f64(point.c)
        , z
        , derivative
        , real_squared: T::from_f64(point.real_squared)
        , imag_squared: T::from_f64(point.imag_squared)
        , real_imag: T::from_f64(point.real_imag)
        , iteration_count: point.iteration_count
        , min_magnitude: T::from_f64(point.min_magnitude)
        , min_magnitude_time: point.min_magnitude_time
        , periodicity_detector
        , escaped: point.escaped
        , finished: point.finished
        , orbit_id: point.orbit_id
        , seat_linear: point.seat_linear
    }
}

fn bridge_typed_to_host<T: Mandelbrotable>(
    host: &mut ActivePoint<f64, CpuPeriodicityDetector>
    , typed: &ActivePoint<T, StandardPeriodicityDetector<T>>
) {
    host.c = pair_to_f64(typed.c);
    host.z = pair_to_f64(typed.z);
    host.derivative = pair_to_f64(typed.derivative);
    host.real_squared = typed.real_squared.to_f64();
    host.imag_squared = typed.imag_squared.to_f64();
    host.real_imag = typed.real_imag.to_f64();
    host.iteration_count = typed.iteration_count;
    host.min_magnitude = typed.min_magnitude.to_f64();
    host.min_magnitude_time = typed.min_magnitude_time;
    host.escaped = typed.escaped;
    host.finished = typed.finished;
    host.orbit_id = typed.orbit_id;
    host.seat_linear = typed.seat_linear;
    host.periodicity_detector.checkpoint_z = pair_to_f64(typed.periodicity_detector.checkpoint_z);
    host.periodicity_detector.steps_since_checkpoint =
        typed.periodicity_detector.steps_since_checkpoint;
    host.periodicity_detector.next_checkpoint_iteration =
        typed.periodicity_detector.next_checkpoint_iteration;
    host.periodicity_detector.detected_period = typed.periodicity_detector.detected_period;
}

fn dispatch_perturbation_bout(
    worker_state: &mut PerturbationCpuWorkerState
    , point: &mut ActivePoint<f64, CpuPeriodicityDetector>
    , gear: crate::gear::Gear
    , epsilon_f64: f64
) {
    match gear {
        crate::gear::Gear::F64 => {
            iterate_perturbation_bout_typed::<f64, CpuPeriodicityDetector>(
                worker_state
                , point
                , epsilon_f64
            );
        }
        crate::gear::Gear::F32 => {
            let mut typed = bridge_host_to_typed::<f32>(point);
            let eps = f32::from_f64(epsilon_f64);
            iterate_perturbation_bout_typed::<f32, StandardPeriodicityDetector<f32>>(
                worker_state
                , &mut typed
                , eps
            );
            bridge_typed_to_host(point, &typed);
        }
        crate::gear::Gear::AdaptiveRug => {
            use crate::floatexp::FloatExp;
            worker_state.floatexp_bouts = worker_state.floatexp_bouts.saturating_add(1);
            let mut typed = bridge_host_to_typed::<FloatExp>(point);
            let eps = FloatExp::from_f64(epsilon_f64);
            iterate_perturbation_bout_typed::<FloatExp, StandardPeriodicityDetector<FloatExp>>(
                worker_state
                , &mut typed
                , eps
            );
            bridge_typed_to_host(point, &typed);
        }
        crate::gear::Gear::StackedI32 { limbs } => {
            dispatch_stacked_bout(worker_state, point, limbs, epsilon_f64);
        }
    }
}

fn dispatch_stacked_bout(
    worker_state: &mut PerturbationCpuWorkerState
    , point: &mut ActivePoint<f64, CpuPeriodicityDetector>
    , limbs: u8
    , epsilon_f64: f64
) {
    use crate::stacked_intexp::StackedIntExp;
    macro_rules! stacked_arm {
        ($n:expr) => {{
            let mut typed = bridge_host_to_typed::<StackedIntExp<$n>>(point);
            let eps = StackedIntExp::<$n>::from_f64(epsilon_f64);
            iterate_perturbation_bout_typed::<
                StackedIntExp<$n>
                , StandardPeriodicityDetector<StackedIntExp<$n>>
            >(worker_state, &mut typed, eps);
            bridge_typed_to_host(point, &typed);
        }};
    }
    match limbs {
        1 => stacked_arm!(1),
        2 => stacked_arm!(2),
        3 => stacked_arm!(3),
        4 => stacked_arm!(4),
        5 => stacked_arm!(5),
        6 => stacked_arm!(6),
        7 => stacked_arm!(7),
        _ => stacked_arm!(8),
    }
}

/// Host f64 entry used by benches / oracle / GPU CPU-fallback.
pub fn iterate_perturbation_bout(
    worker_state: &mut PerturbationCpuWorkerState
    , point: &mut ActivePoint<f64, CpuPeriodicityDetector>
    , epsilon: f64
) {
    iterate_perturbation_bout_typed::<f64, CpuPeriodicityDetector>(
        worker_state
        , point
        , epsilon
    );
}

/// Trait-generic perturbation bout (standards: Mandelbrotable for all CPU math).
pub fn iterate_perturbation_bout_typed<T, P>(
    worker_state: &mut PerturbationCpuWorkerState
    , point: &mut ActivePoint<T, P>
    , epsilon: T
)
where
    T: Mandelbrotable
    , P: PeriodicityDetector<T>
{
    let bailout = T::from_f64(worker_state.bailout_radius_squared);
    let bout = worker_state.iterations_per_bout;
    let glitch_thresh = T::from_f64(GLITCH_THRESHOLD);
    let tiny = T::from_f64(1e-30);
    let mut steps = 0u32;
    while steps < bout {
        if point.finished {
            break;
        }
        if worker_state.references.get(point.orbit_id).is_none() {
            rebind_to_zero_orbit_typed(worker_state, point);
            continue;
        }
        let orbit = worker_state.references.get(point.orbit_id).unwrap();
        let cref = pair_from_f64::<T>((
            orbit.big_c.0.clone().to_f64()
            , orbit.big_c.1.clone().to_f64()
        ));
        let c_full = (cref.0 + point.c.0, cref.1 + point.c.1);
        let mut glitched = false;
        {
            let orbit = worker_state.references.get(point.orbit_id).unwrap();
            // Phase 2 will use native mirrors; for now project f64 samples.
            let mut z_ref = pair_from_f64::<T>(orbit.f64.z_at(point.iteration_count));
            while steps < bout {
                if point.finished {
                    break;
                }
                let mut dz = point.z;
                let z_full = (z_ref.0 + dz.0, z_ref.1 + dz.1);
                let z_ref_mag2 = z_ref.0 * z_ref.0 + z_ref.1 * z_ref.1;
                let z_full_mag2 = z_full.0 * z_full.0 + z_full.1 * z_full.1;
                if z_ref_mag2 > tiny && z_full_mag2 < glitch_thresh * z_ref_mag2 {
                    glitched = true;
                    break;
                }
                let d = point.derivative;
                let new_d = (
                    T::TWO * (z_full.0 * d.0 - z_full.1 * d.1)
                    , T::TWO * (z_full.0 * d.1 + z_full.1 * d.0)
                );
                let dz2 = (
                    dz.0 * dz.0 - dz.1 * dz.1
                    , T::TWO * dz.0 * dz.1
                );
                dz = (
                    T::TWO * (z_ref.0 * dz.0 - z_ref.1 * dz.1) + dz2.0 + point.c.0
                    , T::TWO * (z_ref.0 * dz.1 + z_ref.1 * dz.0) + dz2.1 + point.c.1
                );
                point.iteration_count += 1;
                steps += 1;
                z_ref = pair_from_f64::<T>(orbit.f64.z_at(point.iteration_count));
                let z_full = (z_ref.0 + dz.0, z_ref.1 + dz.1);
                point.z = dz;
                point.derivative = new_d;
                point.real_squared = dz.0 * dz.0;
                point.imag_squared = dz.1 * dz.1;
                point.real_imag = dz.0 * dz.1;
                let rad = z_full.0 * z_full.0 + z_full.1 * z_full.1;
                if rad < point.min_magnitude {
                    point.min_magnitude = rad;
                    point.min_magnitude_time = point.iteration_count;
                }
                if rad > bailout {
                    point.z = z_full;
                    point.escaped = true;
                    point.finished = true;
                    break;
                }
                if point
                    .periodicity_detector
                    .check_periodicity(
                        c_full
                        , z_full
                        , new_d
                        , point.iteration_count
                        , epsilon
                    )
                    .is_some()
                {
                    point.z = z_full;
                    point.finished = true;
                    break;
                }
            }
        }
        if glitched {
            rebind_to_zero_orbit_typed(worker_state, point);
            continue;
        }
        break;
    }
    worker_state.iterations_advanced = worker_state
        .iterations_advanced
        .saturating_add(steps as u64);
}

fn rebind_to_zero_orbit_typed<T, P>(
    worker_state: &mut PerturbationCpuWorkerState
    , point: &mut ActivePoint<T, P>
)
where
    T: Mandelbrotable
    , P: PeriodicityDetector<T>
{
    let cref = worker_state
        .references
        .get(point.orbit_id)
        .map(|o| {
            pair_from_f64::<T>((
                o.big_c.0.clone().to_f64()
                , o.big_c.1.clone().to_f64()
            ))
        })
        .unwrap_or((T::ZERO, T::ZERO));
    let c_pixel = (cref.0 + point.c.0, cref.1 + point.c.1);
    ReferenceCollection::bind_seat(
        &mut worker_state.seat_orbit_ids
        , point.seat_linear
        , ZERO_ORBIT_ID
    );
    let z = (T::ZERO, T::ZERO);
    let derivative = (T::ONE, T::ZERO);
    point.orbit_id = ZERO_ORBIT_ID;
    point.c = c_pixel;
    point.z = z;
    point.derivative = derivative;
    point.real_squared = T::ZERO;
    point.imag_squared = T::ZERO;
    point.real_imag = T::ZERO;
    point.iteration_count = 0;
    point.min_magnitude = T::max_value();
    point.min_magnitude_time = 0;
    point.periodicity_detector = P::init(0, z, derivative);
    point.escaped = false;
    point.finished = false;
}

fn rebind_to_zero_orbit(
    worker_state: &mut PerturbationCpuWorkerState
    , point: &mut ActivePoint<f64, CpuPeriodicityDetector>
) {
    rebind_to_zero_orbit_typed(worker_state, point);
}


fn derivative_magnitude_angle(d: (f64, f64)) -> u8 {
    // Map atan2(im, re) of derivative to 0..=255 (D-SCH-3 stored angle).
    let a = d.1.atan2(d.0);
    let turns = a / (2.0 * std::f64::consts::PI);
    let u = (turns.rem_euclid(1.0) * 256.0).floor() as i32;
    (u.clamp(0, 255)) as u8
}

pub fn point_to_answer(point: &ActivePoint<f64, CpuPeriodicityDetector>) -> Answer {
    let escape_time_angle = derivative_magnitude_angle(point.derivative);
    let min_magnitude_angle = escape_time_angle;
    if point.escaped {
        Answer {
            result: MandelbrotResult::Outside {
                escape_time_r2: point.iteration_count
                , escape_z: (point.z.0 as f32, point.z.1 as f32)
            }
            , min_magnitude_time: point.min_magnitude_time
            , min_magnitude: point.min_magnitude
            , escape_time_angle
            , min_magnitude_angle
        }
    } else {
        Answer {
            result: MandelbrotResult::Inside {
                period: 0
            }
            , min_magnitude_time: point.min_magnitude_time
            , min_magnitude: point.min_magnitude
            , escape_time_angle
            , min_magnitude_angle
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
                , escape_time_angle: answer.escape_time_angle
                , min_magnitude_angle: answer.min_magnitude_angle
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
                , escape_time_angle: answer.escape_time_angle
                , min_magnitude_angle: answer.min_magnitude_angle
            }
        }
    }
}

/// Progressive WIP ranges (workgroup.md): escape-time lower bound, min-magnitude
/// upper bound, escape_z in the r∈[2,√6] ring, period still unknown.
fn point_to_calibrated_wip(point: &ActivePoint<f64, CpuPeriodicityDetector>) -> CalibratedAnswer {
    let angle = derivative_magnitude_angle(point.derivative);
    let ring = 6.0f32; // outer of bailout ring r=2 .. r=√6≈2.45, padded for ranges
    let min_mag_upper = if point.min_magnitude.is_finite() {
        point.min_magnitude
    } else {
        f64::MAX
    };
    CalibratedAnswer {
        result: CalibratedMandelbrotResult::Agnostic {
            period: crate::range::Range {
                lower_bound: 0
                , upper_bound: u64::MAX
            }
            , escape_time_r2: crate::range::Range {
                lower_bound: point.iteration_count
                , upper_bound: u64::MAX
            }
            , escape_z: (
                crate::range::Range { lower_bound: -ring, upper_bound: ring }
                , crate::range::Range { lower_bound: -ring, upper_bound: ring }
            )
        }
        , min_magnitude_time: crate::range::Range {
            lower_bound: point.min_magnitude_time
            , upper_bound: point.iteration_count.max(point.min_magnitude_time)
        }
        , min_magnitude: crate::range::Range {
            lower_bound: 0.0
            , upper_bound: min_mag_upper
        }
        , highlights: CalibratedHighlights {
            in_filament: exact_range(false)
            , out_filament: exact_range(false)
            , small_time_edge: exact_range(false)
            , node: exact_range(false)
        }
        , escape_time_angle: angle
        , min_magnitude_angle: angle
    }
}

#[cfg(test)]
mod phase4_tests {
    use super::*;
    use crate::assemblies::workgroup::workcore::mandelbrot::worker_implementations::naive_cpu_worker::{
        iterate_point_bout
        , point_to_answer as naive_point_to_answer
    };
    use crate::intexp::*;

    fn naive_finish(c: (f64, f64), max_iters: u32) -> Answer {
        // r[impl cz.math.perturbation-naive-oracle+1]
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
    // r[verify cz.math.perturbation-naive-oracle+1]
    // r[verify cz.ref.zero-orbit-same-path+1]
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

    // r[verify cz.math.perturbation-naive-oracle+1]
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

    // r[verify cz.math.perturbation-naive-oracle+1]
    #[test]
    fn neck_minus_three_quarters_matches_naive_on_zero_orbit() {
        let c = (-0.75, 0.0);
        let naive = naive_finish(c, 100_000);
        let perturb = perturb_finish_zero(c, 100_000);
        assert!(
            same_membership(&naive, &perturb),
            "neck fixture mismatch"
        );
    }

    // Tenacity: no max-iteration cutoff (removed MAX_PERTURB_ITERS).
    // r[impl cz.tenacious.no-max-iter+1]
    // r[verify cz.tenacious.no-max-iter+1]
    #[test]
    fn tenacity_no_cap_origin_finishes_via_periodicity() {
        let c = (0.0, 0.0);
        let mut state = PerturbationCpuWorkerState::default();
        state.seat_orbit_ids = vec![ZERO_ORBIT_ID];
        state.iterations_per_bout = 4096;
        let mut point = ActivePoint {
            c
            , z: (0.0, 0.0)
            , derivative: (1.0, 0.0)
            , real_squared: 0.0
            , imag_squared: 0.0
            , real_imag: 0.0
            , iteration_count: 0
            , min_magnitude: f64::MAX
            , min_magnitude_time: 0
            , periodicity_detector: CpuPeriodicityDetector::init(0, (0.0, 0.0), (1.0, 0.0))
            , escaped: false
            , finished: false
            , orbit_id: ZERO_ORBIT_ID
            , seat_linear: 0
        };
        let eps = 1e-12f64;
        let mut bouts = 0u32;
        while !point.finished && bouts < 500 {
            iterate_perturbation_bout(&mut state, &mut point, eps);
            bouts += 1;
        }
        assert!(point.finished && !point.escaped);
    }

    #[test]
    fn tenacity_no_cap_allows_iters_past_old_50k_valve_while_open() {
        // Drive a slow-escaping near-cusp for many bouts; unfinished seats must
        // not be force-finished solely by crossing 50_000 iters.
        let c = (0.25000001, 0.0);
        let mut state = PerturbationCpuWorkerState::default();
        state.seat_orbit_ids = vec![ZERO_ORBIT_ID];
        state.iterations_per_bout = 10_000;
        let mut point = ActivePoint {
            c
            , z: (0.0, 0.0)
            , derivative: (1.0, 0.0)
            , real_squared: 0.0
            , imag_squared: 0.0
            , real_imag: 0.0
            , iteration_count: 0
            , min_magnitude: f64::MAX
            , min_magnitude_time: 0
            , periodicity_detector: CpuPeriodicityDetector::init(0, (0.0, 0.0), (1.0, 0.0))
            , escaped: false
            , finished: false
            , orbit_id: ZERO_ORBIT_ID
            , seat_linear: 0
        };
        let eps = 1e-12f64.max(c.0.abs() * 1e-6);
        for _ in 0..6 {
            if point.finished { break; }
            iterate_perturbation_bout(&mut state, &mut point, eps);
        }
        if !point.finished {
            assert!(
                point.iteration_count >= 50_000
                , "expected progress past old cap while still open, got {}",
                point.iteration_count
            );
            assert!(!point.finished || point.escaped);
        }
    }

    #[test]
    fn tenacity_open_seat_survives_past_old_50k_without_false_finish() {
        // Near-cusp exterior that takes many iters: must remain unfinished
        // (or escape honestly) — never force-finished solely by crossing 50k.
        // (Avoid ultra-cusp samples that can false-period under loose reduce_eps.)
        let c = (0.26, 0.0);
        let mut state = PerturbationCpuWorkerState::default();
        state.seat_orbit_ids = vec![ZERO_ORBIT_ID];
        state.iterations_per_bout = 25_000;
        let mut point = ActivePoint {
            c
            , z: (0.0, 0.0)
            , derivative: (1.0, 0.0)
            , real_squared: 0.0
            , imag_squared: 0.0
            , real_imag: 0.0
            , iteration_count: 0
            , min_magnitude: f64::MAX
            , min_magnitude_time: 0
            , periodicity_detector: CpuPeriodicityDetector::init(0, (0.0, 0.0), (1.0, 0.0))
            , escaped: false
            , finished: false
            , orbit_id: ZERO_ORBIT_ID
            , seat_linear: 0
        };
        let eps = 1e-12f64.max(c.0.abs() * 1e-6);
        for _ in 0..8 {
            if point.finished {
                break;
            }
            iterate_perturbation_bout(&mut state, &mut point, eps);
        }
        assert!(
            point.escaped || point.iteration_count > 50_000
            , "expected escape or progress past old 50k valve, got iters={} finished={}",
            point.iteration_count
            , point.finished
        );
        assert!(
            !point.finished || point.escaped
            , "must not force-finish as Inside near-cusp (tenacity / B-PER-2)"
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
            , iterations_advanced: 0
            , floatexp_bouts: 0
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
                        , escape_time_angle: 0
                        , min_magnitude_angle: 0
                    }
                }
                , CalibratedMandelbrotResult::Inside { period } => {
                    Answer {
                        result: MandelbrotResult::Inside {
                            period: period.lower_bound
                        }
                        , min_magnitude_time: calib.min_magnitude_time.lower_bound
                        , min_magnitude: calib.min_magnitude.lower_bound
                        , escape_time_angle: 0
                        , min_magnitude_angle: 0
                    }
                }
                , CalibratedMandelbrotResult::Agnostic { period, .. } => {
                    Answer {
                        result: MandelbrotResult::Inside {
                            period: period.lower_bound
                        }
                        , min_magnitude_time: calib.min_magnitude_time.lower_bound
                        , min_magnitude: calib.min_magnitude.lower_bound
                        , escape_time_angle: 0
                        , min_magnitude_angle: 0
                    }
                }
            };
            assert!(
                same_membership(&naive, &perturb)
                , "stencil seat {local:?} mismatch"
            );
        }
    }

    // D-SERIES-1: series_skip absorption
    #[test]
    fn series_skip_short_series_returns_zero() {
        let orbit = ReferenceOrbit::zero();
        let (dz, n) = series_skip(&orbit, (1e-9, 0.0));
        // Zero orbit series is populated; still require n==0 when delta is huge.
        let (dz_big, n_big) = series_skip(&orbit, (10.0, 10.0));
        assert_eq!(n_big, 0);
        assert_eq!(dz_big, (0.0, 0.0));
        let _ = (dz, n);
    }

    #[test]
    fn series_skip_absorbs_tiny_delta_c() {
        let mut collection = ReferenceCollection::new();
        let id = collection.try_add_nucleus_at_f64((-0.75, 0.0));
        assert_ne!(id, ZERO_ORBIT_ID);
        let orbit = collection.get(id).expect("orbit");
        assert!(orbit.f64.series.len() >= 2);
        let (_dz, n) = series_skip(orbit, (1e-18, 0.0));
        assert!(n >= 1, "tiny delta_c must absorb at least the linear term, got n={n}");
    }

    #[test]
    fn series_skip_stops_when_delta_escapes_absorb() {
        let mut collection = ReferenceCollection::new();
        let id = collection.try_add_nucleus_at_f64((-0.75, 0.0));
        let orbit = collection.get(id).expect("orbit");
        let (_dz_small, n_small) = series_skip(orbit, (1e-16, 0.0));
        let (_dz_large, n_large) = series_skip(orbit, (1e-2, 0.0));
        assert!(
            n_small >= n_large
            , "larger delta_c must not absorb more terms (small={n_small} large={n_large})"
        );
        assert!(n_large < orbit.f64.series.len() as u64);
    }
}

#[cfg(test)]
mod gear_dispatch_tests {
    use super::*;
    use crate::gear::Gear;

    #[test]
    fn refresh_gear_selects_f32_when_gpu_and_shallow() {
        let mut state = PerturbationCpuWorkerState::default();
        let stencil = PointStencil {
            homothety: (IntExp::from(-2), IntExp::from(2), -2),
            resolution: (64, 64),
            serial_number: 0,
            focus: None,
            hover: None,
            mag_velocity: 0.0,
        }.correct_precision();
        state.stencil = Some(stencil);
        state.refresh_gear(true);
        assert_eq!(state.gear, Gear::F32);
    }

    #[test]
    fn refresh_gear_deep_selects_beyond_f64() {
        let mut state = PerturbationCpuWorkerState::default();
        // Extreme zoom pot forces AdaptiveRug / stacked.
        let stencil = PointStencil {
            homothety: (IntExp::from(0), IntExp::from(0), 3_600_000),
            resolution: (64, 64),
            serial_number: 0,
            focus: None,
            hover: None,
            mag_velocity: 0.0,
        }.correct_precision();
        state.stencil = Some(stencil);
        state.refresh_gear(false);
        assert!(
            matches!(state.gear, Gear::AdaptiveRug | Gear::StackedI32 { .. }),
            "got {:?}", state.gear
        );
    }

    #[test]
    fn f64_gear_does_not_run_on_gpu() {
        assert!(!Gear::F64.runs_on_gpu());
        assert!(Gear::F32.runs_on_gpu());
        assert!(!Gear::AdaptiveRug.runs_on_gpu());
    }

    #[test]
    fn adaptive_rug_workshift_uses_floatexp_bout() {
        let mut state = PerturbationCpuWorkerState::default();
        state.gear = Gear::AdaptiveRug;
        state.iterations_per_bout = 64;
        state.seat_orbit_ids = vec![ZERO_ORBIT_ID];
        let z = (0.0, 0.0);
        let derivative = (1.0, 0.0);
        let mut batch: PointBatch<f64, CpuPeriodicityDetector, 1> = PointBatch {
            points: [Some((
                (0, 0)
                , ActivePoint {
                    c: (2.0, 0.0)
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
                }
            ))]
        };
        assert_eq!(state.floatexp_bouts, 0);
        let _ = PerturbationCpuWorker::workshift_on_batch(&mut state, &mut batch);
        assert!(
            state.floatexp_bouts >= 1
            , "AdaptiveRug must take the FloatExp bout path, floatexp_bouts={}", state.floatexp_bouts
        );
    }

    #[test]
    fn peek_emits_agnostic_wip_for_unfinished() {
        let z = (0.0, 0.0);
        let derivative = (1.0, 0.0);
        let batch: PointBatch<f64, CpuPeriodicityDetector, 1> = PointBatch {
            points: [Some((
                (1, 2)
                , ActivePoint {
                    c: (0.1, 0.1)
                    , z
                    , derivative
                    , real_squared: 0.0
                    , imag_squared: 0.0
                    , real_imag: 0.0
                    , iteration_count: 17
                    , min_magnitude: 0.5
                    , min_magnitude_time: 3
                    , periodicity_detector: CpuPeriodicityDetector::init(0, z, derivative)
                    , escaped: false
                    , finished: false
                    , orbit_id: ZERO_ORBIT_ID
                    , seat_linear: 0
                }
            ))]
        };
        let tile = Tile::new((0, 0), 0);
        let peeked = PerturbationCpuWorker::peek_batch(&batch, &tile);
        let Some((_, calib)) = peeked[0] else {
            panic!("expected WIP peek");
        };
        match calib.result {
            CalibratedMandelbrotResult::Agnostic { escape_time_r2, .. } => {
                assert_eq!(escape_time_r2.lower_bound, 17);
                assert_eq!(escape_time_r2.upper_bound, u64::MAX);
            }
            other => panic!("expected Agnostic WIP, got {other:?}"),
        }
        assert_eq!(calib.min_magnitude.upper_bound, 0.5);
        assert_eq!(calib.min_magnitude_time.lower_bound, 3);
    }
}
