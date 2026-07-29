pub mod worker_implementations;
pub mod scheduler_implementations;

use crate::assemblies::headgroup::window::coords::*;
use crate::assemblies::structs::*;
use crate::assemblies::workgroup::structs::mandelbrotable::*;
use crate::assemblies::workgroup::structs::*;
use crate::assemblies::workgroup::workcore::mandelbrot::worker_implementations::periodicity_detector::*;
use crate::constants::*;
use crate::intexp::*;
use crate::stacked_intexp::StackedIntExp;

fn pack_f64_as_stacked(out: &mut Vec<i32>, value: f64, limbs: usize) {
    // Convert via IntExp → StackedIntExp for the requested limb count.
    let ie = f64_to_intexp(value);
    match limbs {
        1 => {
            let s = StackedIntExp::<1>::from(ie);
            out.extend_from_slice(&s.limbs);
            out.push(s.exp);
        }
        2 => {
            let s = StackedIntExp::<2>::from(ie);
            out.extend_from_slice(&s.limbs);
            out.push(s.exp);
        }
        3 => {
            let s = StackedIntExp::<3>::from(ie);
            out.extend_from_slice(&s.limbs);
            out.push(s.exp);
        }
        4 => {
            let s = StackedIntExp::<4>::from(ie);
            out.extend_from_slice(&s.limbs);
            out.push(s.exp);
        }
        5 => {
            let s = StackedIntExp::<5>::from(ie);
            out.extend_from_slice(&s.limbs);
            out.push(s.exp);
        }
        6 => {
            let s = StackedIntExp::<6>::from(ie);
            out.extend_from_slice(&s.limbs);
            out.push(s.exp);
        }
        7 => {
            let s = StackedIntExp::<7>::from(ie);
            out.extend_from_slice(&s.limbs);
            out.push(s.exp);
        }
        8 => {
            let s = StackedIntExp::<8>::from(ie);
            out.extend_from_slice(&s.limbs);
            out.push(s.exp);
        }
        _ => panic!("limbs must be 1..=8"),
    }
}

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
    , pub orbit_id: OrbitId
    , pub seat_linear: usize
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

pub struct ReferenceOrbit {
    pub big_c: (IntExp, IntExp)
    , pub period: u64
    , pub length: usize
    , pub f32: PeriodicOrbit<f32>
    , pub f64: PeriodicOrbit<f64>
    // Packed stacked mirrors for limbs 1..=8 (index = limbs - 1).
    // Each sample: `limbs` re limbs, `limbs` im limbs, re_exp, im_exp.
    , pub stacked: [Vec<i32>; 8]
    // Packed stacked SA coefficients (same layout), empty when SA unsupported.
    , pub stacked_series: [Vec<i32>; 8]
}

impl ReferenceOrbit {
    pub fn assert_validity(&self) {
        assert_eq!(self.period, self.f32.period);
        assert_eq!(self.period, self.f64.period);
        assert_eq!(self.length, self.f32.big_z_orbit.len());
        assert_eq!(self.length, self.f64.big_z_orbit.len());
    }

    /// Pack the f64 orbit into stacked-i32 limbs for a GPU/CPU gear (1..=8).
    /// Prefers the stored mirror when present; otherwise builds on the fly.
    pub fn stacked_orbit_mirror(&self, limbs: u8) -> Vec<i32> {
        assert!((1..=8).contains(&limbs));
        let stored = &self.stacked[(limbs - 1) as usize];
        if !stored.is_empty() {
            return stored.clone();
        }
        let n = limbs as usize;
        let mut out = Vec::with_capacity(self.f64.big_z_orbit.len() * (2 * n + 2));
        for &(re, im) in &self.f64.big_z_orbit {
            pack_f64_as_stacked(&mut out, re, n);
            pack_f64_as_stacked(&mut out, im, n);
        }
        out
    }

