use std::collections::{HashMap, HashSet};
use std::time::Instant;

use crate::assemblies::structs::*;
use crate::assemblies::workgroup_new::structs::*;
use crate::assemblies::workgroup_new::structs::mandelbrotable::*;
use crate::assemblies::workgroup_new::workcore::mandelbrot::*;
use crate::assemblies::workgroup_new::workcore::mandelbrot::scheduler_implementations::outfill_infill_scheduler::*;
use crate::assemblies::workgroup_new::workcore::mandelbrot::scheduler_implementations::tile_scheduler::*;
use crate::assemblies::workgroup_new::workcore::mandelbrot::worker_implementations::perturbation_gpu_worker::*;
use crate::assemblies::workgroup_new::workcore::mandelbrot::worker_implementations::periodicity_detector::*;
use crate::assemblies::workgroup_new::workcore::mandelbrot::worker_implementations::reference_worker::ReferenceWorker;
use crate::constants::*;
use crate::intexp::*;
use crate::utils::{ObjectivePosAndZoom, Shiftable};

const BATCH_N: usize = GPU_WORKER_BATCH_N;

// r[impl cz.seamless.perturbation-always-on+1]
// r[impl cz.seamless.gpu-preferred+1]
// Live TileSession always iterates through PerturbationGpuWorker (perturbation
// math; GPU preferred when an adapter exists, else CPU). There is no
// naive/perturbation switch and no user-facing GPU or perturbation toggle.
// Reference orbits are owned by ReferenceWorker (mag-change updates; keep old
// until ready; in-flight work retains old orbits) per docs/design/reference_worker.md.

#[derive(Clone, Copy)]
enum SeatKind {
    Outside
    , Inside { period: u32 }
}

struct ActiveTileWork {
    tile_index: usize
    , tile: Tile<()>
    , scheduler: OutfillInfillSchedulerState
    , batch: Option<PointBatch<f64, CpuPeriodicityDetector, BATCH_N>>
}

struct ScredgeWork {
    seats: [Option<(usize, usize)>; BATCH_N]
    , tile: Tile<()>
    , batch: Option<PointBatch<f64, CpuPeriodicityDetector, BATCH_N>>
}

