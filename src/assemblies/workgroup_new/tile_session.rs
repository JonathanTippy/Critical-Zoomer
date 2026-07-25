use std::collections::{HashMap, HashSet};
use std::time::Instant;

use crate::assemblies::structs::*;
use crate::assemblies::workgroup_new::structs::*;
use crate::assemblies::workgroup_new::structs::mandelbrotable::*;
use crate::assemblies::workgroup_new::workcore::mandelbrot::*;
use crate::assemblies::workgroup_new::workcore::mandelbrot::scheduler_implementations::outfill_infill_scheduler::*;
use crate::assemblies::workgroup_new::workcore::mandelbrot::scheduler_implementations::tile_scheduler::*;
use crate::assemblies::workgroup_new::workcore::mandelbrot::worker_implementations::naive_gpu_worker::*;
use crate::assemblies::workgroup_new::workcore::mandelbrot::worker_implementations::periodicity_detector::*;
use crate::constants::*;
use crate::intexp::*;
use crate::utils::ObjectivePosAndZoom;

const BATCH_N: usize = GPU_WORKER_BATCH_N;

#[derive(Clone, Copy)]
enum SeatKind {
    Outside
    , Inside { period: u32 }
}

struct ActiveTileWork {
    tile_index: usize
    , tile: Tile<()>
    , scheduler: OutfillInfillSchedulerState
    , batch: Option<PointBatch<f32, GpuPeriodicityDetector, BATCH_N>>
}

struct ScredgeWork {
    seats: [Option<(usize, usize)>; BATCH_N]
    , tile: Tile<()>
    , batch: Option<PointBatch<f32, GpuPeriodicityDetector, BATCH_N>>
}

pub struct TileSession {
    pub stencil: PointStencil
    , pub screen_res: (usize, usize)
    , pub location: ObjectivePosAndZoom
    , pub attention: (i32, i32)
    , screen_done: Vec<bool>
    , screen_kind: Vec<Option<SeatKind>>
    , screen_answer: Vec<Option<CalibratedAnswer>>
    , seats_done: usize
    , seats_total: usize
    , tile_scheduler: TileSchedulerState
    , active_tile: Option<ActiveTileWork>
    , scredge_work: Option<ScredgeWork>
    , worker_state: NaiveGpuWorkerState
    , workshifts: u32
    , answer_tiles: HashMap<(usize, usize), Tile<Answer>>
    , unsent_origins: HashSet<(usize, usize)>
    , references: ReferenceCollection
    , seat_orbit_ids: Vec<OrbitId>
}

impl TileSession {
    pub fn new(location: ObjectivePosAndZoom, screen_res: (u32, u32)) -> Self {
        let res = (screen_res.0 as usize, screen_res.1 as usize);
        let stencil = PointStencil {
            homothety: (
                location.pos.0.clone()
                , IntExp::ZERO - location.pos.1.clone()
                , location.zoom_pot
            )
            , resolution: res
            , serial_number: 0
            , focus: None
            , hover: None
        }.correct_precision();
        let mut worker_state = NaiveGpuWorkerState::default();
        worker_state.stencil = Some(stencil.clone());
        TileSession {
            stencil
            , screen_res: res
            , location: location.clone()
            , attention: ((res.0 / 2) as i32, (res.1 / 2) as i32)
            , screen_done: vec![false; res.0 * res.1]
            , screen_kind: vec![None; res.0 * res.1]
            , screen_answer: vec![None; res.0 * res.1]
            , seats_done: 0
            , seats_total: res.0 * res.1
            , tile_scheduler: TileScheduler::init(res)
            , active_tile: None
            , scredge_work: None
            , worker_state
            , workshifts: 0
            , answer_tiles: HashMap::new()
            , unsent_origins: HashSet::new()
            , references: ReferenceCollection::new()
            , seat_orbit_ids: vec![ZERO_ORBIT_ID; res.0 * res.1]
        }
    }

    pub fn set_attention(&mut self, attention: (i32, i32)) {
        self.attention = attention;
    }

    pub fn percent_completed(&self) -> f64 {
        if self.seats_total == 0 {
            return 100.0;
        }
        (self.seats_done as f64) * 100.0 / (self.seats_total as f64)
    }

    pub fn workshift(&mut self) {
        let started = Instant::now();
        while started.elapsed().as_millis() < 10 {
            if self.seats_done >= self.seats_total {
                break;
            }
            let progressed = self.work_once();
            if !progressed {
                break;
            }
        }
        self.workshifts = self.workshifts.wrapping_add(1);
    }

