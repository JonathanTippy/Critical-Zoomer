use std::collections::{HashMap, HashSet};
use std::time::Instant;

use crate::assemblies::structs::*;
use crate::assemblies::workgroup::structs::*;
use crate::assemblies::workgroup::structs::mandelbrotable::*;
use crate::assemblies::workgroup::workcore::mandelbrot::*;
use crate::assemblies::workgroup::workcore::mandelbrot::scheduler_implementations::outfill_infill_scheduler::*;
use crate::assemblies::workgroup::workcore::mandelbrot::scheduler_implementations::tile_scheduler::*;
use crate::assemblies::workgroup::workcore::mandelbrot::worker_implementations::active_gear_work::ActiveGearWork;
use crate::assemblies::workgroup::workcore::mandelbrot::worker_implementations::perturbation_gpu_worker::*;
use crate::assemblies::workgroup::workcore::mandelbrot::worker_implementations::periodicity_detector::*;
use crate::assemblies::workgroup::workcore::mandelbrot::worker_implementations::reference_worker::ReferenceWorker;
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
    , batch: Option<ActiveGearWork<BATCH_N>>
}

struct ScredgeWork {
    seats: [Option<(usize, usize)>; BATCH_N]
    , tile: Tile<()>
    , batch: Option<ActiveGearWork<BATCH_N>>
}