    pub fn stacked_series_mirror(&self, limbs: u8) -> Vec<i32> {
        assert!((1..=8).contains(&limbs));
        let stored = &self.stacked_series[(limbs - 1) as usize];
        if !stored.is_empty() {
            return stored.clone();
        }
        let n = limbs as usize;
        let mut out = Vec::with_capacity(self.f64.series.len() * (2 * n + 2));
        for &(re, im) in &self.f64.series {
            pack_f64_as_stacked(&mut out, re, n);
            pack_f64_as_stacked(&mut out, im, n);
        }
        out
    }

    fn fill_stacked_mirrors_from_f64(f64_orbit: &PeriodicOrbit<f64>) -> ([Vec<i32>; 8], [Vec<i32>; 8]) {
        let mut stacked: [Vec<i32>; 8] = Default::default();
        let mut stacked_series: [Vec<i32>; 8] = Default::default();
        for limbs in 1u8..=8 {
            let n = limbs as usize;
            let mut orbit_pack = Vec::with_capacity(f64_orbit.big_z_orbit.len() * (2 * n + 2));
            for &(re, im) in &f64_orbit.big_z_orbit {
                pack_f64_as_stacked(&mut orbit_pack, re, n);
                pack_f64_as_stacked(&mut orbit_pack, im, n);
            }
            stacked[(limbs - 1) as usize] = orbit_pack;
            let mut series_pack = Vec::with_capacity(f64_orbit.series.len() * (2 * n + 2));
            for &(re, im) in &f64_orbit.series {
                pack_f64_as_stacked(&mut series_pack, re, n);
                pack_f64_as_stacked(&mut series_pack, im, n);
            }
            stacked_series[(limbs - 1) as usize] = series_pack;
        }
        (stacked, stacked_series)
    }

    pub fn zero() -> Self {
        let f32 = PeriodicOrbit {
            period: 1
            , big_z_orbit: vec![(0.0f32, 0.0f32)]
            , series: Vec::new()
        };
        let f64 = PeriodicOrbit {
            period: 1
            , big_z_orbit: vec![(0.0f64, 0.0f64)]
            , series: Vec::new()
        };
        let (stacked, stacked_series) = Self::fill_stacked_mirrors_from_f64(&f64);
        ReferenceOrbit {
            big_c: (IntExp::ZERO, IntExp::ZERO)
            , period: 1
            , length: 1
            , f32
            , f64
            , stacked
            , stacked_series
        }
    }

    pub fn approx_bytes(&self) -> usize {
        let point_bytes = self.length
            * (
                std::mem::size_of::<(f32, f32)>()
                + std::mem::size_of::<(f64, f64)>()
            );
        let stacked_bytes: usize = self.stacked.iter().map(|v: &Vec<i32>| v.len() * 4).sum();
        let stacked_series_bytes: usize = self.stacked_series.iter().map(|v: &Vec<i32>| v.len() * 4).sum();
        point_bytes
            + self.f32.series.len() * std::mem::size_of::<(f32, f32)>()
            + self.f64.series.len() * std::mem::size_of::<(f64, f64)>()
            + stacked_bytes
            + stacked_series_bytes
            + std::mem::size_of::<ReferenceOrbit>()
    }
}

pub struct PeriodicOrbit<T: Mandelbrotable> {
    pub period: u64
    , pub big_z_orbit: Vec<(T, T)>
    , pub series: Vec<(T, T)>
}

impl<T: Mandelbrotable> PeriodicOrbit<T> {
    /// Sample Z at iteration `index`, wrapping the periodic tail.
    #[inline(always)]
    pub fn z_at(&self, index: u64) -> (T, T) {
        let len = self.big_z_orbit.len() as u64;
        debug_assert!(len > 0, "empty periodic orbit");
        if len <= 1 {
            return self.big_z_orbit[0];
        }
        let period = self.period.max(1);
        let loop_start = len - period;
        if index < loop_start {
            self.big_z_orbit[index as usize]
        } else {
            let offset = (index - loop_start) % period;
            self.big_z_orbit[(loop_start + offset) as usize]
        }
    }
}

use std::ops::*;