    pub fn drain_publish_tiles(&mut self) -> Vec<Tile<Answer>> {
        let mut out = Vec::new();
        for origin in self.unsent_origins.drain() {
            if let Some(tile) = self.answer_tiles.get(&origin) {
                out.push(*tile);
            }
        }
        out
    }

    fn work_once(&mut self) -> bool {
        if self.advance_open_batch() {
            return true;
        }
        if self.try_resolve_periods() {
            return true;
        }
        if self.active_tile.is_some() {
            let seats = {
                let work = self.active_tile.as_mut().unwrap();
                OutfillInfillScheduler::get_next_n_seats::<BATCH_N>(
                    &mut work.scheduler
                    , &mut work.tile
                )
            };
            if seats.iter().all(|s| s.is_none()) {
                let tile_index = self.active_tile.as_ref().unwrap().tile_index;
                self.active_tile = None;
                TileScheduler::note_tile_finished(&mut self.tile_scheduler, tile_index);
                return true;
            }
            let mut need_work: [Option<(usize, usize)>; BATCH_N] = [const { None }; BATCH_N];
            let mut any = false;
            for i in 0..BATCH_N {
                let Some((local, hint)) = seats[i] else { continue };
                any = true;
                if let Some(hint_answer) = hint {
                    let screen = self.active_tile.as_ref().unwrap().tile.screen_seat(local);
                    let tile_index = self.active_tile.as_ref().unwrap().tile_index;
                    self.finish_screen_seat(screen, hint_answer);
                    let updates: [Option<((usize, usize), CalibratedAnswer)>; BATCH_N] = {
                        let mut u = [const { None }; BATCH_N];
                        u[0] = Some((local, hint_answer));
                        u
                    };
                    let work = self.active_tile.as_mut().unwrap();
                    OutfillInfillScheduler::update(
                        &mut work.scheduler
                        , &mut work.tile
                        , &updates
                    );
                    if matches!(
                        seat_kind_from_calibrated(&hint_answer)
                        , SeatKind::Outside
                    ) {
                        TileScheduler::note_tile_has_outside(
                            &mut self.tile_scheduler
                            , tile_index
                        );
                    }
                } else {
                    need_work[i] = Some(local);
                }
            }
            if need_work.iter().any(|s| s.is_some()) {
                let work = self.active_tile.as_mut().unwrap();
                let batch = NaiveGpuWorker::initialize_batch(
                    &self.worker_state
                    , &work.tile
                    , need_work
                );
                work.batch = Some(batch);
                return true;
            }
            return any;
        }
        match TileScheduler::next(&mut self.tile_scheduler) {
            TileSchedulerNext::Scredge(seat) => {
                let origin = tile_origin_for_seat(seat, self.screen_res);
                let tile = Tile::new(origin, self.location.zoom_pot);
                let mut screen_seats: [Option<(usize, usize)>; BATCH_N] = [const { None }; BATCH_N];
                let mut locals: [Option<(usize, usize)>; BATCH_N] = [const { None }; BATCH_N];
                screen_seats[0] = Some(seat);
                locals[0] = Some(tile.local_seat(seat).unwrap_or((0, 0)));
                for i in 1..BATCH_N {
                    let Some(next_seat) = TileScheduler::take_scredge_for_origin(
                        &mut self.tile_scheduler
                        , origin
                        , self.screen_res
                    ) else {
                        break;
                    };
                    screen_seats[i] = Some(next_seat);
                    locals[i] = Some(tile.local_seat(next_seat).unwrap_or((0, 0)));
                }
                let batch = NaiveGpuWorker::initialize_batch(
                    &self.worker_state
                    , &tile
                    , locals
                );
                self.scredge_work = Some(ScredgeWork {
                    seats: screen_seats
                    , tile
                    , batch: Some(batch)
                });
                true
            }
            TileSchedulerNext::BeginTile(tile_index) => {
                self.begin_tile(tile_index);
                true
            }
            TileSchedulerNext::Idle => false
        }
    }