struct LookaheadWork {
    publish_location: ObjectivePosAndZoom
    , tile: Tile<()>
    , answer_tile: Box<Tile<Answer>>
    , scheduler: OutfillInfillSchedulerState
    , batch: Option<ActiveGearWork<BATCH_N>>
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
    // Nanoseconds spent on current-stencil work (standards 50/50 foveation).
    // r[impl cz.perf.foveation-half-time+1]
    , screen_work_ns: u128
    // Nanoseconds spent on lookahead work.
    , lookahead_work_ns: u128
    // After retarget/gesture: prefer screen + republish until something is visible
    // again (standards 100ms play bar).
    , play_need_visible: bool
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
            , hover: None,
            mag_velocity: 0.0
        }.correct_precision();
        // Prefer GPU when available; always perturbation (no toggles).
        // Harness/Xvfb: CZ_FORCE_CPU_BOUTS=1 skips sync GPU readback that starves fill.
        let mut worker_state = if std::env::var("CZ_FORCE_CPU_BOUTS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
        {
            PerturbationGpuWorkerState::with_forced_path(PerturbationComputePath::Cpu)
        } else {
            PerturbationGpuWorkerState::prefer_available_gpu()
        };
        // #region agent log
        crate::assemblies::workgroup::debug_session::log(
            "H-GPU-PATH",
            "tile_session.rs:new",
            "worker_path_at_session_start",
            &format!(
                "{{\"path\":\"{:?}\",\"gpu_desired\":{},\"force_cpu_env\":{}}}",
                worker_state.path,
                worker_state.is_gpu_preferred(),
                std::env::var("CZ_FORCE_CPU_BOUTS").is_ok()
            ),
        );
        // #endregion
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
        worker_state.refresh_selected_gear();
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
            , screen_work_ns: 0
            , lookahead_work_ns: 0
            , play_need_visible: true
        }
    }

    pub fn set_attention(&mut self, attention: (i32, i32)) {
        self.attention = attention;
        TileScheduler::set_attention(&mut self.tile_scheduler, attention);
    }

    pub fn set_mag_velocity(&mut self, mag_velocity: i32) {
        TileScheduler::set_mag_velocity(&mut self.tile_scheduler, mag_velocity);
        // Mag-mode changes often arrive as a Retarget with unchanged location;
        // still demand a visible refresh within the play bar.
        self.touch_play_visible();
    }

    /// Mark that a gesture/retarget requires something publishable soon.
    /// Re-queues existing answer tiles so play is not starved by lookahead.
    fn touch_play_visible(&mut self) {
        self.play_need_visible = true;
        for origin in self.answer_tiles.keys().copied() {
            self.unsent_origins.insert(origin);
        }
        if self.has_unsent_publish() {
            self.play_need_visible = false;
        }
    }

    pub fn mag_velocity(&self) -> i32 {
        self.tile_scheduler.mag_velocity
    }

    pub fn force_cpu_bouts_for_test(&mut self) {
        self.worker_state.use_cpu_bouts_only();
    }

    pub fn set_iterations_per_bout_for_test(&mut self, bout: u32) {
        self.worker_state.cpu.iterations_per_bout = bout.max(1);
    }

    /// Full wipe of screen progress (no pan remap). Used by full-stack IPS bars so
    /// scheduling stays live for the whole timed window.
    pub fn work_once_for_ips_test(&mut self) -> bool { self.work_once() }
    pub fn seats_done_for_test(&self) -> usize { self.seats_done }
    pub fn seats_total_for_test(&self) -> usize { self.seats_total }
    pub fn has_active_tile_for_test(&self) -> bool { self.active_tile.is_some() }
    pub fn active_batch_open_count_for_test(&self) -> usize {
        let Some(work) = self.active_tile.as_ref() else { return 0 };
        let Some(batch) = work.batch.as_ref() else { return 0 };
        let host = batch.to_host_batch();
        host.points.iter().filter(|s| s.as_ref().map(|(_,p)| !p.finished).unwrap_or(false)).count()
    }
    pub fn active_batch_slot_count_for_test(&self) -> usize {
        let Some(work) = self.active_tile.as_ref() else { return 0 };
        let Some(batch) = work.batch.as_ref() else { return 0 };
        let host = batch.to_host_batch();
        host.points.iter().filter(|s| s.is_some()).count()
    }
    pub fn bound_orbit_for_test(&self) -> u32 { self.reference_worker.bound_orbit_id() }


    /// IPS bar helper: pack via the normal scheduler, then spend the budget in
    /// tight host/GPU bouts on the open batch (still the live worker path).

    /// Rebind open batch points to a fixed delta-c on the zero orbit (IPS fixtures).
    pub fn rebind_open_batch_c_for_ips_test(&mut self, c: (f64, f64)) {
        let rebind = |batch: &mut ActiveGearWork<BATCH_N>| {
            batch.with_host_mut(|host| {
                for slot in host.points.iter_mut() {
                    let Some((_, point)) = slot else { continue };
                    if point.finished { continue; }
                    point.orbit_id = ZERO_ORBIT_ID;
                    point.c = c;
                    point.z = (0.0, 0.0);
                    point.derivative = (1.0, 0.0);
                    point.real_squared = 0.0;
                    point.imag_squared = 0.0;
                    point.real_imag = 0.0;
                    point.iteration_count = 0;
                    point.min_magnitude = f64::MAX;
                    point.min_magnitude_time = 0;
                    point.periodicity_detector = CpuPeriodicityDetector::init(0, point.z, point.derivative);
                    point.escaped = false;
                    point.finished = false;
                }
            });
        };
        if let Some(work) = self.active_tile.as_mut() {
            if let Some(batch) = work.batch.as_mut() {
                rebind(batch);
            }
        }
        if let Some(work) = self.scredge_work.as_mut() {
            if let Some(batch) = work.batch.as_mut() {
                rebind(batch);
            }
        }
    }

    pub fn workshift_ips_burst_for_test(&mut self, budget_ms: u128) {
        let started = Instant::now();
        // Pack until we have an open batch or the screen is done.
        while started.elapsed().as_millis() < budget_ms {
            if self.seats_done >= self.seats_total {
                self.wipe_screen_progress_for_ips_test();
            }
            let has_batch = self
                .active_tile
                .as_ref()
                .map(|w| w.batch.is_some())
                .unwrap_or(false)
                || self
                    .scredge_work
                    .as_ref()
                    .map(|w| w.batch.is_some())
                    .unwrap_or(false);
            if has_batch {
                break;
            }
            if !self.work_once() {
                break;
            }
        }
        // Bout-heavy stretch on the live worker.
        while started.elapsed().as_millis() < budget_ms {
            let progressed = if let Some(work) = self.active_tile.as_mut() {
                if let Some(batch) = work.batch.as_mut() {
                    self.worker_state.workshift_active_gear(batch)
                } else {
                    false
                }
            } else if let Some(work) = self.scredge_work.as_mut() {
                if let Some(batch) = work.batch.as_mut() {
                    self.worker_state.workshift_active_gear(batch)
                } else {
                    false
                }
            } else {
                false
            };
            if !progressed {
                // Harvest / repack through the normal path.
                if !self.work_once() {
                    if self.percent_completed() >= 95.0 {
                        self.wipe_screen_progress_for_ips_test();
                    } else {
                        break;
                    }
                }
            }
        }
        self.workshifts = self.workshifts.wrapping_add(1);
    }

    pub fn wipe_screen_progress_for_ips_test(&mut self) {
        let res = self.screen_res;
        self.screen_done.fill(false);
        self.screen_kind.fill(None);
        self.screen_answer.fill(None);
        self.seats_done = 0;
        self.tile_scheduler = TileScheduler::init(res);
        TileScheduler::set_base_mag(&mut self.tile_scheduler, self.location.zoom_pot);
        TileScheduler::set_attention(&mut self.tile_scheduler, self.attention);
        let mag_velocity = self.mag_velocity();
        TileScheduler::set_mag_velocity(&mut self.tile_scheduler, mag_velocity);
        self.active_tile = None;
        self.scredge_work = None;
        self.lookahead_work = None;
        self.answer_tiles.clear();
        self.unsent_origins.clear();
        self.lookahead_unsent.clear();
        self.stencil.serial_number = self.stencil.serial_number.wrapping_add(1);
        self.worker_state.stencil = Some(self.stencil.clone());
    }

    pub fn worker_is_gpu_preferred(&self) -> bool {
        self.worker_state.is_gpu_preferred()
    }

    pub fn worker_gpu_device_held(&self) -> bool {
        self.worker_state.gpu_device_held()
    }

    /// Calibrated GPU point-iteration IPS from the submission budget, if any.
    pub fn gpu_observed_ips(&self) -> Option<f64> {
        if !self.worker_state.budget.is_calibrated() {
            return None;
        }
        Some(self.worker_state.budget.estimated_ips())
    }

    pub fn skip_lookahead_column_for_test(&mut self) {
        self.tile_scheduler.lookahead_column_done = true;
        self.tile_scheduler.lookahead_bump = 8;
    }

    /// Standards foveation: cumulative work time on current stencil vs lookahead.
    pub fn foveation_work_ns(&self) -> (u128, u128) {
        (self.screen_work_ns, self.lookahead_work_ns)
    }

    fn prefer_screen_half(&self) -> bool {
        // r[impl cz.perf.foveation-half-time+1]
        // r[impl cz.perf.play-8bump-100ms+1]
        // Play: until *some* current-stencil work exists, do not spend the
        // quantum on lookahead setup — first paint must land inside 100ms.
        if self.play_need_visible || self.seats_done == 0 {
            return true;
        }
        // Balance wall time only when both halves can accept work; otherwise
        // stay on the current stencil (stationary / zoom-out).
        if !self.lookahead_half_eligible() {
            return true;
        }
        self.screen_work_ns <= self.lookahead_work_ns
    }

    fn lookahead_half_eligible(&self) -> bool {
        if self.lookahead_work.is_some() {
            return true;
        }
        let mag = self.tile_scheduler.mag_velocity;
        if mag < 0 || self.tile_scheduler.lookahead_column_done {
            return false;
        }
        if mag == 0 {
            let screen_pending = self.tile_scheduler.screen_work_pending();
            if screen_pending {
                return false;
            }
        }
        self.tile_scheduler.lookahead_bump < 8
    }

    fn work_screen_half(&mut self) -> bool {
        if self.try_active_tile_step() {
            return true;
        }
        if self.try_resolve_periods() {
            return true;
        }
        // Stuck active tile: mark finished for scheduling so foveated next()
        // can begin never-started tiles (releasing back to unbegun livelocks
        // on the attention neighborhood). Idle reopen_incomplete retries holes.
        if self.active_tile.is_some() {
            let tile_index = self.active_tile.as_ref().unwrap().tile_index;
            self.active_tile = None;
            TileScheduler::note_tile_finished(&mut self.tile_scheduler, tile_index);
        }
        // Under zoom-in, screen half may begin tiles even while the lookahead
        // column would otherwise monopolize TileScheduler::next.
        if self.tile_scheduler.mag_velocity >= 0 {
            if let Some(tile_index) = TileScheduler::claim_next_screen_tile(&mut self.tile_scheduler) {
                self.begin_tile(tile_index);
                // Play: chain arming seats in the same step — setup-only returns
                // are pure play and miss the 100ms first-paint bar.
                let _ = self.try_active_tile_step();
                if self.seats_done == 0 {
                    let _ = self.advance_open_batch();
                }
                return true;
            }
        }
        false
    }

    fn work_lookahead_half(&mut self) -> bool {
        if self.advance_lookahead_work() {
            return true;
        }
        // Mag-velocity policy (inside the lookahead half):
        // zoom-in opens the column immediately; stationary waits until screen
        // tiles/scredge are claimed; zoom-out skips lookahead.
        let mag = self.tile_scheduler.mag_velocity;
        if mag < 0 {
            return false;
        }
        if mag == 0 {
            let screen_pending = self.tile_scheduler.screen_work_pending();
            if screen_pending {
                return false;
            }
        }
        if let Some(zoom_pot) = TileScheduler::claim_next_lookahead_zoom(&mut self.tile_scheduler) {
            self.begin_lookahead(zoom_pot);
            return true;
        }
        false
    }

    pub fn has_active_tile(&self) -> bool {
        self.active_tile.is_some()
    }

    pub fn has_open_lookahead(&self) -> bool {
        self.lookahead_work.is_some()
    }

    pub fn open_lookahead_zoom(&self) -> Option<i32> {
        self.lookahead_work
            .as_ref()
            .map(|w| w.publish_location.zoom_pot)
    }

    pub fn reference_bound_mag(&self) -> Option<i32> {
        Some(self.location.zoom_pot)
    }

    pub fn bound_orbit_id_for_test(&self) -> u32 {
        self.worker_state
            .seat_orbit_ids
            .first()
            .copied()
            .unwrap_or(0)
    }

    /// Retarget an existing session to a new stencil without always wiping
    /// reference orbits. Same-mag pans keep the bound references (design updates
    /// refs on mag change); zoom/res changes rebuild screen state and notify the
    /// reference worker so the old orbit stays until the new one is ready.
    // r[impl cz.int.stencil-retarget+1]
    // r[impl cz.int.session-pipeline+1]
    pub fn retarget(&mut self, location: ObjectivePosAndZoom, screen_res: (u32, u32)) {
        let new_res = (screen_res.0 as usize, screen_res.1 as usize);
        if new_res != self.screen_res {
            let attention = self.attention;
            *self = Self::new(location, screen_res);
            self.set_attention(attention);
            // Caller sets mag_velocity from EWMA mode after retarget.
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
                , hover: None,
            mag_velocity: 0.0
            }.correct_precision();
            worker_state.stencil = Some(stencil.clone());
            worker_state.screen_width = res.0;
            worker_state.seat_orbit_ids = vec![new_bound; res.0 * res.1];
            worker_state.refresh_selected_gear();
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
                , screen_work_ns: 0
                , lookahead_work_ns: 0
                , play_need_visible: true
            });
            *self = *rebuilt;
            // Caller sets mag_velocity from EWMA mode after retarget.
            return;
        }
        if self.location == location {
            // No geometry change (often mag-mode-only Retarget) — still demand a
            // visible refresh so lookahead cannot starve the play bar.
            self.touch_play_visible();
            return;
        }
        // Same-mag pan: remap screen-relative progress instead of wiping the hoard buffer.
        if let Some((dx, dy)) = same_mag_seat_delta(&self.location, &location) {
            self.remap_same_mag_pan(location, dx, dy);
            self.touch_play_visible();
            return;
        }
        // Non-integer seat delta — rebuild screen desire; headgroup absolute tiles stay.
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
            , hover: None,
            mag_velocity: 0.0
        }.correct_precision();
        self.worker_state.stencil = Some(self.stencil.clone());
        self.worker_state.screen_width = res.0;
        if self.worker_state.refresh_selected_gear() {
            self.active_tile = None;
            self.scredge_work = None;
            self.lookahead_work = None;
        }
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
        self.play_need_visible = true;
        // Caller sets mag_velocity from EWMA mode after retarget.
    }

    /// Integer seat delta of `new` relative to `old` at the same magnification.
    fn remap_same_mag_pan(&mut self, location: ObjectivePosAndZoom, dx: i32, dy: i32) {
        let w = self.screen_res.0;
        let h = self.screen_res.1;
        let n = w * h;
        let mut new_done = vec![false; n];
        let mut new_kind = vec![None; n];
        let mut new_answer = vec![None; n];
        let mut seats_done = 0usize;
        let wi = w as i32;
        let hi = h as i32;
        for y in 0..hi {
            for x in 0..wi {
                let ox = x + dx;
                let oy = y + dy;
                if ox < 0 || oy < 0 || ox >= wi || oy >= hi {
                    continue;
                }
                let old_i = (oy as usize) * w + (ox as usize);
                let new_i = (y as usize) * w + (x as usize);
                new_done[new_i] = self.screen_done[old_i];
                new_kind[new_i] = self.screen_kind[old_i];
                new_answer[new_i] = self.screen_answer[old_i];
                if new_done[new_i] {
                    seats_done += 1;
                }
            }
        }
        let mut new_tiles = HashMap::new();
        let mut new_unsent = HashSet::new();
        let old_unsent = std::mem::take(&mut self.unsent_origins);
        for ((ox, oy), mut tile) in std::mem::take(&mut self.answer_tiles) {
            let nx = ox as i32 - dx;
            let ny = oy as i32 - dy;
            if nx + (TILE_EDGE_LENGTH as i32) <= 0 || ny + (TILE_EDGE_LENGTH as i32) <= 0 {
                continue;
            }
            if nx >= wi || ny >= hi {
                continue;
            }
            if nx < 0 || ny < 0 {
                // Origin left the screen; drop screen-relative key (headgroup keeps absolute).
                continue;
            }
            let new_origin = (nx as usize, ny as usize);
            tile.origin_seat = new_origin;
            if old_unsent.contains(&(ox, oy)) {
                new_unsent.insert(new_origin);
            }
            new_tiles.insert(new_origin, tile);
        }
        self.location = location.clone();
        self.stencil = PointStencil {
            homothety: (
                location.pos.0.clone()
                , IntExp::ZERO - location.pos.1.clone()
                , location.zoom_pot
            )
            , resolution: (w, h)
            , serial_number: self.stencil.serial_number.wrapping_add(1)
            , focus: None
            , hover: None
            , mag_velocity: 0.0
        }.correct_precision();
        self.worker_state.stencil = Some(self.stencil.clone());
        self.worker_state.screen_width = w;
        if self.worker_state.refresh_selected_gear() {
            // Gear identity changed with the new reference/stencil — drop typed batches.
            self.active_tile = None;
            self.scredge_work = None;
            self.lookahead_work = None;
        }
        self.screen_done = new_done;
        self.screen_kind = new_kind;
        self.screen_answer = new_answer;
        self.seats_done = seats_done;
        self.answer_tiles = new_tiles;
        self.unsent_origins = new_unsent;
        self.active_tile = None;
        self.scredge_work = None;
        self.lookahead_work = None;
        self.lookahead_unsent.clear();
        self.tile_scheduler = TileScheduler::init((w, h));
        TileScheduler::set_base_mag(&mut self.tile_scheduler, location.zoom_pot);
        TileScheduler::set_attention(&mut self.tile_scheduler, self.attention);
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                if !self.screen_done[i] {
                    continue;
                }
                let kind = match self.screen_kind[i] {
                    Some(SeatKind::Outside) => TileSeatKind::Outside
                    , Some(SeatKind::Inside { .. }) => TileSeatKind::Inside
                    , None => TileSeatKind::Outside
                };
                TileScheduler::note_finished(&mut self.tile_scheduler, (x, y), kind);
            }
        }
        TileScheduler::seed_completed_tiles(
            &mut self.tile_scheduler
            , &self.screen_done
            , (w, h)
        );
    }

    pub fn percent_completed(&self) -> f64 {
        if self.seats_total == 0 {
            return 100.0;
        }
        (self.seats_done as f64) * 100.0 / (self.seats_total as f64)
    }

    /// Point-iterations advanced (CPU + GPU counters) for full-stack IPS bars.
    pub fn iterations_advanced(&self) -> u64 {
        self.worker_state
            .iterations_advanced
            .max(self.worker_state.cpu.iterations_advanced)
    }

    pub fn workshift(&mut self) {
        // Play: short quanta when seats_done==0 or zooming. Stationary fill:
        // long quanta so home view can hit ≥100 completed-whole TPS (standards).
        let budget_ms: u128 = if self.tile_scheduler.mag_velocity != 0 {
            if self.seats_done == 0 {
                8
            } else {
                16
            }
        } else if self.seats_done == 0 {
            8
        } else if self.percent_completed() < 95.0 {
            48
        } else {
            16
        };
        self.workshift_budget_ms(budget_ms);
    }

    /// Bounded work quantum so a hosting actor can re-check inputs ≈1000Hz.
    pub fn workshift_budget_ms(&mut self, budget_ms: u128) {
        let started = Instant::now();
        // Play (≤2ms): one step so Retarget is re-checked immediately.
        // Throughput: allow roughly one step per ms of budget (cap 64).
        let max_steps: u32 = if budget_ms <= 2 {
            1
        } else {
            // Stationary fill: more steps per quantum — each step is cheap after
            // pop_matching removal; the old 64 cap left idle budget on the table.
            let cap = if self.tile_scheduler.mag_velocity == 0 { 256 } else { 64 };
            (budget_ms as u32).saturating_mul(4).clamp(1, cap)
        };
        // #region agent log
        let mut steps = 0u32;
        // #endregion
        while started.elapsed().as_millis() < budget_ms && steps < max_steps {
            if self.seats_done >= self.seats_total {
                break;
            }
            // Play: once a batch is armed and nothing is finished yet, spend the
            // rest of the quantum finishing seats — not beginning more tiles.
            let has_batch = self
                .active_tile
                .as_ref()
                .map(|w| w.batch.is_some())
                .unwrap_or(false)
                || self
                    .scredge_work
                    .as_ref()
                    .map(|w| w.batch.is_some())
                    .unwrap_or(false);
            let progressed = if self.seats_done == 0 && has_batch {
                self.advance_open_batch()
            } else {
                self.work_once()
            };
            if !progressed {
                if self.seats_done == 0 && has_batch {
                    // Batch not finishing — fall through to normal work_once.
                    if !self.work_once() {
                        break;
                    }
                } else {
                    break;
                }
            }
            // #region agent log
            steps += 1;
            // #endregion
        }
        // #region agent log
        {
            let n = crate::assemblies::workgroup::debug_session::pub_tick();
            // Throughput debug: denser samples while screen still filling.
            let sample = n <= 24 || n % 8 == 0 || self.percent_completed() < 96.0 && n % 4 == 0;
            if sample {
                let ms = started.elapsed().as_secs_f64() * 1000.0;
                let whole = self
                    .answer_tiles
                    .values()
                    .filter(|t| {
                        t.data.iter().filter(|c| c.is_some()).count() == TILE_SEAT_COUNT
                    })
                    .count();
                let (scr_ns, look_ns) = (self.screen_work_ns, self.lookahead_work_ns);
                crate::assemblies::workgroup::debug_session::log(
                    "H-FILL-TPS",
                    "tile_session.rs:workshift",
                    "workshift_quantum",
                    &format!(
                        "{{\"n\":{n},\"budget_ms\":{budget_ms},\"elapsed_ms\":{ms:.3},\"steps\":{steps},\"pct\":{:.2},\"seats_done\":{},\"whole\":{whole},\"scr_ms\":{:.2},\"look_ms\":{:.2},\"iters\":{},\"mag_v\":{},\"path_cpu\":{}}}",
                        self.percent_completed(),
                        self.seats_done,
                        scr_ns as f64 / 1e6,
                        look_ns as f64 / 1e6,
                        self.worker_state.cpu.iterations_advanced,
                        self.tile_scheduler.mag_velocity,
                        !self.worker_state.is_gpu_preferred()
                    ),
                );
            }
        }
        // #endregion
        self.workshifts = self.workshifts.wrapping_add(1);
        if std::env::var("CZ_DEBUG_FILL")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
            && self.workshifts % 20 == 1
        {
            let (begun, finished, unbegun) = self.tile_scheduler.debug_tile_counts();
            let (scredge_q, scredge_act) = self.tile_scheduler.debug_scredge_lens();
            let mut nores = 0u32;
            let mut none_seats = 0u32;
            let mut filled_seats = 0u32;
            for ((ox, oy), tile) in &self.answer_tiles {
                for ly in 0..TILE_EDGE_LENGTH {
                    for lx in 0..TILE_EDGE_LENGTH {
                        let sx = ox + lx;
                        let sy = oy + ly;
                        if sx >= self.screen_res.0 || sy >= self.screen_res.1 {
                            continue;
                        }
                        match tile.get((lx, ly)) {
                            None => none_seats += 1,
                            Some(a) => {
                                filled_seats += 1;
                                if let MandelbrotResult::Outside { escape_time_r2, .. } = a.result {
                                    if escape_time_r2 == 1 && a.min_magnitude.is_infinite() {
                                        nores += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            let _ = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open("/tmp/cz_fill_debug.log")
                .and_then(|mut file| {
                    use std::io::Write;
                    writeln!(
                        file
                        , "ws={} pct={:.2} seats={}/{} active={} scredge_work={} look={} begun={} fin={} unbegun={} scredge_q={} scredge_act={} answer_tiles={} unsent={} filled={} none={} nores={}"
                        , self.workshifts
                        , self.percent_completed()
                        , self.seats_done
                        , self.seats_total
                        , self.active_tile.is_some()
                        , self.scredge_work.is_some()
                        , self.lookahead_work.is_some()
                        , begun
                        , finished
                        , unbegun
                        , scredge_q
                        , scredge_act
                        , self.answer_tiles.len()
                        , self.unsent_origins.len()
                        , filled_seats
                        , none_seats
                        , nores
                    )
                });
        }
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

    /// Republish every answer tile (used once screen seats are complete so the
    /// headed path cannot strand a half-uploaded hoard).
    pub fn drain_all_answer_tiles(&mut self) -> Vec<Tile<Answer>> {
        self.unsent_origins.clear();
        self.answer_tiles.values().copied().collect()
    }

    /// Lookahead tiles carry their own location (deeper mag under attention).
    pub fn drain_lookahead_publishes(&mut self) -> Vec<(ObjectivePosAndZoom, Box<Tile<Answer>>)> {
        std::mem::take(&mut self.lookahead_unsent)
    }

    pub fn has_unsent_publish(&self) -> bool {
        !self.unsent_origins.is_empty() || !self.lookahead_unsent.is_empty()
    }

    /// Apply shared tile-manager prune; returns bump size when protected tiles exceed limit.
    // r[impl cz.int.memory-bump+1]
    pub fn prune_for_memory(&mut self, memory_limit_bytes: usize) -> Option<usize> {
        use crate::assemblies::workgroup::tile_manager::{
            apply_memory_bump, plan_prunes, required_limit_bump, ManagedTileMeta, TileKeepClass,
        };

        let z = self.location.zoom_pot;
        let tile_bytes = std::mem::size_of::<Tile<Answer>>().max(TILE_SEAT_COUNT);
        let mut meta = HashMap::new();
        let mut used_bytes = 0usize;
        for ((ox, oy), _tile) in &self.answer_tiles {
            let key = (z, *ox as i32, *oy as i32);
            meta.insert(
                key,
                ManagedTileMeta {
                    keep: TileKeepClass::CurrentStencil,
                    bytes: tile_bytes,
                },
            );
            used_bytes = used_bytes.saturating_add(tile_bytes);
        }
        if let Some(la) = &self.lookahead_work {
            let key = (
                la.publish_location.zoom_pot,
                la.tile.origin_seat.0 as i32,
                la.tile.origin_seat.1 as i32,
            );
            meta.insert(
                key,
                ManagedTileMeta {
                    keep: TileKeepClass::Lookahead,
                    bytes: tile_bytes,
                },
            );
            used_bytes = used_bytes.saturating_add(tile_bytes);
        }
        for (loc, tile) in &self.lookahead_unsent {
            let key = (
                loc.zoom_pot,
                tile.origin_seat.0 as i32,
                tile.origin_seat.1 as i32,
            );
            meta.insert(
                key,
                ManagedTileMeta {
                    keep: TileKeepClass::Lookahead,
                    bytes: tile_bytes,
                },
            );
            used_bytes = used_bytes.saturating_add(tile_bytes);
        }

        let bump = required_limit_bump(&meta, memory_limit_bytes);
        let limit = match bump {
            Some(needed) => apply_memory_bump(memory_limit_bytes, needed),
            None => memory_limit_bytes,
        };
        for key in plan_prunes(&meta, limit, used_bytes) {
            // Never prune current/lookahead (plan_prunes already skips them).
            let origin = (key.1 as usize, key.2 as usize);
            if key.0 == z {
                self.answer_tiles.remove(&origin);
                self.unsent_origins.remove(&origin);
            }
        }
        bump
    }

    fn work_once(&mut self) -> bool {
        // Play bar: starve lookahead only until *some* current-stencil progress
        // exists. Clearing here restores foveation half/half once seats move.
        if self.play_need_visible && (self.seats_done > 0 || self.has_unsent_publish()) {
            self.play_need_visible = false;
        }
        if self.seats_done >= self.seats_total {
            // Screen seats finished — drop leftover active/scredge so we do not
            // spin on a begun-but-scheduler-finished tile forever under headed pacing.
            self.active_tile = None;
            self.scredge_work = None;
            return false;
        }
        // Period resolve before bout progress: an open Inside batch can otherwise
        // monopolize advance_open_batch and starve flood-in forever.
        if self.active_tile.is_some()
            && OutfillInfillScheduler::needs_period_resolve(
                &self.active_tile.as_ref().unwrap().scheduler
            )
            && self.try_resolve_periods()
        {
            return true;
        }
        // Standards: dedicate half working time to current stencil, half to lookahead.
        // Mag-velocity still shapes *what* each half does (via TileScheduler::next).
        let prefer_screen = self.prefer_screen_half();
        let open_is_lookahead = self.lookahead_work.is_some()
            && self.active_tile.is_none()
            && self.scredge_work.is_none();
        // Stationary/zoom-out: always drain the open batch (home-fill path).
        // Zoom-in: do not let the ahead half's open batch starve the behind half.
        let advance_open = if self.tile_scheduler.mag_velocity > 0 {
            if open_is_lookahead {
                !prefer_screen
            } else {
                prefer_screen || self.lookahead_work.is_none()
            }
        } else {
            true
        };
        if advance_open {
            let t_open = Instant::now();
            if self.advance_open_batch() {
                let ns = t_open.elapsed().as_nanos().max(1);
                if open_is_lookahead {
                    self.lookahead_work_ns = self.lookahead_work_ns.saturating_add(ns);
                } else {
                    self.screen_work_ns = self.screen_work_ns.saturating_add(ns);
                }
                // Stationary fill: immediately arm the next batch so one work_once
                // covers finish→arm (was two steps) and home whole-TPS can climb.
                if self.tile_scheduler.mag_velocity == 0
                    && self
                        .active_tile
                        .as_ref()
                        .map(|w| w.batch.is_none())
                        .unwrap_or(false)
                {
                    let t_arm = Instant::now();
                    if self.try_active_tile_step() {
                        self.screen_work_ns = self
                            .screen_work_ns
                            .saturating_add(t_arm.elapsed().as_nanos().max(1));
                    }
                }
                return true;
            }
        }
        let t0 = Instant::now();
        let did = if prefer_screen {
            self.work_screen_half()
        } else {
            self.work_lookahead_half()
        };
        let ns = t0.elapsed().as_nanos();
        if did {
            if prefer_screen {
                self.screen_work_ns = self.screen_work_ns.saturating_add(ns);
            } else {
                self.lookahead_work_ns = self.lookahead_work_ns.saturating_add(ns);
            }
            return true;
        }
        // Preferred half idle — try the other half before scheduler next.
        let t1 = Instant::now();
        let did_other = if prefer_screen {
            self.work_lookahead_half()
        } else {
            self.work_screen_half()
        };
        let ns_other = t1.elapsed().as_nanos();
        if did_other {
            if prefer_screen {
                self.lookahead_work_ns = self.lookahead_work_ns.saturating_add(ns_other);
            } else {
                self.screen_work_ns = self.screen_work_ns.saturating_add(ns_other);
            }
            return true;
        }
        match TileScheduler::next(&mut self.tile_scheduler) {
            TileSchedulerNext::Scredge(seat) => {
                let origin = tile_origin_for_seat(seat, self.screen_res);
                let tile = Tile::new(origin, self.location.zoom_pot);
                let mut screen_seats: [Option<(usize, usize)>; BATCH_N] = [const { None }; BATCH_N];
                let mut locals: [Option<(usize, usize)>; BATCH_N] = [const { None }; BATCH_N];
                screen_seats[0] = Some(seat);
                locals[0] = Some(tile.local_seat(seat).unwrap_or((0, 0)));
                // GPU path must keep ≥1024-wide parallelism (GPU_WORKER_BATCH_N).
                let mut packed = 1usize;
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
                    packed += 1;
                }
                // #region agent log
                {
                    let n = crate::assemblies::workgroup::debug_session::rpc_tick();
                    if crate::assemblies::workgroup::debug_session::should_sample(n) {
                        crate::assemblies::workgroup::debug_session::log(
                            "H-GPU-WIDTH",
                            "tile_session.rs:scredge",
                            "scredge_pack",
                            &format!(
                                "{{\"n\":{n},\"packed\":{packed},\"batch_n\":{BATCH_N}}}"
                            ),
                        );
                    }
                }
                // #endregion
                let batch = ActiveGearWork::initialize(
                    self.worker_state.cpu.gear
                    , &self.worker_state
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
            TileSchedulerNext::Idle => {
                // In-flight scredge seats with no open batch → requeue or we stall.
                if self.scredge_work.is_none()
                    && TileScheduler::reclaim_orphaned_scredge_active(
                        &mut self.tile_scheduler
                    )
                {
                    return true;
                }
                // Tiles marked finished/begun with remaining NORES seats (e.g. prior
                // abandon) must be reopened or the spiral idles forever mid-viewport.
                if self.seats_done < self.seats_total
                    && TileScheduler::reopen_incomplete_tiles(
                        &mut self.tile_scheduler
                        , &self.screen_done
                        , self.screen_res.0
                    )
                {
                    return true;
                }
                false
            }
        }
    }

    /// One outfill step on the active screen tile, or false if none / gated.
    fn try_active_tile_step(&mut self) -> bool {
        if self.active_tile.is_none() {
            return false;
        }
        let seats = {
            let work = self.active_tile.as_mut().unwrap();
            // #region agent log
            let t_get = Instant::now();
            // #endregion
            let seats = OutfillInfillScheduler::get_next_n_seats::<BATCH_N>(
                &mut work.scheduler
                , &mut work.tile
            );
            // #region agent log
            {
                let n = crate::assemblies::workgroup::debug_session::rpc_tick();
                if n <= 24 || n % 32 == 0 {
                    let armed = seats.iter().filter(|s| s.is_some()).count();
                    crate::assemblies::workgroup::debug_session::log(
                        "H-GET-NEXT",
                        "tile_session.rs:active",
                        "get_next_n_seats",
                        &format!(
                            "{{\"n\":{n},\"armed\":{armed},\"get_ms\":{:.3}}}",
                            t_get.elapsed().as_secs_f64() * 1000.0
                        ),
                    );
                }
            }
            // #endregion
            seats
        };
        // #region agent log
        {
            let n = crate::assemblies::workgroup::debug_session::rpc_tick();
            if crate::assemblies::workgroup::debug_session::should_sample(n) {
                let armed = seats.iter().filter(|s| s.is_some()).count();
                crate::assemblies::workgroup::debug_session::log(
                    "H-GPU-WIDTH",
                    "tile_session.rs:active",
                    "active_pack",
                    &format!("{{\"n\":{n},\"armed\":{armed},\"batch_n\":{BATCH_N}}}"),
                );
            }
        }
        // #endregion
        if seats.iter().all(|s| s.is_none()) {
            let (still_has_work, batch_open) = {
                let work = self.active_tile.as_ref().unwrap();
                (
                    OutfillInfillScheduler::has_work(&work.scheduler)
                    , work.batch.is_some()
                )
            };
            if still_has_work {
                // Batch cleared but seats left in `active` → get_next skips them.
                // Reclaim so the tile can continue instead of freezing the spiral.
                if !batch_open {
                    let reclaimed = {
                        let work = self.active_tile.as_mut().unwrap();
                        OutfillInfillScheduler::reclaim_orphaned_active(&mut work.scheduler)
                    };
                    if reclaimed {
                        return true;
                    }
                    // Edge-remaining can desync from done[] (absorb + live finish).
                    // Force queues open instead of abandoning (abandon left permanent
                    // NORES holes while note_tile_finished blocked re-BeginTile).
                    {
                        let work = self.active_tile.as_mut().unwrap();
                        OutfillInfillScheduler::force_progress(&mut work.scheduler);
                    }
                    let seats = {
                        let work = self.active_tile.as_mut().unwrap();
                        OutfillInfillScheduler::get_next_n_seats::<BATCH_N>(
                            &mut work.scheduler
                            , &mut work.tile
                        )
                    };
                    if seats.iter().any(|s| s.is_some()) {
                        return self.apply_active_seats(seats);
                    }
                    // Period resolve is has_work but not get_next — resolve now and
                    // re-seed queues so get_next can proceed (do not spin empty).
                    if OutfillInfillScheduler::needs_period_resolve(
                        &self.active_tile.as_ref().unwrap().scheduler
                    ) {
                        if self.try_resolve_periods() {
                            let seats = {
                                let work = self.active_tile.as_mut().unwrap();
                                OutfillInfillScheduler::get_next_n_seats::<BATCH_N>(
                                    &mut work.scheduler
                                    , &mut work.tile
                                )
                            };
                            if seats.iter().any(|s| s.is_some()) {
                                return self.apply_active_seats(seats);
                            }
                            return true;
                        }
                        return false;
                    }
                    // Scheduler still claims work but offers no seats: finish for
                    // scheduling so never-begun tiles can start; Idle reopen retries.
                    if OutfillInfillScheduler::has_work(
                        &self.active_tile.as_ref().unwrap().scheduler
                    ) {
                        let tile_index = self.active_tile.as_ref().unwrap().tile_index;
                        self.active_tile = None;
                        TileScheduler::note_tile_finished(
                            &mut self.tile_scheduler
                            , tile_index
                        );
                        return true;
                    }
                    // No work left in the outfill scheduler — tile is done.
                    let tile_index = self.active_tile.as_ref().unwrap().tile_index;
                    self.active_tile = None;
                    TileScheduler::note_tile_finished(&mut self.tile_scheduler, tile_index);
                    return true;
                }
                // Batch still open — wait for advance_open_batch.
                return false;
            }
            let tile_index = self.active_tile.as_ref().unwrap().tile_index;
            self.active_tile = None;
            TileScheduler::note_tile_finished(&mut self.tile_scheduler, tile_index);
            return true;
        }
        self.apply_active_seats(seats)
    }

    fn apply_active_seats(
        &mut self
        , seats: [Option<((usize, usize), Option<CalibratedAnswer>)>; BATCH_N]
    ) -> bool {
        let mut need_work: [Option<(usize, usize)>; BATCH_N] = [const { None }; BATCH_N];
        let mut hint_updates: [Option<((usize, usize), CalibratedAnswer)>; BATCH_N] =
            [const { None }; BATCH_N];
        let mut any = false;
        let mut any_outside = false;
        let mut tile_index = 0usize;
        for i in 0..BATCH_N {
            let Some((local, hint)) = seats[i] else { continue };
            any = true;
            if let Some(hint_answer) = hint {
                let screen = self.active_tile.as_ref().unwrap().tile.screen_seat(local);
                tile_index = self.active_tile.as_ref().unwrap().tile_index;
                self.finish_screen_seat(screen, hint_answer);
                hint_updates[i] = Some((local, hint_answer));
                if matches!(
                    seat_kind_from_calibrated(&hint_answer)
                    , SeatKind::Outside
                ) {
                    any_outside = true;
                }
            } else {
                need_work[i] = Some(local);
            }
        }
        if hint_updates.iter().any(|u| u.is_some()) {
            let work = self.active_tile.as_mut().unwrap();
            OutfillInfillScheduler::update(
                &mut work.scheduler
                , &mut work.tile
                , &hint_updates
            );
            if any_outside {
                TileScheduler::note_tile_has_outside(
                    &mut self.tile_scheduler
                    , tile_index
                );
            }
        }
        if need_work.iter().any(|s| s.is_some()) {
            // #region agent log
            let t_init = Instant::now();
            let armed = need_work.iter().filter(|s| s.is_some()).count();
            // #endregion
            let work = self.active_tile.as_mut().unwrap();
            let batch = ActiveGearWork::initialize(
                self.worker_state.cpu.gear
                , &self.worker_state
                , &work.tile
                , need_work
            );
            // #region agent log
            {
                let n = crate::assemblies::workgroup::debug_session::rpc_tick();
                if n <= 24 || n % 32 == 0 {
                    crate::assemblies::workgroup::debug_session::log(
                        "H-INIT-BATCH",
                        "tile_session.rs:apply_active",
                        "initialize_batch",
                        &format!(
                            "{{\"n\":{n},\"armed\":{armed},\"init_ms\":{:.3}}}",
                            t_init.elapsed().as_secs_f64() * 1000.0
                        ),
                    );
                }
            }
            // #endregion
            work.batch = Some(batch);
            return true;
        }
        any
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
                    let still_working = self.worker_state.workshift_active_gear(batch);
                    let peeked = batch.peek(&work.tile);
                    let mut finishes = Vec::new();
                    let mut any_finished = false;
                    for i in 0..BATCH_N {
                        let Some((_local, answer)) = peeked[i] else { continue };
                        let Some(seat) = work.seats[i] else { continue };
                        any_finished = true;
                        finishes.push((seat, answer));
                        work.seats[i] = None;
                        batch.clear_slot(i);
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
                // Seats still marked in this batch (init miss / never peeked)
                // must leave scredge_active or take_scredge rotates forever and
                // next() used to BeginTile over an undrained perimeter.
                if let Some(work) = self.scredge_work.as_ref() {
                    for seat in work.seats.iter().flatten().copied() {
                        TileScheduler::release_scredge_active(
                            &mut self.tile_scheduler
                            , seat
                        );
                    }
                }
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
            , wip_updates
        ) = {
            let Some(work) = self.active_tile.as_mut() else {
                return false;
            };
            let Some(batch) = work.batch.as_mut() else {
                return false;
            };
            let still_working = self.worker_state.workshift_active_gear(batch);
            // #region agent log
            let t_after_bout = Instant::now();
            // #endregion
            let peeked = batch.peek(&work.tile);
            let mut updates: [Option<((usize, usize), CalibratedAnswer)>; BATCH_N] =
                [const { None }; BATCH_N];
            let mut finishes = Vec::new();
            let mut wip_updates = Vec::new();
            let mut any_outside = false;
            let mut any_finished = false;
            for i in 0..BATCH_N {
                let Some((local, answer)) = peeked[i] else { continue };
                // Progressive Agnostic WIP must not clear the open seat.
                if matches!(answer.result, CalibratedMandelbrotResult::Agnostic { .. }) {
                    let screen = work.tile.screen_seat(local);
                    wip_updates.push((screen, answer));
                    continue;
                }
                any_finished = true;
                updates[i] = Some((local, answer));
                let screen = work.tile.screen_seat(local);
                if matches!(seat_kind_from_calibrated(&answer), SeatKind::Outside) {
                    any_outside = true;
                }
                finishes.push((screen, answer));
                batch.clear_slot(i);
            }
            // #region agent log
            let t_after_peek = Instant::now();
            // #endregion
            if !any_finished {
                if !still_working {
                    // Empty batch or all slots None: seats may still sit in `active`.
                    OutfillInfillScheduler::reclaim_orphaned_active(&mut work.scheduler);
                    work.batch = None;
                }
                (Vec::new(), false, work.tile_index, still_working, wip_updates)
            } else {
            OutfillInfillScheduler::update(
                &mut work.scheduler
                , &mut work.tile
                , &updates
            );
            // #region agent log
            let update_ms = t_after_peek.elapsed().as_secs_f64() * 1000.0;
            let n = crate::assemblies::workgroup::debug_session::rpc_tick();
            if n <= 24 || n % 16 == 0 {
                let fin_n = finishes.len();
                crate::assemblies::workgroup::debug_session::log(
                    "H-SCHED-COST",
                    "tile_session.rs:advance_open",
                    "post_bout_costs",
                    &format!(
                        "{{\"n\":{n},\"fin\":{fin_n},\"peek_ms\":{:.3},\"update_ms\":{update_ms:.3}}}",
                        (t_after_peek - t_after_bout).as_secs_f64() * 1000.0
                    ),
                );
            }
            // #endregion
            if !still_working {
                // Seats that failed initialize_batch stay in `active` with no point.
                OutfillInfillScheduler::reclaim_orphaned_active(&mut work.scheduler);
                work.batch = None;
            }
            (finishes, any_outside, work.tile_index, true, wip_updates)
            }
        };
        if any_outside {
            TileScheduler::note_tile_has_outside(&mut self.tile_scheduler, tile_index);
        }
        for (screen, answer) in wip_updates {
            if screen.0 < self.screen_res.0 && screen.1 < self.screen_res.1 {
                let idx = linear_index(screen, self.screen_res.0);
                self.screen_answer[idx] = Some(answer);
            }
        }
        // #region agent log
        let t_finish = Instant::now();
        let fin_n = finishes.len();
        // #endregion
        if !finishes.is_empty() {
            // Same active tile ⇒ one origin: avoid 1024× HashMap entry lookups.
            let origin = tile_origin_for_seat(finishes[0].0, self.screen_res);
            let mag = self.location.zoom_pot;
            let tile = self.answer_tiles.entry(origin).or_insert_with(|| {
                Tile::new(origin, mag)
            });
            self.unsent_origins.insert(origin);
            for (seat, answer) in finishes {
                if seat.0 >= self.screen_res.0 || seat.1 >= self.screen_res.1 {
                    continue;
                }
                let index = linear_index(seat, self.screen_res.0);
                let local = (seat.0 - origin.0, seat.1 - origin.1);
                let proximate = tile.get(local);
                let plain = crate::assemblies::workgroup::tile_publisher::publish_seat(
                    answer
                    , proximate
                );
                let kind = match plain.result {
                    MandelbrotResult::Outside { .. } => SeatKind::Outside
                    , MandelbrotResult::Inside { period } => SeatKind::Inside {
                        period: period.min(u32::MAX as u64) as u32
                    }
                };
                let newly = !self.screen_done[index];
                self.screen_done[index] = true;
                self.screen_kind[index] = Some(kind);
                self.screen_answer[index] = Some(answer);
                if newly {
                    self.seats_done += 1;
                }
                tile.set(local, plain);
            }
        }
        // #region agent log
        if fin_n > 0 {
            let n = crate::assemblies::workgroup::debug_session::rpc_tick();
            if n <= 24 || n % 32 == 0 {
                crate::assemblies::workgroup::debug_session::log(
                    "H-FINISH-SEAT",
                    "tile_session.rs:advance_open",
                    "finish_screen_loop",
                    &format!(
                        "{{\"n\":{n},\"fin\":{fin_n},\"finish_ms\":{:.3}}}",
                        t_finish.elapsed().as_secs_f64() * 1000.0
                    ),
                );
            }
        }
        // #endregion
        progressed
    }

    fn try_resolve_periods(&mut self) -> bool {
        let Some(work) = self.active_tile.as_mut() else {
            return false;
        };
        // Period resolve must not wait for an open bout batch: deep Inside points
        // can iterate for a long time, and without resolved periods flood-in never
        // starts — the whole spiral freezes on one tile under headed CPU pacing.
        if !OutfillInfillScheduler::needs_period_resolve(&work.scheduler) {
            return false;
        }
        let locals = OutfillInfillScheduler::take_period_resolve_locals(&work.scheduler);
        if locals.is_empty() {
            OutfillInfillScheduler::mark_period_resolve_done(&mut work.scheduler);
            OutfillInfillScheduler::force_progress(&mut work.scheduler);
            return true;
        }
        let Some(generator) = self.worker_state.stencil.as_ref().and_then(|s| {
            s.get_c_generator::<f64>()
        }) else {
            OutfillInfillScheduler::mark_period_resolve_done(&mut work.scheduler);
            OutfillInfillScheduler::force_progress(&mut work.scheduler);
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
        // Period unlock: seed flood/in queues so get_next is not empty next step.
        OutfillInfillScheduler::force_progress(&mut work.scheduler);
        for (screen, answer) in updates {
            self.finish_screen_seat(screen, answer);
        }
        true
    }

    fn begin_tile(&mut self, tile_index: usize) {
        let origin = TileScheduler::tile_origin(&self.tile_scheduler, tile_index);
        let extent = TileScheduler::tile_extent(&self.tile_scheduler, tile_index);
        let tile = Tile::new(origin, self.location.zoom_pot);
        let touches = origin.0 == 0
            || origin.1 == 0
            || origin.0 + extent.0 >= self.screen_res.0
            || origin.1 + extent.1 >= self.screen_res.1;
        let mut scheduler = OutfillInfillScheduler::init_for_tile_extent_screen(extent, touches);
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
            , hover: None,
            mag_velocity: 0.0
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
                let work = self.lookahead_work.as_mut().unwrap();
                let proximate = work.answer_tile.get(local);
                let plain = crate::assemblies::workgroup::tile_publisher::publish_seat(
                    hint_answer
                    , proximate
                );
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
            let batch = ActiveGearWork::initialize(
                self.worker_state.cpu.gear
                , &self.worker_state
                , &work.tile
                , need_work
            );
            work.batch = Some(batch);
            return true;
        }
        if let Some(work) = self.lookahead_work.as_mut() {
            if let Some(batch) = work.batch.as_mut() {
                let still = self.worker_state.workshift_active_gear(batch);
                let peeked = batch.peek(&work.tile);
                let mut updates: [Option<((usize, usize), CalibratedAnswer)>; BATCH_N] =
                    [const { None }; BATCH_N];
                let mut any_finished = false;
                for i in 0..BATCH_N {
                    let Some((local, answer)) = peeked[i] else { continue };
                    any_finished = true;
                    updates[i] = Some((local, answer));
                    let proximate = work.answer_tile.get(local);
                    work.answer_tile.set(
                        local
                        , crate::assemblies::workgroup::tile_publisher::publish_seat(
                            answer
                            , proximate
                        )
                    );
                    batch.clear_slot(i);
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
        let origin = tile_origin_for_seat(seat, self.screen_res);
        let local = (seat.0 - origin.0, seat.1 - origin.1);
        let proximate = self.answer_tiles.get(&origin).and_then(|t| t.get(local));
        let plain = crate::assemblies::workgroup::tile_publisher::publish_seat(
            answer
            , proximate
        );
        let kind = match plain.result {
            MandelbrotResult::Outside { .. } => SeatKind::Outside
            , MandelbrotResult::Inside { period } => SeatKind::Inside {
                period: period.min(u32::MAX as u64) as u32
            }
        };
        let newly = !self.screen_done[index];
        self.screen_done[index] = true;
        self.screen_kind[index] = Some(kind);
        self.screen_answer[index] = Some(answer);
        if newly {
            self.seats_done += 1;
        }
        let tile = self.answer_tiles.entry(origin).or_insert_with(|| {
            Tile::new(origin, self.location.zoom_pot)
        });
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

fn same_mag_seat_delta(
    old: &ObjectivePosAndZoom
    , new: &ObjectivePosAndZoom
) -> Option<(i32, i32)> {
    crate::assemblies::headgroup::window::gpu_display::seat_delta_pixels(old, new)
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
        // Agnostic is unfinished: never treat as Inside for flood/period edges.
        , CalibratedMandelbrotResult::Agnostic { .. } => SeatKind::Outside
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
                , escape_time_angle: 0
                , min_magnitude_angle: 0
            }
        }
        , CalibratedMandelbrotResult::Inside { period } => {
            Answer {
                result: MandelbrotResult::Inside {
                    period: period.lower_bound
                }
                , min_magnitude_time: answer.min_magnitude_time.lower_bound
                , min_magnitude: answer.min_magnitude.lower_bound
                , escape_time_angle: 0
                , min_magnitude_angle: 0
            }
        }
        , CalibratedMandelbrotResult::Agnostic { .. } => {
            // Live publish uses publish_seat; this helper must not invent Inside.
            NORES_ANSWER
        }
    }
}

#[cfg(test)]
#[path = "tile_session_tests.rs"]
mod tile_session_tests;