impl<T: Mandelbrotable> Index<u64> for PeriodicOrbit<T> {
    type Output = (T, T);
    #[inline(always)]
    fn index(&self, index: u64) -> &(T, T) {
        let len = self.big_z_orbit.len() as u64;
        if len == 0 {
            panic!("empty periodic orbit");
        }
        if len == 1 {
            return &self.big_z_orbit[0];
        }
        let period = self.period.max(1);
        let loop_start = len - period;
        if index < loop_start {
            &self.big_z_orbit[index as usize]
        } else {
            let offset = (index - loop_start) % period;
            &self.big_z_orbit[(loop_start + offset) as usize]
        }
    }
}

pub type OrbitId = u32;

pub const ZERO_ORBIT_ID: OrbitId = 0;

pub struct ReferenceCollection {
    orbits: Vec<ReferenceOrbit>
}

impl ReferenceCollection {
    pub fn new() -> Self {
        ReferenceCollection {
            orbits: vec![ReferenceOrbit::zero()]
        }
    }

    pub fn get(&self, id: OrbitId) -> Option<&ReferenceOrbit> {
        self.orbits.get(id as usize)
    }

    pub fn len(&self) -> usize {
        self.orbits.len()
    }

    pub fn approx_bytes(&self) -> usize {
        self.orbits.iter().map(ReferenceOrbit::approx_bytes).sum()
    }

    pub fn bind_seat(seat_orbit_ids: &mut [OrbitId], seat_linear: usize, id: OrbitId) {
        if let Some(slot) = seat_orbit_ids.get_mut(seat_linear) {
            *slot = id;
        }
    }

    pub fn seat_orbit_id(seat_orbit_ids: &[OrbitId], seat_linear: usize) -> OrbitId {
        seat_orbit_ids
            .get(seat_linear)
            .copied()
            .unwrap_or(ZERO_ORBIT_ID)
    }

    pub fn try_add_nucleus_at_c(&mut self, big_c: (IntExp, IntExp)) -> OrbitId {
        let c = (big_c.0.clone().to_f64(), big_c.1.clone().to_f64());
        self.try_add_nucleus_at_f64_with_big_c(c, big_c)
    }

    pub fn try_add_nucleus_at_f64(&mut self, c: (f64, f64)) -> OrbitId {
        // r[impl cz.seamless.reference-background+1]
        let big_c = (f64_to_intexp(c.0), f64_to_intexp(c.1));
        self.try_add_nucleus_at_f64_with_big_c(c, big_c)
    }

    fn try_add_nucleus_at_f64_with_big_c(
        &mut self
        , c: (f64, f64)
        , big_c: (IntExp, IntExp)
    ) -> OrbitId {
        if c.0 == 0.0 && c.1 == 0.0 {
            return ZERO_ORBIT_ID;
        }
        let Some(period) = detect_period_for_c(c, REFERENCE_NUCLEUS_SEEK_ITERS_INTERACTIVE) else {
            return ZERO_ORBIT_ID;
        };
        let Some(orbit) = build_reference_orbit_f64(big_c, c, period) else {
            return ZERO_ORBIT_ID;
        };
        if self.approx_bytes() + orbit.approx_bytes() > REFERENCE_ORBIT_COLLECTION_BUDGET_BYTES {
            return ZERO_ORBIT_ID;
        }
        let id = self.orbits.len() as OrbitId;
        if id == OrbitId::MAX {
            return ZERO_ORBIT_ID;
        }
        self.orbits.push(orbit);
        id
    }
}