    fn advance_open_batch(&mut self) -> bool {
        if self.scredge_work.is_some() {
            let (
                finishes
                , clear_work
            ) = {
                let work = self.scredge_work.as_mut().unwrap();
                if work.batch.is_none() {
                    (Vec::new(), true)
                } else {
                    let batch = work.batch.as_mut().unwrap();
                    let still_working = NaiveGpuWorker::workshift_on_batch(
                        &mut self.worker_state
                        , batch
                    );
                    let peeked = NaiveGpuWorker::peek_batch(batch, &work.tile);
                    let mut finishes = Vec::new();
                    let mut any_finished = false;
                    for i in 0..BATCH_N {
                        let Some((_local, answer)) = peeked[i] else { continue };
                        let Some(seat) = work.seats[i] else { continue };
                        any_finished = true;
                        finishes.push((seat, answer));
                        work.seats[i] = None;
                        batch.points[i] = None;
                    }
                    if !any_finished && still_working {
                        (Vec::new(), false)
                    } else if !still_working {
                        work.batch = None;
                        (finishes, true)
                    } else {
                        (finishes, false)
                    }
                }
            };
            if clear_work {
                self.scredge_work = None;
            }
            for (seat, answer) in finishes {
                let tile_kind = match seat_kind_from_calibrated(&answer) {
                    SeatKind::Outside => TileSeatKind::Outside
                    , SeatKind::Inside { .. } => TileSeatKind::Inside
                };
                TileScheduler::note_finished(&mut self.tile_scheduler, seat, tile_kind);
                self.finish_screen_seat(seat, answer);
            }
            return true;
        }
        let (
            finishes
            , any_outside
            , tile_index
            , progressed
        ) = {
            let Some(work) = self.active_tile.as_mut() else {
                return false;
            };
            let Some(batch) = work.batch.as_mut() else {
                return false;
            };
            let still_working = NaiveGpuWorker::workshift_on_batch(
                &mut self.worker_state
                , batch
            );
            let peeked = NaiveGpuWorker::peek_batch(batch, &work.tile);
            let mut updates: [Option<((usize, usize), CalibratedAnswer)>; BATCH_N] =
                [const { None }; BATCH_N];
            let mut finishes = Vec::new();
            let mut any_outside = false;
            let mut any_finished = false;
            for i in 0..BATCH_N {
                let Some((local, answer)) = peeked[i] else { continue };
                any_finished = true;
                updates[i] = Some((local, answer));
                let screen = work.tile.screen_seat(local);
                if matches!(seat_kind_from_calibrated(&answer), SeatKind::Outside) {
                    any_outside = true;
                }
                finishes.push((screen, answer));
                batch.points[i] = None;
            }
            if !any_finished {
                if !still_working {
                    work.batch = None;
                }
                return true;
            }
            OutfillInfillScheduler::update(
                &mut work.scheduler
                , &mut work.tile
                , &updates
            );
            if !still_working {
                work.batch = None;
            }
            (finishes, any_outside, work.tile_index, true)
        };
        if any_outside {
            TileScheduler::note_tile_has_outside(&mut self.tile_scheduler, tile_index);
        }
        for (screen, answer) in finishes {
            self.finish_screen_seat(screen, answer);
        }
        progressed
    }

    fn try_resolve_periods(&mut self) -> bool {
        let Some(work) = self.active_tile.as_mut() else {
            return false;
        };
        if work.batch.is_some() {
            return false;
        }
        if !OutfillInfillScheduler::needs_period_resolve(&work.scheduler) {
            return false;
        }
        let locals = OutfillInfillScheduler::take_period_resolve_locals(&work.scheduler);
        if locals.is_empty() {
            OutfillInfillScheduler::mark_period_resolve_done(&mut work.scheduler);
            return true;
        }
        let Some(generator) = self.worker_state.stencil.as_ref().and_then(|s| {
            s.get_c_generator::<f64>()
        }) else {
            OutfillInfillScheduler::mark_period_resolve_done(&mut work.scheduler);
            return true;
        };
        let started = Instant::now();
        let mut updates = Vec::new();
        let mut attempted = 0usize;
        for local in &locals {
            if attempted >= 8 || started.elapsed().as_millis() >= 5 {
                break;
            }
            attempted += 1;
            let screen = work.tile.screen_seat(*local);
            if screen.0 >= self.screen_res.0 || screen.1 >= self.screen_res.1 {
                continue;
            }
            let c = generator.get_c((
                screen.0.min(u16::MAX as usize) as u16
                , screen.1.min(u16::MAX as usize) as u16
            ));
            let Some(period) = detect_period_for_c(c, 20_000) else {
                continue;
            };
            let period_u = period.min(u32::MAX as u64) as u32;
            OutfillInfillScheduler::apply_period_resolved(
                &mut work.scheduler
                , *local
                , period_u
            );
            if let Some(answer) = self.screen_answer[linear_index(screen, self.screen_res.0)].as_mut() {
                if let CalibratedMandelbrotResult::Inside { period: ref mut p } = answer.result {
                    p.lower_bound = period;
                    p.upper_bound = period;
                }
                updates.push((screen, *answer));
            }
        }
        OutfillInfillScheduler::mark_period_resolve_done(&mut work.scheduler);
        for (screen, answer) in updates {
            self.finish_screen_seat(screen, answer);
        }
        true
    }

