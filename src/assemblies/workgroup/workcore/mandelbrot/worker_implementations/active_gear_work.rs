// read delivery.md for project context
//! Global typed batch for the worker's active gear.
//!
//! Gear is chosen from stencil + reference and only changes when the stencil
//! changes or a new reference becomes available. One arm is live at a time;
//! dispatch sits outside bout loops.

use crate::assemblies::structs::*;
use crate::assemblies::workgroup::structs::*;
use crate::assemblies::workgroup::structs::mandelbrotable::*;
use crate::assemblies::workgroup::workcore::mandelbrot::*;
use crate::assemblies::workgroup::workcore::mandelbrot::worker_implementations::periodicity_detector::*;
use crate::assemblies::workgroup::workcore::mandelbrot::worker_implementations::perturbation_cpu_worker::{
    iterate_perturbation_bout_typed, PerturbationCpuWorker, PerturbationCpuWorkerState,
};
use crate::assemblies::workgroup::workcore::mandelbrot::worker_implementations::perturbation_gpu_worker::{
    PerturbationGpuWorker, PerturbationGpuWorkerState,
};
use crate::floatexp::FloatExp;
use crate::gear::Gear;
use crate::stacked_intexp::StackedIntExp;

/// Typed point batch for the worker's single active gear (`BATCH_N` seats).
pub enum ActiveGearWork<const N: usize> {
    F32(Box<PointBatch<f32, StandardPeriodicityDetector<f32>, N>>)
    , F64(Box<PointBatch<f64, CpuPeriodicityDetector, N>>)
    , Stacked1(Box<PointBatch<StackedIntExp<1>, StandardPeriodicityDetector<StackedIntExp<1>>, N>>)
    , Stacked2(Box<PointBatch<StackedIntExp<2>, StandardPeriodicityDetector<StackedIntExp<2>>, N>>)
    , Stacked3(Box<PointBatch<StackedIntExp<3>, StandardPeriodicityDetector<StackedIntExp<3>>, N>>)
    , Stacked4(Box<PointBatch<StackedIntExp<4>, StandardPeriodicityDetector<StackedIntExp<4>>, N>>)
    , Stacked5(Box<PointBatch<StackedIntExp<5>, StandardPeriodicityDetector<StackedIntExp<5>>, N>>)
    , Stacked6(Box<PointBatch<StackedIntExp<6>, StandardPeriodicityDetector<StackedIntExp<6>>, N>>)
    , Stacked7(Box<PointBatch<StackedIntExp<7>, StandardPeriodicityDetector<StackedIntExp<7>>, N>>)
    , Stacked8(Box<PointBatch<StackedIntExp<8>, StandardPeriodicityDetector<StackedIntExp<8>>, N>>)
    , Adaptive(Box<PointBatch<FloatExp, StandardPeriodicityDetector<FloatExp>, N>>)
}

impl<const N: usize> ActiveGearWork<N> {
    pub fn gear(&self) -> Gear {
        match self {
            Self::F32(_) => Gear::F32
            , Self::F64(_) => Gear::F64
            , Self::Stacked1(_) => Gear::StackedI32 { limbs: 1 }
            , Self::Stacked2(_) => Gear::StackedI32 { limbs: 2 }
            , Self::Stacked3(_) => Gear::StackedI32 { limbs: 3 }
            , Self::Stacked4(_) => Gear::StackedI32 { limbs: 4 }
            , Self::Stacked5(_) => Gear::StackedI32 { limbs: 5 }
            , Self::Stacked6(_) => Gear::StackedI32 { limbs: 6 }
            , Self::Stacked7(_) => Gear::StackedI32 { limbs: 7 }
            , Self::Stacked8(_) => Gear::StackedI32 { limbs: 8 }
            , Self::Adaptive(_) => Gear::AdaptiveRug
        }
    }