fn build_reference_orbit_f64(
    big_c: (IntExp, IntExp)
    , c: (f64, f64)
    , period: u64
) -> Option<ReferenceOrbit> {
    let period = period.max(1);
    let length = (period as usize).saturating_add(1).max(1);
    let mut orbit_f64 = Vec::with_capacity(length);
    let mut z = (0.0f64, 0.0f64);
    orbit_f64.push(z);
    for _ in 1..length {
        let rad = z.0 * z.0 + z.1 * z.1;
        if rad > 4.0 {
            return None;
        }
        z = (
            z.0 * z.0 - z.1 * z.1 + c.0
            , 2.0 * z.0 * z.1 + c.1
        );
        orbit_f64.push(z);
    }
    let mut series_f64 = Vec::with_capacity(orbit_f64.len());
    series_f64.push((0.0f64, 0.0f64));
    if orbit_f64.len() > 1 {
        series_f64.push((1.0f64, 0.0f64));
        for n in 1..orbit_f64.len() - 1 {
            let z = orbit_f64[n];
            let a = series_f64[n];
            let two_z_a = (
                2.0 * (z.0 * a.0 - z.1 * a.1)
                , 2.0 * (z.0 * a.1 + z.1 * a.0)
            );
            series_f64.push((two_z_a.0 + 1.0, two_z_a.1));
        }
    }
    let orbit_f32: Vec<(f32, f32)> = orbit_f64
        .iter()
        .map(|&(re, im)| (re as f32, im as f32))
        .collect();
    let series_f32: Vec<(f32, f32)> = series_f64
        .iter()
        .map(|&(re, im)| (re as f32, im as f32))
        .collect();
    let f64_orbit = PeriodicOrbit {
        period
        , big_z_orbit: orbit_f64
        , series: series_f64
    };
    let (stacked, stacked_series) = ReferenceOrbit::fill_stacked_mirrors_from_f64(&f64_orbit);
    let orbit = ReferenceOrbit {
        big_c
        , period
        , length: f64_orbit.big_z_orbit.len()
        , f32: PeriodicOrbit {
            period
            , big_z_orbit: orbit_f32
            , series: series_f32
        }
        , f64: f64_orbit
        , stacked
        , stacked_series
    };
    orbit.assert_validity();
    Some(orbit)
}

#[cfg(test)]
mod reference_collection_tests {
    use super::*;

    // r[verify cz.seamless.reference-background+1]
    #[test]
    fn zero_orbit_id_is_immortal_slot_zero() {
        let collection = ReferenceCollection::new();
        assert_eq!(ZERO_ORBIT_ID, 0);
        assert_eq!(collection.len(), 1);
        let zero = collection.get(ZERO_ORBIT_ID).expect("zero orbit");
        assert_eq!(zero.period, 1);
        assert_eq!(zero.length, 1);
        assert_eq!(zero.big_c.0, IntExp::ZERO);
        assert_eq!(zero.big_c.1, IntExp::ZERO);
        assert_eq!(zero.f64.big_z_orbit[0], (0.0, 0.0));
        assert_eq!(zero.f32.big_z_orbit[0], (0.0, 0.0));
    }

    #[test]
    fn stacked_orbit_mirror_packs_one_through_eight() {
        let zero = ReferenceOrbit::zero();
        for limbs in 1u8..=8 {
            let packed = zero.stacked_orbit_mirror(limbs);
            // one sample: re limbs+exp + im limbs+exp
            assert_eq!(packed.len(), 2 * (limbs as usize + 1));
            assert!(
                !zero.stacked[(limbs - 1) as usize].is_empty()
                , "stored stacked mirror must be filled for limbs={limbs}"
            );
        }
    }

    #[test]
    fn build_reference_fills_all_stacked_mirrors() {
        let orbit = build_reference_orbit_f64(
            (IntExp::from(-1), IntExp::ZERO)
            , (-1.0, 0.0)
            , 2
        )
        .expect("period-2 nucleus orbit");
        for limbs in 1u8..=8 {
            assert_eq!(
                orbit.stacked[(limbs - 1) as usize].len()
                , orbit.length * 2 * (limbs as usize + 1)
            );
        }
    }

    #[test]
    fn collection_get_zero_works() {
        let collection = ReferenceCollection::new();
        assert!(collection.get(0).is_some());
        assert!(collection.get(1).is_none());
    }

    #[test]
    fn cardioid_nucleus_adds_nonzero_or_binds_zero() {
        let mut collection = ReferenceCollection::new();
        let c = cardioid_c_from_mu((0.25, 0.0));
        let id = collection.try_add_nucleus_at_f64(c);
        if id == ZERO_ORBIT_ID {
            assert!(collection.get(ZERO_ORBIT_ID).is_some());
        } else {
            assert_ne!(id, ZERO_ORBIT_ID);
            let orbit = collection.get(id).expect("added orbit");
            assert!(orbit.period >= 1);
            assert!(orbit.length >= 1);
            orbit.assert_validity();
        }
    }
}
