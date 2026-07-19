pub mod highlighting;
pub mod worker_implementations;
pub mod scheduler_implementations;

use crate::assemblies::structs::*;
use crate::assemblies::workgroup_new::structs::mandelbrotable::*;
use crate::assemblies::workgroup_new::structs::*;
use crate::intexp::*;

pub trait Scheduler<T: Mandelbrotable, P: PeriodicityDetector<T>, W: Worker<T, P>>{

    type State;

    fn init_for_tile(
        active_view: &mut Tile<()>
    ) -> Self::State;

    fn get_next_n_seats<const N:usize>(
        scheduler_state: &mut Self::State
        , active_tile: &mut Tile<()>
    ) -> [Option<((usize, usize), Option<CalibratedAnswer>)>; N];

    fn update<const N: usize>(
        scheduler_state: &mut Self::State
        , active_tile: &mut Tile<()>
        , updates: &[Option<((usize, usize), CalibratedAnswer)>; N]
    );
}

pub trait Worker<T: Mandelbrotable, P: PeriodicityDetector<T>>{

    type State;

    fn initialize_batch<const N:usize>(
        worker_state: &Self::State
        , active_tile: &Tile<()>
        , seats: [Option<(usize, usize)>; N]
    ) -> PointBatch<T, P, N>;

    fn workshift_on_batch<const N:usize>(
        worker_state: &mut Self::State
        , active_batch: &mut PointBatch<T, P, N>
    ) -> bool;

    fn peek_batch<const N: usize>(
        active_batch: &PointBatch<T, P, N>
        , active_tile: &Tile<()>
    ) -> [Option<((usize, usize), CalibratedAnswer)>; N];

    fn pack_batches<const N:usize, const B:usize>(
        batches: [PointBatch<T, P, N>;B]
    ) -> [Option<PointBatch<T, P, N>>;B];
}

#[derive(Clone)]
pub struct ActivePoint<T: Mandelbrotable, P: PeriodicityDetector<T>> {
    pub c: (T, T)
    , pub z: (T, T)
    , pub derivative: (T, T)
    , pub real_squared: T
    , pub imag_squared: T
    , pub real_imag: T
    , pub iteration_count: u64
    , pub min_magnitude: T
    , pub min_magnitude_time: u64
    , pub periodicity_detector: P
    , pub escaped: bool
    , pub finished: bool
}

pub struct PointBatch<T: Mandelbrotable, P: PeriodicityDetector<T>, const N: usize> {
    pub points: [
        Option<(
            (usize, usize)
            , ActivePoint<T, P>
        )>; N
    ]
}

pub trait PeriodicityDetector<T: Mandelbrotable> {
    fn init(iteration_count: u64, z: (T, T), derivative: (T, T)) -> Self;
    fn check_periodicity(
        &mut self
        , c: (T, T)
        , z: (T, T)
        , derivative: (T, T)
        , iteration_count: u64
        , epsilon: T
    ) -> Option<u64>;
    fn is_periodic(&self) -> bool;
    fn detected_period(&self) -> Option<u64>;
}

struct ReferenceOrbit {
    big_c: (IntExp, IntExp)
    , period: u64
    , length: usize
    , f32: PeriodicOrbit<f32>
    , f64: PeriodicOrbit<f64>
}

impl ReferenceOrbit {
    fn assert_validity(&self) {
        assert_eq!(self.period, self.f32.period);
        assert_eq!(self.period, self.f64.period);
        assert_eq!(self.length, self.f32.big_z_orbit.len());
        assert_eq!(self.length, self.f64.big_z_orbit.len());
    }
}

struct PeriodicOrbit<T: Mandelbrotable> {
    period: u64
    , big_z_orbit: Vec<(T, T)>
    , series: Vec<(T, T)>
}


use std::ops::*;

impl<T: Mandelbrotable> Index<u64> for PeriodicOrbit<T> {
    type Output = (T, T);
    fn index(&self, index: u64) -> &(T, T) {
        let loop_start = self.big_z_orbit.len() as u64 - self.period;
        if index < loop_start {
            return &self.big_z_orbit[index as usize];
        } else {
            return &self.big_z_orbit[
                (loop_start + (index-loop_start % self.period)) as usize
            ];
        }
    }
}