struct LookaheadWork {
    publish_location: ObjectivePosAndZoom
    , tile: Tile<()>
    , answer_tile: Box<Tile<Answer>>
    , scheduler: OutfillInfillSchedulerState
    , batch: Option<PointBatch<f64, CpuPeriodicityDetector, BATCH_N>>
    , saved_stencil: Option<PointStencil>
    , saved_seat_orbit_ids: Vec<OrbitId>
    , saved_screen_width: usize
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
    , active_tile: Option<Box<ActiveTileWork>>
    , scredge_work: Option<Box<ScredgeWork>>
    , lookahead_work: Option<Box<LookaheadWork>>
    , worker_state: PerturbationGpuWorkerState // r[impl cz.seamless.perturbation-always-on+1]
    , reference_worker: ReferenceWorker // r[impl cz.seamless.reference-background+1]
    , workshifts: u32
    , answer_tiles: HashMap<(usize, usize), Tile<Answer>>
    , unsent_origins: HashSet<(usize, usize)>
    , lookahead_unsent: Vec<(ObjectivePosAndZoom, Box<Tile<Answer>>)>
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
        // Prefer GPU when available; always perturbation (no toggles).
        let mut worker_state = PerturbationGpuWorkerState::prefer_available_gpu();
        worker_state.stencil = Some(stencil.clone());
        worker_state.screen_width = res.0;
        // r[impl cz.seamless.reference-background+1]
        // Seed reference orbit via the background API (no UI); bind every seat.
        let reference_worker = ReferenceWorker::seed_into(
            &mut worker_state.cpu.references
            , (stencil.homothety.0.clone(), stencil.homothety.1.clone())
            , location.zoom_pot
        );
        let orbit_id = reference_worker.bound_orbit_id();
        worker_state.seat_orbit_ids = vec![orbit_id; res.0 * res.1];
        let attention = ((res.0 / 2) as i32, (res.1 / 2) as i32);
        let mut tile_scheduler = TileScheduler::init(res);
        TileScheduler::set_base_mag(&mut tile_scheduler, location.zoom_pot);
        TileScheduler::set_attention(&mut tile_scheduler, attention);
        TileSession {
            stencil
            , screen_res: res
            , location: location.clone()
            , attention
            , screen_done: vec![false; res.0 * res.1]
            , screen_kind: vec![None; res.0 * res.1]
            , screen_answer: vec![None; res.0 * res.1]
            , seats_done: 0
            , seats_total: res.0 * res.1
            , tile_scheduler
            , active_tile: None
            , scredge_work: None
            , lookahead_work: None
            , worker_state
            , reference_worker
            , workshifts: 0
            , answer_tiles: HashMap::new()
            , unsent_origins: HashSet::new()
            , lookahead_unsent: Vec::new()
        }
    }

    pub fn set_attention(&mut self, attention: (i32, i32)) {
        self.attention = attention;
        TileScheduler::set_attention(&mut self.tile_scheduler, attention);
    }

    pub fn set_mag_velocity(&mut self, mag_velocity: i32) {
        TileScheduler::set_mag_velocity(&mut self.tile_scheduler, mag_velocity);
    }

    /// Retarget an existing session to a new stencil without always wiping
    /// reference orbits. Same-mag pans keep the bound references (design updates
    /// refs on mag change); zoom/res changes rebuild screen state and notify the
    /// reference worker so the old orbit stays until the new one is ready.
    pub fn retarget(&mut self, location: ObjectivePosAndZoom, screen_res: (u32, u32)) {
        let new_res = (screen_res.0 as usize, screen_res.1 as usize);
        let mag_vel = location.zoom_pot - self.location.zoom_pot;
        if new_res != self.screen_res {
            let attention = self.attention;
            *self = Self::new(location, screen_res);
            self.set_attention(attention);
            self.set_mag_velocity(mag_vel);
            return;
        }
        if location.zoom_pot != self.location.zoom_pot {
            // Mag change: keep reference collection, notify background worker,
            // poll to bind the new orbit when ready; rebuild screen/tile state.
            let attention = self.attention;
            let mut worker_state = std::mem::take(&mut self.worker_state);
            let mut reference_worker = std::mem::replace(
                &mut self.reference_worker
                , ReferenceWorker::empty()
            );
            // Mark any open work as still using the old bound orbit.
            let old_bound = reference_worker.bound_orbit_id();
            let _ = reference_worker.begin_work_with_bound();
            reference_worker.notify_mag_change(
                (
                    location.pos.0.clone()
                    , IntExp::ZERO - location.pos.1.clone()
                )
                , location.zoom_pot
            );
            reference_worker.poll(&mut worker_state.cpu.references);
            let new_bound = reference_worker.bound_orbit_id();
            // Old in-flight claim remains until we end_work below.
            let res = new_res;
            let stencil = PointStencil {
                homothety: (
                    location.pos.0.clone()
                    , IntExp::ZERO - location.pos.1.clone()
                    , location.zoom_pot
                )
                , resolution: res
                , serial_number: self.stencil.serial_number.wrapping_add(1)
                , focus: None
                , hover: None
            }.correct_precision();
            worker_state.stencil = Some(stencil.clone());
            worker_state.screen_width = res.0;
            worker_state.seat_orbit_ids = vec![new_bound; res.0 * res.1];
            reference_worker.end_work(old_bound);
            let mut tile_scheduler = TileScheduler::init(res);
            TileScheduler::set_base_mag(&mut tile_scheduler, location.zoom_pot);
            TileScheduler::set_attention(&mut tile_scheduler, attention);
            let rebuilt = Box::new(TileSession {
                stencil
                , screen_res: res
                , location: location.clone()
                , attention
                , screen_done: vec![false; res.0 * res.1]
                , screen_kind: vec![None; res.0 * res.1]
                , screen_answer: vec![None; res.0 * res.1]
                , seats_done: 0
                , seats_total: res.0 * res.1
                , tile_scheduler
                , active_tile: None
                , scredge_work: None
                , lookahead_work: None
                , worker_state
                , reference_worker
                , workshifts: 0
                , answer_tiles: HashMap::new()
                , unsent_origins: HashSet::new()
                , lookahead_unsent: Vec::new()
            });
            *self = *rebuilt;
            self.set_mag_velocity(mag_vel);
            return;
        }
        if self.location == location {
            return;
        }
        self.location = location.clone();
        let res = self.screen_res;
        self.stencil = PointStencil {
            homothety: (
                location.pos.0.clone()
                , IntExp::ZERO - location.pos.1.clone()
                , location.zoom_pot
            )
            , resolution: res
            , serial_number: self.stencil.serial_number.wrapping_add(1)
            , focus: None
            , hover: None
        }.correct_precision();
        self.worker_state.stencil = Some(self.stencil.clone());
        self.worker_state.screen_width = res.0;
        self.screen_done.fill(false);
        self.screen_kind.fill(None);
        self.screen_answer.fill(None);
        self.seats_done = 0;
        self.tile_scheduler = TileScheduler::init(res);
        TileScheduler::set_base_mag(&mut self.tile_scheduler, location.zoom_pot);
        TileScheduler::set_attention(&mut self.tile_scheduler, self.attention);
        self.active_tile = None;
        self.scredge_work = None;
        self.lookahead_work = None;
        self.answer_tiles.clear();
        self.unsent_origins.clear();
        self.lookahead_unsent.clear();
        self.set_mag_velocity(0);
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

    /// Lookahead tiles carry their own location (deeper mag under attention).
    pub fn drain_lookahead_publishes(&mut self) -> Vec<(ObjectivePosAndZoom, Box<Tile<Answer>>)> {
        std::mem::take(&mut self.lookahead_unsent)
    }

    fn work_once(&mut self) -> bool {
        if self.advance_open_batch() {
            return true;
        }
        if self.advance_lookahead_work() {
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
                let batch = PerturbationGpuWorker::initialize_batch(
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
                let batch = PerturbationGpuWorker::initialize_batch(
                    &self.worker_state
                    , &tile
                    , locals
                );
                self.scredge_work = Some(Box::new(ScredgeWork {
                    seats: screen_seats
                    , tile
                    , batch: Some(batch)
                }));
                true
            }
            TileSchedulerNext::BeginTile(tile_index) => {
                self.begin_tile(tile_index);
                true
            }
            TileSchedulerNext::BeginLookahead { zoom_pot } => {
                self.begin_lookahead(zoom_pot);
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
                    let still_working = PerturbationGpuWorker::workshift_on_batch(
                        &mut self.worker_state
                        , batch
                    );
                    let peeked = PerturbationGpuWorker::peek_batch(batch, &work.tile);
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
            let still_working = PerturbationGpuWorker::workshift_on_batch(
                &mut self.worker_state
                , batch
            );
            let peeked = PerturbationGpuWorker::peek_batch(batch, &work.tile);
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
        self.active_tile = Some(Box::new(ActiveTileWork {
            tile_index
            , tile
            , scheduler
            , batch: None
        }));
    }

    fn begin_lookahead(&mut self, zoom_pot: i32) {
        let Some(publish_location) = attention_tile_location(
            &self.location
            , self.attention
            , zoom_pot
        ) else {
            return;
        };
        let edge = TILE_EDGE_LENGTH;
        let stencil = PointStencil {
            homothety: (
                publish_location.pos.0.clone()
                , IntExp::ZERO - publish_location.pos.1.clone()
                , zoom_pot
            )
            , resolution: (edge, edge)
            , serial_number: self.stencil.serial_number.wrapping_add(1)
            , focus: None
            , hover: None
        }.correct_precision();
        let saved_stencil = self.worker_state.stencil.take();
        let saved_seat_orbit_ids = std::mem::take(&mut self.worker_state.seat_orbit_ids);
        let saved_screen_width = self.worker_state.screen_width;
        self.worker_state.stencil = Some(stencil.clone());
        self.worker_state.screen_width = edge;
        let orbit_id = self.worker_state.references.try_add_nucleus_at_c((
            stencil.homothety.0.clone()
            , stencil.homothety.1.clone()
        ));
        self.worker_state.seat_orbit_ids = vec![orbit_id; edge * edge];
        let tile = Tile::new((0, 0), zoom_pot);
        let scheduler = OutfillInfillScheduler::init_for_tile_extent((edge, edge));
        self.lookahead_work = Some(Box::new(LookaheadWork {
            publish_location
            , tile
            , answer_tile: boxed_empty_answer_tile(zoom_pot)
            , scheduler
            , batch: None
            , saved_stencil
            , saved_seat_orbit_ids
            , saved_screen_width
        }));
    }

    fn restore_worker_from_lookahead(work: &LookaheadWork, worker: &mut PerturbationGpuWorkerState) {
        worker.stencil = work.saved_stencil.clone();
        worker.seat_orbit_ids = work.saved_seat_orbit_ids.clone();
        worker.screen_width = work.saved_screen_width;
    }

    fn finish_lookahead(&mut self) {
        let Some(work) = self.lookahead_work.take() else {
            return;
        };
        Self::restore_worker_from_lookahead(&work, &mut self.worker_state);
        // Only publish if at least one seat answered.
        if work.answer_tile.data.iter().any(|c| c.is_some()) {
            self.lookahead_unsent.push((work.publish_location, work.answer_tile));
        }
    }

    fn advance_lookahead_work(&mut self) -> bool {
        if self.lookahead_work.is_none() {
            return false;
        }
        // Drive outfill like active_tile, writing into answer_tile only.
        let seats = {
            let work = self.lookahead_work.as_mut().unwrap();
            OutfillInfillScheduler::get_next_n_seats::<BATCH_N>(
                &mut work.scheduler
                , &mut work.tile
            )
        };
        if seats.iter().all(|s| s.is_none()) {
            // Tile finished (or never started seats) — close the bump.
            if self.lookahead_work.as_ref().unwrap().batch.is_none() {
                self.finish_lookahead();
                return true;
            }
        }
        let mut need_work: [Option<(usize, usize)>; BATCH_N] = [const { None }; BATCH_N];
        let mut any = false;
        for i in 0..BATCH_N {
            let Some((local, hint)) = seats[i] else { continue };
            any = true;
            if let Some(hint_answer) = hint {
                let plain = calibrated_to_answer(hint_answer);
                let work = self.lookahead_work.as_mut().unwrap();
                work.answer_tile.set(local, plain);
                let updates: [Option<((usize, usize), CalibratedAnswer)>; BATCH_N] = {
                    let mut u = [const { None }; BATCH_N];
                    u[0] = Some((local, hint_answer));
                    u
                };
                OutfillInfillScheduler::update(
                    &mut work.scheduler
                    , &mut work.tile
                    , &updates
                );
            } else {
                need_work[i] = Some(local);
            }
        }
        if need_work.iter().any(|s| s.is_some()) {
            let work = self.lookahead_work.as_mut().unwrap();
            let batch = PerturbationGpuWorker::initialize_batch(
                &self.worker_state
                , &work.tile
                , need_work
            );
            work.batch = Some(batch);
            return true;
        }
        if let Some(work) = self.lookahead_work.as_mut() {
            if let Some(batch) = work.batch.as_mut() {
                let still = PerturbationGpuWorker::workshift_on_batch(
                    &mut self.worker_state
                    , batch
                );
                let peeked = PerturbationGpuWorker::peek_batch(batch, &work.tile);
                let mut updates: [Option<((usize, usize), CalibratedAnswer)>; BATCH_N] =
                    [const { None }; BATCH_N];
                let mut any_finished = false;
                for i in 0..BATCH_N {
                    let Some((local, answer)) = peeked[i] else { continue };
                    any_finished = true;
                    updates[i] = Some((local, answer));
                    work.answer_tile.set(local, calibrated_to_answer(answer));
                    batch.points[i] = None;
                }
                if any_finished {
                    OutfillInfillScheduler::update(
                        &mut work.scheduler
                        , &mut work.tile
                        , &updates
                    );
                }
                if !still {
                    work.batch = None;
                }
                return true;
            }
        }
        if !any {
            self.finish_lookahead();
            return true;
        }
        true
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

fn floor_div_tile_i32(v: i32) -> i32 {
    let edge = TILE_EDGE_LENGTH as i32;
    let d = v / edge;
    let r = v % edge;
    if r != 0 && v < 0 {
        d - 1
    } else {
        d
    }
}

fn intexp_seat_i32(v: IntExp) -> Option<i32> {
    v.val.shift(v.exp).to_i32()
}

/// Absolute dyadic tile under attention at `zoom_pot`, as a location whose UL is
/// that tile's UL (origin_seat (0,0) in the published tile).
fn attention_tile_location(
    location: &ObjectivePosAndZoom
    , attention: (i32, i32)
    , zoom_pot: i32
) -> Option<ObjectivePosAndZoom> {
    let pixel = -(location.zoom_pot + PIXELS_PER_UNIT_POT);
    let att = (
        location.pos.0.clone() + IntExp::from(attention.0).shift(pixel)
        , location.pos.1.clone() + IntExp::from(attention.1).shift(pixel)
    );
    let wx = intexp_seat_i32(
        att.0.clone().shift(zoom_pot).shift(PIXELS_PER_UNIT_POT)
    )?;
    let wy = intexp_seat_i32(
        att.1.clone().shift(zoom_pot).shift(PIXELS_PER_UNIT_POT)
    )?;
    let ox = floor_div_tile_i32(wx);
    let oy = floor_div_tile_i32(wy);
    let edge = TILE_EDGE_LENGTH as i32;
    let ul_x = ox.saturating_mul(edge);
    let ul_y = oy.saturating_mul(edge);
    let ul_pixel = -(zoom_pot + PIXELS_PER_UNIT_POT);
    Some(ObjectivePosAndZoom {
        pos: (
            IntExp::from(ul_x).shift(ul_pixel)
            , IntExp::from(ul_y).shift(ul_pixel)
        )
        , zoom_pot
    })
}

/// Build a full answer tile on the heap (64² Option<Answer> must not touch the stack).
fn boxed_empty_answer_tile(magnification_pot: i32) -> Box<Tile<Answer>> {
    let mut tile = Box::<Tile<Answer>>::new_uninit();
    let ptr = tile.as_mut_ptr();
    unsafe {
        std::ptr::addr_of_mut!((*ptr).origin_seat).write((0, 0));
        std::ptr::addr_of_mut!((*ptr).magnification_pot).write(magnification_pot);
        for i in 0..TILE_SEAT_COUNT {
            std::ptr::addr_of_mut!((*ptr).data[i]).write(None);
        }
        tile.assume_init()
    }
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

#[cfg(test)]
mod perturbation_always_on_tests {
    use super::*;
    use crate::assemblies::workgroup_new::workcore::mandelbrot::worker_implementations::naive_cpu_worker::{
        iterate_point_bout
        , point_to_answer as naive_point_to_answer
    };
    use proptest::prelude::*;

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

    fn same_membership(a: &Answer, b: &Answer) -> bool {
        match (&a.result, &b.result) {
            (
                MandelbrotResult::Outside { escape_time_r2: ea, .. }
                , MandelbrotResult::Outside { escape_time_r2: eb, .. }
            ) => ea == eb
            , (MandelbrotResult::Inside { .. }, MandelbrotResult::Inside { .. }) => true
            , _ => false
        }
    }

    // r[verify cz.seamless.perturbation-always-on+1]
    // r[verify cz.seamless.reference-background+1]
    #[test]
    fn new_session_seeds_nonzero_reference_orbit_at_period_two_corner() {
        let location = ObjectivePosAndZoom {
            pos: (IntExp::from(-1), IntExp::ZERO)
            , zoom_pot: 4
        };
        let session = TileSession::new(location, (8, 8));
        assert!(
            session.worker_state.references.len() > 1
            , "expected a nonzero reference orbit seeded from the period-2 nucleus at the stencil corner"
        );
        let orbit_id = session.worker_state.seat_orbit_ids[0];
        assert_ne!(orbit_id, ZERO_ORBIT_ID);
        assert!(
            session.worker_state.seat_orbit_ids.iter().all(|&id| id == orbit_id)
            , "every seat must start bound to the same seeded reference orbit: \
               perturbation is always-on for the whole screen, not opt-in per seat"
        );
    }

    // r[verify cz.seamless.perturbation-always-on+1]
    #[test]
    fn new_session_falls_back_to_zero_orbit_when_corner_has_no_nucleus() {
        let location = ObjectivePosAndZoom {
            pos: (IntExp::from(3), IntExp::ZERO)
            , zoom_pot: 0
        };
        let session = TileSession::new(location, (4, 4));
        assert_eq!(session.worker_state.references.len(), 1);
        assert!(
            session.worker_state.seat_orbit_ids.iter().all(|&id| id == ZERO_ORBIT_ID)
            , "with no nucleus at the corner, every seat still goes through the \
               perturbation worker, just bound to the immortal zero orbit"
        );
    }

    // r[verify cz.seamless.perturbation-always-on+1]
    #[test]
    fn live_workshift_perturbation_matches_naive_near_period_two_nucleus() {
        // TileSession embeds a full GPU_WORKER_BATCH_N-sized PointBatch directly
        // (not boxed) whenever a tile or scredge batch is open, matching the
        // real actor's stack budget (main.rs sizes actor stacks at 100MiB via
        // with_default_actor_stack_size). Run this on a thread with a comparable
        // stack instead of the default ~2MiB test-thread stack.
        let handle = std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(live_workshift_matches_naive_body)
            .expect("spawn test thread");
        handle.join().expect("live workshift parity test thread panicked");
    }

    fn live_workshift_matches_naive_body() {
        // Corner chosen with a nonzero (but tiny) imaginary part on purpose:
        // PointStencil's own f64-availability check (type_contains_all_points)
        // degenerates whenever a homothety coordinate is exactly zero (adding
        // then subtracting zero trivially "loses no precision"), which is a
        // pre-existing quirk of docs/design geometry unrelated to perturbation.
        // Keeping the offset tiny (but at/above this stencil's own pixel
        // precision, so it survives PointStencil::correct_precision instead
        // of rounding back to zero) still lands essentially on the period-2
        // nucleus at c=-1, so period detection converges in a handful of
        // iterations instead of the slow brute-force search that happens
        // well inside (but off-center of) a hyperbolic component.
        let im = IntExp::from(1).shift(-8);
        let location = ObjectivePosAndZoom {
            pos: (IntExp::from(-1), IntExp::ZERO - im)
            , zoom_pot: 2
        };
        let res: (usize, usize) = (6, 6);
        let mut session = TileSession::new(location, (res.0 as u32, res.1 as u32));
        // Periodicity for this fixture is CPU-side; skip GPU bout round-trips so
        // the parity check stays interactive. Preference still selected at new().
        session.worker_state.use_cpu_bouts_only();
        assert_ne!(
            session.worker_state.seat_orbit_ids[0]
            , ZERO_ORBIT_ID
            , "fixture must exercise a real (nonzero) reference orbit, not the zero-orbit fallback"
        );
        let mut guard = 0;
        while session.percent_completed() < 100.0 {
            session.workshift();
            guard += 1;
            assert!(guard < 500, "live session did not complete work in time");
        }
        let generator = session.stencil.get_c_generator::<f64>().expect("f64 c generator");
        for y in 0..res.1 {
            for x in 0..res.0 {
                let idx = y * res.0 + x;
                let calibrated = session.screen_answer[idx]
                    .expect("every seat must be finished once the session reports 100%");
                let live_answer = calibrated_to_answer(calibrated);
                let c = generator.get_c((x as u16, y as u16));
                let naive_answer = naive_finish(c, 100_000);
                assert!(
                    same_membership(&live_answer, &naive_answer)
                    , "live perturbation path disagrees with naive iteration at seat ({x},{y}) c={c:?}: \
                       live={live_answer:?} naive={naive_answer:?}"
                );
            }
        }
    }

    // r[verify cz.seamless.foveated-mag-velocity+1]
    #[test]
    fn workshift_starts_lookahead_at_deeper_mag() {
        // LookaheadWork embeds large outfill state; use a fat stack like the
        // live workshift parity test.
        let handle = std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let location = ObjectivePosAndZoom {
                    pos: (IntExp::from(-1), IntExp::ZERO)
                    , zoom_pot: 2
                };
                let mut session = TileSession::new(location, (64, 64));
                session.set_mag_velocity(1);
                session.workshift();
                assert!(
                    session.lookahead_work.is_some()
                    , "zoom-in should begin the DFS lookahead column under attention"
                );
                let zoom = session.lookahead_work.as_ref().unwrap().publish_location.zoom_pot;
                assert_eq!(zoom, 3, "first column bump is base_mag+1");
            })
            .expect("spawn");
        handle.join().expect("lookahead start test panicked");
    }

    // r[verify cz.seamless.foveated-mag-velocity+1]
    #[test]
    fn attention_tile_location_contains_attention_seat() {
        let location = ObjectivePosAndZoom {
            pos: (IntExp::ZERO, IntExp::ZERO)
            , zoom_pot: 3
        };
        let attention = (40, 20);
        let deeper = attention_tile_location(&location, attention, 5).expect("loc");
        assert_eq!(deeper.zoom_pot, 5);
        let att = (
            location.pos.0.clone()
                + IntExp::from(attention.0).shift(-(location.zoom_pot + PIXELS_PER_UNIT_POT))
            , location.pos.1.clone()
                + IntExp::from(attention.1).shift(-(location.zoom_pot + PIXELS_PER_UNIT_POT))
        );
        let wx = intexp_seat_i32(att.0.shift(5).shift(PIXELS_PER_UNIT_POT)).unwrap();
        let wy = intexp_seat_i32(att.1.shift(5).shift(PIXELS_PER_UNIT_POT)).unwrap();
        let edge = TILE_EDGE_LENGTH as i32;
        let ox = floor_div_tile_i32(wx) * edge;
        let oy = floor_div_tile_i32(wy) * edge;
        let ulx = intexp_seat_i32(
            deeper.pos.0.clone().shift(5).shift(PIXELS_PER_UNIT_POT)
        ).unwrap();
        let uly = intexp_seat_i32(
            deeper.pos.1.clone().shift(5).shift(PIXELS_PER_UNIT_POT)
        ).unwrap();
        assert_eq!((ulx, uly), (ox, oy));
        assert!(wx >= ox && wx < ox + edge);
        assert!(wy >= oy && wy < oy + edge);
    }

    // r[verify cz.seamless.foveated-mag-velocity+1]
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(32))]
        #[test]
        fn lookahead_publish_location_zoom_matches_requested_mag(
            base_zoom in -4i32..8,
            bump in 1i32..8,
            att_x in 0i32..64,
            att_y in 0i32..64,
        ) {
            let location = ObjectivePosAndZoom {
                pos: (IntExp::ZERO, IntExp::ZERO)
                , zoom_pot: base_zoom
            };
            let target = base_zoom + bump;
            let Some(pub_loc) = attention_tile_location(&location, (att_x, att_y), target) else {
                return Ok(());
            };
            prop_assert_eq!(pub_loc.zoom_pot, target);
        }
    }

    // r[verify cz.seamless.reference-background+1]
    #[test]
    fn retarget_pan_keeps_bound_reference_orbit() {
        let location = ObjectivePosAndZoom {
            pos: (IntExp::from(-1), IntExp::ZERO)
            , zoom_pot: 4
        };
        let mut session = TileSession::new(location.clone(), (8, 8));
        let orbit_before = session.worker_state.seat_orbit_ids[0];
        let refs_before = session.worker_state.references.len();
        assert_ne!(orbit_before, ZERO_ORBIT_ID);
        let panned = ObjectivePosAndZoom {
            pos: (
                location.pos.0.clone() + IntExp::from(1).shift(-(4 + PIXELS_PER_UNIT_POT))
                , location.pos.1.clone()
            )
            , zoom_pot: 4
        };
        session.retarget(panned, (8, 8));
        assert_eq!(session.worker_state.seat_orbit_ids[0], orbit_before);
        assert_eq!(session.worker_state.references.len(), refs_before);
        assert_eq!(session.seats_done, 0);
    }

    // r[verify cz.seamless.reference-background+1]
    #[test]
    fn retarget_zoom_rebuilds_session() {
        let handle = std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let location = ObjectivePosAndZoom {
                    pos: (IntExp::from(-1), IntExp::ZERO)
                    , zoom_pot: 4
                };
                let mut session = TileSession::new(location, (8, 8));
                session.seats_done = 3;
                let refs_before = session.worker_state.references.len();
                session.retarget(
                    ObjectivePosAndZoom {
                        pos: (IntExp::from(-1), IntExp::ZERO)
                        , zoom_pot: 5
                    }
                    , (8, 8)
                );
                assert_eq!(session.location.zoom_pot, 5);
                assert_eq!(session.seats_done, 0);
                assert_eq!(session.tile_scheduler.mag_velocity, 1);
                assert_eq!(session.reference_worker.bound_mag(), Some(5));
                // Mag-change path keeps the prior collection (old orbit retained).
                assert!(session.worker_state.references.len() >= refs_before);
            })
            .expect("spawn");
        handle.join().expect("retarget zoom test panicked");
    }
}