    fn begin_tile(&mut self, tile_index: usize) {
        let origin = TileScheduler::tile_origin(&self.tile_scheduler, tile_index);
        let extent = TileScheduler::tile_extent(&self.tile_scheduler, tile_index);
        let tile = Tile::new(origin, self.location.zoom_pot);
        let mut scheduler = OutfillInfillScheduler::init_for_tile_extent(extent);
        for local_y in 0..extent.1 {
            for local_x in 0..extent.0 {
                let local = (local_x, local_y);
                let screen = (origin.0 + local_x, origin.1 + local_y);
                if screen.0 >= self.screen_res.0 || screen.1 >= self.screen_res.1 {
                    continue;
                }
                let index = linear_index(screen, self.screen_res.0);
                if let Some(answer) = self.screen_answer[index] {
                    OutfillInfillScheduler::absorb_known(&mut scheduler, local, answer);
                }
            }
        }
        OutfillInfillScheduler::reseed_after_absorb(&mut scheduler);
        self.active_tile = Some(ActiveTileWork {
            tile_index
            , tile
            , scheduler
            , batch: None
        });
    }

    fn finish_screen_seat(
        &mut self
        , seat: (usize, usize)
        , answer: CalibratedAnswer
    ) {
        if seat.0 >= self.screen_res.0 || seat.1 >= self.screen_res.1 {
            return;
        }
        let index = linear_index(seat, self.screen_res.0);
        let kind = seat_kind_from_calibrated(&answer);
        let newly = !self.screen_done[index];
        self.screen_done[index] = true;
        self.screen_kind[index] = Some(kind);
        self.screen_answer[index] = Some(answer);
        if newly {
            self.seats_done += 1;
        }
        let plain = calibrated_to_answer(answer);
        let origin = tile_origin_for_seat(seat, self.screen_res);
        let tile = self.answer_tiles.entry(origin).or_insert_with(|| {
            Tile::new(origin, self.location.zoom_pot)
        });
        let local = (seat.0 - origin.0, seat.1 - origin.1);
        tile.set(local, plain);
        self.unsent_origins.insert(origin);
    }
}

fn linear_index(seat: (usize, usize), screen_width: usize) -> usize {
    seat.1 * screen_width + seat.0
}

fn seat_kind_from_calibrated(answer: &CalibratedAnswer) -> SeatKind {
    match answer.result {
        CalibratedMandelbrotResult::Outside { .. } => SeatKind::Outside
        , CalibratedMandelbrotResult::Inside { period } => SeatKind::Inside {
            period: period.lower_bound.min(u32::MAX as u64) as u32
        }
        , CalibratedMandelbrotResult::Agnostic { period, .. } => SeatKind::Inside {
            period: period.lower_bound.min(u32::MAX as u64) as u32
        }
    }
}

fn calibrated_to_answer(answer: CalibratedAnswer) -> Answer {
    match answer.result {
        CalibratedMandelbrotResult::Outside { escape_time_r2, escape_z } => {
            Answer {
                result: MandelbrotResult::Outside {
                    escape_time_r2: escape_time_r2.lower_bound
                    , escape_z: (escape_z.0.lower_bound, escape_z.1.lower_bound)
                }
                , min_magnitude_time: answer.min_magnitude_time.lower_bound
                , min_magnitude: answer.min_magnitude.lower_bound
            }
        }
        , CalibratedMandelbrotResult::Inside { period } => {
            Answer {
                result: MandelbrotResult::Inside {
                    period: period.lower_bound
                }
                , min_magnitude_time: answer.min_magnitude_time.lower_bound
                , min_magnitude: answer.min_magnitude.lower_bound
            }
        }
        , CalibratedMandelbrotResult::Agnostic { period, .. } => {
            Answer {
                result: MandelbrotResult::Inside {
                    period: period.lower_bound
                }
                , min_magnitude_time: answer.min_magnitude_time.lower_bound
                , min_magnitude: answer.min_magnitude.lower_bound
            }
        }
    }
}