    /// Initialize a typed batch for `gear` from the existing f64 initialize path,
    /// then bridge into the gear's Mandelbrotable host.
    pub fn initialize(
        gear: Gear
        , worker_state: &PerturbationGpuWorkerState
        , tile: &Tile<()>
        , seats: [Option<(usize, usize)>; N]
    ) -> Self {
        let host = PerturbationGpuWorker::initialize_batch(worker_state, tile, seats);
        Self::from_host_batch(gear, host)
    }

    pub fn from_host_batch(
        gear: Gear
        , host: PointBatch<f64, CpuPeriodicityDetector, N>
    ) -> Self {
        match gear {
            Gear::F32 => Self::F32(Box::new(map_batch(&host, bridge_point::<f32>)))
            , Gear::F64 => Self::F64(Box::new(host))
            , Gear::AdaptiveRug => {
                Self::Adaptive(Box::new(map_batch(&host, bridge_point::<FloatExp>)))
            }
            , Gear::StackedI32 { limbs } => match limbs {
                1 => Self::Stacked1(Box::new(map_batch(&host, bridge_point::<StackedIntExp<1>>)))
                , 2 => Self::Stacked2(Box::new(map_batch(&host, bridge_point::<StackedIntExp<2>>)))
                , 3 => Self::Stacked3(Box::new(map_batch(&host, bridge_point::<StackedIntExp<3>>)))
                , 4 => Self::Stacked4(Box::new(map_batch(&host, bridge_point::<StackedIntExp<4>>)))
                , 5 => Self::Stacked5(Box::new(map_batch(&host, bridge_point::<StackedIntExp<5>>)))
                , 6 => Self::Stacked6(Box::new(map_batch(&host, bridge_point::<StackedIntExp<6>>)))
                , 7 => Self::Stacked7(Box::new(map_batch(&host, bridge_point::<StackedIntExp<7>>)))
                , _ => Self::Stacked8(Box::new(map_batch(&host, bridge_point::<StackedIntExp<8>>)))
            }
        }
    }

    /// CPU typed workshift (Mandelbrotable monomorph). GPU path still uses host f64.
    pub fn workshift_cpu(&mut self, cpu: &mut PerturbationCpuWorkerState) -> bool {
        let gear = self.gear();
        let eps_scale = |c: (f64, f64)| {
            gear_period_epsilon(gear, c.0.abs().max(c.1.abs()))
        };
        match self {
            Self::F64(batch) => {
                PerturbationCpuWorker::workshift_on_batch(cpu, batch)
            }
            Self::F32(batch) => workshift_typed(cpu, batch, |c| f32::from_f64(eps_scale(c)))
            , Self::Adaptive(batch) => {
                cpu.floatexp_bouts = cpu.floatexp_bouts.saturating_add(1);
                workshift_typed(cpu, batch, |c| FloatExp::from_f64(eps_scale(c)))
            }
            , Self::Stacked1(batch) => workshift_typed(cpu, batch, |c| {
                StackedIntExp::<1>::from_f64(eps_scale(c))
            })
            , Self::Stacked2(batch) => workshift_typed(cpu, batch, |c| {
                StackedIntExp::<2>::from_f64(eps_scale(c))
            })
            , Self::Stacked3(batch) => workshift_typed(cpu, batch, |c| {
                StackedIntExp::<3>::from_f64(eps_scale(c))
            })
            , Self::Stacked4(batch) => workshift_typed(cpu, batch, |c| {
                StackedIntExp::<4>::from_f64(eps_scale(c))
            })
            , Self::Stacked5(batch) => workshift_typed(cpu, batch, |c| {
                StackedIntExp::<5>::from_f64(eps_scale(c))
            })
            , Self::Stacked6(batch) => workshift_typed(cpu, batch, |c| {
                StackedIntExp::<6>::from_f64(eps_scale(c))
            })
            , Self::Stacked7(batch) => workshift_typed(cpu, batch, |c| {
                StackedIntExp::<7>::from_f64(eps_scale(c))
            })
            , Self::Stacked8(batch) => workshift_typed(cpu, batch, |c| {
                StackedIntExp::<8>::from_f64(eps_scale(c))
            })
        }
    }

