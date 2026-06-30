pub mod highlighting;
pub mod worker_implementations;
pub mod scheduler_implementations;

use crate::assemblies::structs::*;
use crate::assemblies::workgroup_new::structs::mandelbrotable::*;
use crate::assemblies::workgroup_new::structs::*;
pub trait Scheduler<T: Mandelbrotable, P: PeriodicityDetector<T>, W: Worker<T, P>>{

    type State;

    fn init_for_view(
        active_view: &mut View<()>
    ) -> Self::State;

    fn get_next_n_seats<const N:usize>(
        scheduler_state: &Self::State
        , active_view: &mut View<()> // must respect WIP and then update WIP
    ) -> [Option<((usize, usize), Option<CalibratedAnswer>)>; N];

    fn update<const N: usize>(
        scheduler_state: &mut Self::State
        , active_view: &mut View<()> // must update bitmap
        , updates: &[Option<((usize, usize), CalibratedAnswer)>; N]
    );
}

pub trait Worker<T: Mandelbrotable, P: PeriodicityDetector<T>>{

    type State;

    fn initialize_batch<const N:usize>(
        worker_state: &Self::State
        , active_view: &View<()>
        , seats: [Option<(usize, usize)>; N]
    ) -> PointBatch<T, P, N>;

    // \/ Should take about 33ms worst case, ending if the batch is done and returning false \/
    fn workshift_on_batch<const N:usize>(
        worker_state: &mut Self::State
        , active_batch: &mut PointBatch<T, P, N>
    ) -> bool;

    fn peek_batch<const N: usize>(
        active_batch: &PointBatch<T, P, N>
        , active_view: &View<()>
    ) -> [Option<((usize, usize), CalibratedAnswer)>; N];

    fn pack_batches<const N:usize, const B:usize>(
        batches: [PointBatch<T, P, N>;B]
    ) -> [Option<PointBatch<T, P, N>>;B];
}

struct ActivePoint<T: Mandelbrotable, P: PeriodicityDetector<T>> {
    c: (T, T)
    , z: (T, T)
    , iteration_count: u64
    , min_magnitude: f64
    , min_magnitude_time: u64
    , periodicity_detector: P
}

struct PointBatch<T: Mandelbrotable, P: PeriodicityDetector<T>, const N: usize> {
    points: [
        Option<(
            (usize, usize)
            , ActivePoint<T, P>
        )>; N
    ]
}

pub trait PeriodicityDetector<T: Mandelbrotable> {
    fn init(iteration_count: u64, z: (T, T)) -> Self;
    fn update(&mut self, iteration_count: u64, z: (T, T));
    fn is_periodic(&self) -> bool;
}