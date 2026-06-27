pub mod highlighting;
pub mod worker_implementations;
pub mod scheduler_implementations;

use crate::assemblies::structs::*;
use crate::assemblies::workgroup_new::structs::mandelbrotable::*;
use crate::assemblies::workgroup_new::structs::*;
pub trait Scheduler<W: Worker>{

    type State;

    fn init_for_view(
        active_view: &View<()>
    ) -> Self::State;

    fn get_next_n_seats<const N:usize>(
        scheduler_state: &Self::State
        , active_view: &View<()>
    ) -> [Option<(usize, usize)>; N];

    fn update<const N: usize>(
        scheduler_state: &mut Self::State
        , active_view: &View<()>
        , updates: &[Option<CalibratedAnswer>; N]
    );
}

pub trait Worker{

    type State;
    type PointBatch<const N:usize>;

    fn initialize_batch<const N:usize>(
        worker_state: &Self::State
        , active_view: &View<()>
        , seats: [Option<(usize, usize)>; N]
    ) -> Self::PointBatch<N>;

    fn workshift_on_batch<const N:usize>(
        worker_state: &mut Self::State
        , active_batch: &mut Self::PointBatch<N>
    );

    fn update_from_batch<const N: usize>(
        active_batch: &Self::PointBatch<N>
        , active_view: &mut View<()>
    ) -> [Option<CalibratedAnswer>; N];

    fn batch_all_done<const N: usize>(
        active_batch: &Self::PointBatch<N>
    ) -> bool;

    fn get_batch_seats<const N: usize>(
        active_batch: &Self::PointBatch<N>
    ) -> [Option<(usize, usize)>;N];
}

struct ActivePoint<T: Mandelbrotable, P: PeriodicityDetector<T>> {
    c: (T, T)
    , z: (T, T)
    , iteration_count: u64
    , min_magnitude: f64
    , min_magnitude_time: u64
    , periodicity_detector: P
}

pub trait PeriodicityDetector<T: Mandelbrotable> {
    fn init(iteration_count: u64, z: (T, T)) -> Self;
    fn update(&mut self, iteration_count: u64, z: (T, T));
    fn is_periodic(&self) -> bool;
}


// - Escape time
// - Period
// - min_magnitude_time
struct HighlightingResult {
    escaped: bool
    , period_or_escape_time_r2: u64
    ,
}