    /// Bridge to host f64 for the existing GPU worker / peek path.
    pub fn to_host_batch(&self) -> PointBatch<f64, CpuPeriodicityDetector, N> {
        match self {
            Self::F64(b) => {
                let mut points: [Option<((usize, usize), ActivePoint<f64, CpuPeriodicityDetector>)>; N] =
                    [const { None }; N];
                for i in 0..N {
                    points[i] = b.points[i].clone();
                }
                PointBatch { points }
            }
            , Self::F32(b) => map_batch(b, unbridge_point::<f32>)
            , Self::Adaptive(b) => map_batch(b, unbridge_point::<FloatExp>)
            , Self::Stacked1(b) => map_batch(b, unbridge_point::<StackedIntExp<1>>)
            , Self::Stacked2(b) => map_batch(b, unbridge_point::<StackedIntExp<2>>)
            , Self::Stacked3(b) => map_batch(b, unbridge_point::<StackedIntExp<3>>)
            , Self::Stacked4(b) => map_batch(b, unbridge_point::<StackedIntExp<4>>)
            , Self::Stacked5(b) => map_batch(b, unbridge_point::<StackedIntExp<5>>)
            , Self::Stacked6(b) => map_batch(b, unbridge_point::<StackedIntExp<6>>)
            , Self::Stacked7(b) => map_batch(b, unbridge_point::<StackedIntExp<7>>)
            , Self::Stacked8(b) => map_batch(b, unbridge_point::<StackedIntExp<8>>)
        }
    }

    pub fn absorb_host_batch(&mut self, host: PointBatch<f64, CpuPeriodicityDetector, N>) {
        *self = Self::from_host_batch(self.gear(), host);
    }

    pub fn peek(
        &self
        , tile: &Tile<()>
    ) -> [Option<((usize, usize), CalibratedAnswer)>; N] {
        let host = self.to_host_batch();
        PerturbationGpuWorker::peek_batch(&host, tile)
    }

    pub fn clear_slot(&mut self, i: usize) {
        match self {
            Self::F32(b) => b.points[i] = None
            , Self::F64(b) => b.points[i] = None
            , Self::Stacked1(b) => b.points[i] = None
            , Self::Stacked2(b) => b.points[i] = None
            , Self::Stacked3(b) => b.points[i] = None
            , Self::Stacked4(b) => b.points[i] = None
            , Self::Stacked5(b) => b.points[i] = None
            , Self::Stacked6(b) => b.points[i] = None
            , Self::Stacked7(b) => b.points[i] = None
            , Self::Stacked8(b) => b.points[i] = None
            , Self::Adaptive(b) => b.points[i] = None
        }
    }

    pub fn with_host_mut<R>(
        &mut self
        , f: impl FnOnce(&mut PointBatch<f64, CpuPeriodicityDetector, N>) -> R
    ) -> R {
        let mut host = self.to_host_batch();
        let out = f(&mut host);
        self.absorb_host_batch(host);
        out
    }
}

fn workshift_typed<T, P, const N: usize>(
    cpu: &mut PerturbationCpuWorkerState
    , batch: &mut PointBatch<T, P, N>
    , epsilon_for: impl Fn((f64, f64)) -> T
) -> bool
where
    T: Mandelbrotable
    , P: PeriodicityDetector<T>
{
    let mut any = false;
    for slot in batch.points.iter_mut() {
        if let Some((_, point)) = slot {
            if point.finished {
                continue;
            }
            any = true;
            let c_abs = (point.c.0.to_f64(), point.c.1.to_f64());
            let eps = epsilon_for(c_abs);
            iterate_perturbation_bout_typed(cpu, point, eps);
        }
    }
    any
}

fn map_batch<T, P, U, Q, const N: usize>(
    src: &PointBatch<T, P, N>
    , map_point: impl Fn(&ActivePoint<T, P>) -> ActivePoint<U, Q>
) -> PointBatch<U, Q, N>
where
    T: Mandelbrotable
    , P: PeriodicityDetector<T>
    , U: Mandelbrotable
    , Q: PeriodicityDetector<U>
{
    let mut points: [Option<((usize, usize), ActivePoint<U, Q>)>; N] = [const { None }; N];
    for i in 0..N {
        if let Some((seat, point)) = &src.points[i] {
            points[i] = Some((*seat, map_point(point)));
        }
    }
    PointBatch { points }
}

fn bridge_point<T: Mandelbrotable>(
    point: &ActivePoint<f64, CpuPeriodicityDetector>
) -> ActivePoint<T, StandardPeriodicityDetector<T>> {
    let z = (T::from_f64(point.z.0), T::from_f64(point.z.1));
    let derivative = (T::from_f64(point.derivative.0), T::from_f64(point.derivative.1));
    let mut periodicity_detector = StandardPeriodicityDetector::init(
        point.iteration_count
        , z
        , derivative
    );
    periodicity_detector.checkpoint_z = (
        T::from_f64(point.periodicity_detector.checkpoint_z.0)
        , T::from_f64(point.periodicity_detector.checkpoint_z.1)
    );
    periodicity_detector.steps_since_checkpoint = point.periodicity_detector.steps_since_checkpoint;
    periodicity_detector.next_checkpoint_iteration =
        point.periodicity_detector.next_checkpoint_iteration;
    periodicity_detector.detected_period = point.periodicity_detector.detected_period;
    ActivePoint {
        c: (T::from_f64(point.c.0), T::from_f64(point.c.1))
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

fn unbridge_point<T: Mandelbrotable>(
    point: &ActivePoint<T, StandardPeriodicityDetector<T>>
) -> ActivePoint<f64, CpuPeriodicityDetector> {
    let z = (point.z.0.to_f64(), point.z.1.to_f64());
    let derivative = (point.derivative.0.to_f64(), point.derivative.1.to_f64());
    let mut periodicity_detector = CpuPeriodicityDetector::init(
        point.iteration_count
        , z
        , derivative
    );
    periodicity_detector.checkpoint_z = (
        point.periodicity_detector.checkpoint_z.0.to_f64()
        , point.periodicity_detector.checkpoint_z.1.to_f64()
    );
    periodicity_detector.steps_since_checkpoint = point.periodicity_detector.steps_since_checkpoint;
    periodicity_detector.next_checkpoint_iteration =
        point.periodicity_detector.next_checkpoint_iteration;
    periodicity_detector.detected_period = point.periodicity_detector.detected_period;
    ActivePoint {
        c: (point.c.0.to_f64(), point.c.1.to_f64())
        , z
        , derivative
        , real_squared: point.real_squared.to_f64()
        , imag_squared: point.imag_squared.to_f64()
        , real_imag: point.real_imag.to_f64()
        , iteration_count: point.iteration_count
        , min_magnitude: point.min_magnitude.to_f64()
        , min_magnitude_time: point.min_magnitude_time
        , periodicity_detector
        , escaped: point.escaped
        , finished: point.finished
        , orbit_id: point.orbit_id
        , seat_linear: point.seat_linear
    }
}

#[cfg(test)]
mod active_gear_work_tests {
    use super::*;

    #[test]
    fn from_host_preserves_gear_identity() {
        let host: PointBatch<f64, CpuPeriodicityDetector, 1> = PointBatch {
            points: [const { None }; 1]
        };
        let work = ActiveGearWork::from_host_batch(Gear::F32, host);
        assert_eq!(work.gear(), Gear::F32);
    }

    #[test]
    fn stacked_limb_arms_roundtrip_gear() {
        for limbs in 1u8..=8 {
            let host: PointBatch<f64, CpuPeriodicityDetector, 1> = PointBatch {
                points: [const { None }; 1]
            };
            let work = ActiveGearWork::from_host_batch(
                Gear::StackedI32 { limbs }
                , host
            );
            assert_eq!(work.gear(), Gear::StackedI32 { limbs });
        }
    }
}
