use std::collections::{HashSet, VecDeque};

use crate::assemblies::structs::*;
use crate::assemblies::workgroup_new::structs::*;
use crate::assemblies::workgroup_new::workcore::mandelbrot::*;
use crate::assemblies::workgroup_new::workcore::mandelbrot::worker_implementations::periodicity_detector::*;
use crate::constants::*;
use crate::range::*;

pub struct OutfillInfillScheduler;

#[derive(Clone, Copy)]
enum Step {
    Out
    , Edge
    , InFilamentEdge
    , OutFilamentEdge
    , Scredge
    , PeriodEdge
    , FloodIn
    , In
    , SmallTimeEdge
}

impl Step {
    /// Auth intratile preference: fill out > edge > scredge > period edge > flood in > in.
    fn preference_rank(self) -> u8 {
        // PO: out → in/out+filament edges → scredge → period → flood → in → STE last.
        match self {
            Step::Out => 0,
            Step::Edge => 1,
            Step::InFilamentEdge => 2,
            Step::OutFilamentEdge => 3,
            Step::Scredge => 4,
            Step::PeriodEdge => 5,
            Step::FloodIn => 6,
            Step::In => 7,
            Step::SmallTimeEdge => 8,
        }
    }

    fn prefers_over(self, other: Step) -> bool {
        self.preference_rank() < other.preference_rank()
    }
}

/// D-SCH-3: track the active phase job and suspend immediately when higher preference work appears.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhaseJobTracker {
    active: Option<u8>,
    suspended: Option<u8>,
}

impl PhaseJobTracker {
    pub fn idle() -> Self {
        PhaseJobTracker {
            active: None,
            suspended: None,
        }
    }

    pub fn start(&mut self, phase_rank: u8) {
        self.active = Some(phase_rank);
    }

    /// If `incoming` is higher preference (lower rank) than active, suspend active immediately.
    pub fn consider_incoming(&mut self, incoming_rank: u8) -> bool {
        match self.active {
            Some(active) if incoming_rank < active => {
                self.suspended = Some(active);
                self.active = Some(incoming_rank);
                true
            }
            None => {
                self.active = Some(incoming_rank);
                false
            }
            Some(_) => false,
        }
    }

    pub fn active_rank(&self) -> Option<u8> {
        self.active
    }

    pub fn suspended_rank(&self) -> Option<u8> {
        self.suspended
    }

    pub fn resume_suspended(&mut self) {
        if let Some(s) = self.suspended.take() {
            self.active = Some(s);
        }
    }
}

#[derive(Clone, Copy)]
enum SeatKind {
    Outside
    , Inside { period: u32 }
}

pub struct OutfillInfillSchedulerState {
    done: [bool; TILE_SEAT_COUNT]
    , kind: [Option<SeatKind>; TILE_SEAT_COUNT]
    , scredge: VecDeque<(i32, i32)>
    , edge_queue: VecDeque<((i32, i32), u32)>
    , in_filament_edge_queue: VecDeque<((i32, i32), u32)>
    , out_filament_edge_queue: VecDeque<((i32, i32), u32)>
    , small_time_edge_queue: VecDeque<((i32, i32), u32)>
    , out_queue: VecDeque<((i32, i32), u32)>
    , period_edge_queue: VecDeque<((i32, i32), u32)>
    , flood_in_queue: VecDeque<((i32, i32), u32)>
    , in_queue: VecDeque<((i32, i32), u32)>
    , tile_edge_remaining: usize
    , active: HashSet<(usize, usize)>
    , extent: (usize, usize)
    , hint_queue: VecDeque<((usize, usize), CalibratedAnswer)>
    , period_resolve_done: bool
    , phase_jobs: PhaseJobTracker
    , out_fill_complete: bool
}

fn exact_range<T: crate::range::Value>(value: T) -> crate::range::Range<T> {
    crate::range::Range { lower_bound: value, upper_bound: value }
}

fn inside_calibrated(period: u64) -> CalibratedAnswer {
    CalibratedAnswer {
        result: CalibratedMandelbrotResult::Inside {
            period: exact_range(period)
        }
        , min_magnitude_time: exact_range(0)
        , min_magnitude: exact_range(f64::INFINITY)
        , highlights: CalibratedHighlights {
            in_filament: exact_range(false)
            , out_filament: exact_range(false)
            , small_time_edge: exact_range(false)
            , node: exact_range(false)
        }
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

fn local_index(local: (usize, usize)) -> usize {
    Tile::<()>::in_tile_index(local)
}

fn tile_perimeter_local(extent: (usize, usize)) -> Vec<(i32, i32)> {
    let mut seats = Vec::new();
    if extent.0 == 0 || extent.1 == 0 {
        return seats;
    }
    for x in 0..extent.0 {
        seats.push((x as i32, 0));
        if extent.1 > 1 {
            seats.push((x as i32, (extent.1 - 1) as i32));
        }
    }
    for y in 1..extent.1.saturating_sub(1) {
        seats.push((0, y as i32));
        if extent.0 > 1 {
            seats.push(((extent.0 - 1) as i32, y as i32));
        }
    }
    seats
}

impl OutfillInfillScheduler {
    pub fn get_next_n_seats<const N: usize>(
        scheduler_state: &mut OutfillInfillSchedulerState
        , active_tile: &mut Tile<()>
    ) -> [Option<((usize, usize), Option<CalibratedAnswer>)>; N] {
        let _ = active_tile;
        let mut out: [Option<((usize, usize), Option<CalibratedAnswer>)>; N] = [const { None }; N];
        for i in 0..N {
            if let Some((local, hint)) = scheduler_state.hint_queue.pop_front() {
                out[i] = Some((local, Some(hint)));
                continue;
            }
            loop {
                let Some((pos, step)) = scheduler_state.pick_step() else {
                    if !scheduler_state.screen_edge_complete() {
                        break;
                    }
                    if scheduler_state.out_queue.is_empty()
                        && scheduler_state.edge_queue.is_empty()
                        && scheduler_state.scredge.is_empty()
                        && scheduler_state.period_edge_queue.is_empty()
                        && scheduler_state.flood_in_queue.is_empty()
                        && scheduler_state.in_queue.is_empty()
                    {
                        scheduler_state.fill_remaining_in();
                        if scheduler_state.in_queue.is_empty() {
                            break;
                        }
                        continue;
                    }
                    break;
                };
                if !scheduler_state.in_bounds(pos) {
                    scheduler_state.pop_step(step);
                    continue;
                }
                let local = (pos.0 as usize, pos.1 as usize);
                let index = local_index(local);
                if scheduler_state.done[index] {
                    scheduler_state.pop_step(step);
                    continue;
                }
                if scheduler_state.active.contains(&local) {
                    scheduler_state.pop_step(step);
                    continue;
                }
                if matches!(step, Step::FloodIn) {
                    let period = scheduler_state.flood_in_queue.front().map(|(_, p)| *p).unwrap_or(0);
                    scheduler_state.pop_step(step);
                    let flooded = scheduler_state.flood_in_fill(pos, period);
                    if flooded.is_empty() {
                        scheduler_state.in_queue.push_back((pos, period));
                        continue;
                    }
                    let mut flooded = flooded.into_iter();
                    let (first_local, first_hint) = flooded.next().unwrap();
                    out[i] = Some((first_local, Some(first_hint)));
                    for item in flooded {
                        scheduler_state.hint_queue.push_back(item);
                    }
                    break;
                }
                scheduler_state.pop_step(step);
                scheduler_state.active.insert(local);
                out[i] = Some((local, None));
                break;
            }
        }
        out
    }

    pub fn update<const N: usize>(
        scheduler_state: &mut OutfillInfillSchedulerState
        , active_tile: &mut Tile<()>
        , updates: &[Option<((usize, usize), CalibratedAnswer)>; N]
    ) {
        let _ = active_tile;
        for item in updates.iter() {
            let Some((local, answer)) = item else { continue };
            scheduler_state.pop_matching(*local);
            scheduler_state.apply_finished(*local, *answer);
        }
    }
}

impl OutfillInfillSchedulerState {
    fn in_bounds(&self, pos: (i32, i32)) -> bool {
        pos.0 >= 0
            && pos.1 >= 0
            && (pos.0 as usize) < self.extent.0
            && (pos.1 as usize) < self.extent.1
    }

    fn is_tile_edge_seat(&self, pos: (i32, i32)) -> bool {
        pos.0 == 0
            || pos.1 == 0
            || pos.0 as usize + 1 == self.extent.0
            || pos.1 as usize + 1 == self.extent.1
    }

    fn seed_scredge(&mut self) {
        self.scredge.clear();
        for pos in tile_perimeter_local(self.extent) {
            let local = (pos.0 as usize, pos.1 as usize);
            let index = local_index(local);
            if !self.done[index] && !self.active.contains(&local) {
                self.scredge.push_back(pos);
            }
        }
    }

    fn recount_tile_edge_remaining(&mut self) {
        let mut remaining = 0usize;
        for pos in tile_perimeter_local(self.extent) {
            let index = local_index((pos.0 as usize, pos.1 as usize));
            if !self.done[index] {
                remaining += 1;
            }
        }
        self.tile_edge_remaining = remaining;
    }

    fn screen_edge_complete(&self) -> bool {
        self.tile_edge_remaining == 0
    }

    fn pick_step(&mut self) -> Option<((i32, i32), Step)> {
        if self.tile_edge_remaining > 0 && self.scredge.is_empty() {
            self.seed_scredge();
        }
        let try_order: &[Step] = &[
            Step::Out
            , Step::Edge
            , Step::InFilamentEdge
            , Step::OutFilamentEdge
            , Step::Scredge
            , Step::PeriodEdge
            , Step::FloodIn
            , Step::In
            , Step::SmallTimeEdge
        ];
        for step in try_order {
            // STE only after out-fill drained (PO phase 4).
            if matches!(step, Step::SmallTimeEdge) && !self.out_fill_complete {
                if self.out_queue.is_empty() && self.edge_queue.is_empty() {
                    self.out_fill_complete = true;
                } else {
                    continue;
                }
            }
            let found = match step {
                Step::Out => self.out_queue.front().map(|(pos, _)| (*pos, Step::Out)),
                Step::Edge => self.edge_queue.front().map(|(pos, _)| (*pos, Step::Edge)),
                Step::InFilamentEdge => self.in_filament_edge_queue.front().map(|(pos, _)| (*pos, Step::InFilamentEdge)),
                Step::OutFilamentEdge => self.out_filament_edge_queue.front().map(|(pos, _)| (*pos, Step::OutFilamentEdge)),
                Step::Scredge => self.scredge.front().map(|pos| (*pos, Step::Scredge)),
                Step::PeriodEdge => {
                    if !self.screen_edge_complete() { None }
                    else { self.period_edge_queue.front().map(|(pos, _)| (*pos, Step::PeriodEdge)) }
                }
                Step::FloodIn => {
                    if !self.screen_edge_complete() { None }
                    else { self.flood_in_queue.front().map(|(pos, _)| (*pos, Step::FloodIn)) }
                }
                Step::In => {
                    if !self.screen_edge_complete() { None }
                    else { self.in_queue.front().map(|(pos, _)| (*pos, Step::In)) }
                }
                Step::SmallTimeEdge => self.small_time_edge_queue.front().map(|(pos, _)| (*pos, Step::SmallTimeEdge)),
            };
            if let Some((pos, st)) = found {
                self.phase_jobs.consider_incoming(st.preference_rank());
                return Some((pos, st));
            }
        }
        None
    }

    fn pop_step(&mut self, step: Step) {
        match step {
            Step::Scredge => { self.scredge.pop_front(); }
            Step::Edge => { self.edge_queue.pop_front(); }
            Step::InFilamentEdge => { self.in_filament_edge_queue.pop_front(); }
            Step::OutFilamentEdge => { self.out_filament_edge_queue.pop_front(); }
            Step::SmallTimeEdge => { self.small_time_edge_queue.pop_front(); }
            Step::Out => { self.out_queue.pop_front(); }
            Step::PeriodEdge => { self.period_edge_queue.pop_front(); }
            Step::FloodIn => { self.flood_in_queue.pop_front(); }
            Step::In => { self.in_queue.pop_front(); }
        }
    }

    fn rotate_step(&mut self, step: Step) {
        match step {
            Step::Out => {
                if let Some(item) = self.out_queue.pop_front() {
                    self.out_queue.push_back(item);
                }
            }
            Step::In => {
                if let Some(item) = self.in_queue.pop_front() {
                    self.in_queue.push_back(item);
                }
            }
            Step::FloodIn => {
                if let Some(item) = self.flood_in_queue.pop_front() {
                    self.flood_in_queue.push_back(item);
                }
            }
            Step::Scredge => {
                if let Some(item) = self.scredge.pop_front() {
                    self.scredge.push_back(item);
                }
            }
            Step::Edge | Step::PeriodEdge
            | Step::InFilamentEdge | Step::OutFilamentEdge | Step::SmallTimeEdge => {}
        }
    }

    fn queue_incomplete_neighbors(&mut self, pos: (i32, i32), outside: bool) {
        let neighbors = [
            (pos.0 + 1, pos.1)
            , (pos.0 - 1, pos.1)
            , (pos.0, pos.1 + 1)
            , (pos.0, pos.1 - 1)
        ];
        for n in neighbors {
            if !self.in_bounds(n) {
                continue;
            }
            let index = local_index((n.0 as usize, n.1 as usize));
            if self.done[index] {
                continue;
            }
            if outside {
                self.out_queue.push_back((n, 0));
            } else {
                self.in_queue.push_back((n, 0));
            }
        }
    }

    fn queue_flood_in_neighbors(&mut self, pos: (i32, i32), period: u32) {
        if period == 0 {
            return;
        }
        let neighbors = [
            (pos.0 + 1, pos.1)
            , (pos.0 - 1, pos.1)
            , (pos.0, pos.1 + 1)
            , (pos.0, pos.1 - 1)
        ];
        for n in neighbors {
            if !self.in_bounds(n) {
                continue;
            }
            let index = local_index((n.0 as usize, n.1 as usize));
            match self.kind[index] {
                Some(SeatKind::Outside) => { continue; }
                Some(SeatKind::Inside { period: p }) if self.done[index] && p != 0 => {
                    continue;
                }
                _ => {}
            }
            self.flood_in_queue.push_back((n, period));
        }
    }

    fn queue_period_edge_neighbors(&mut self, pos: (i32, i32), period: u32) {
        if period == 0 {
            return;
        }
        let neighbors = [
            (pos.0 + 1, pos.1)
            , (pos.0 - 1, pos.1)
            , (pos.0, pos.1 + 1)
            , (pos.0, pos.1 - 1)
        ];
        for n in neighbors {
            if !self.in_bounds(n) {
                continue;
            }
            let index = local_index((n.0 as usize, n.1 as usize));
            if let Some(SeatKind::Inside { period: p }) = self.kind[index] {
                if self.done[index] && p != 0 && p != period {
                    self.period_edge_queue.push_back((n, p));
                }
            }
        }
    }

    fn seat_is_edge(&self, pos: (i32, i32)) -> Option<((i32, i32), (i32, i32))> {
        let index = local_index((pos.0 as usize, pos.1 as usize));
        let Some(kind) = self.kind[index] else { return None };
        let neighbors = [
            (pos.0 + 1, pos.1)
            , (pos.0 - 1, pos.1)
            , (pos.0, pos.1 + 1)
            , (pos.0, pos.1 - 1)
        ];
        for n in neighbors {
            if !self.in_bounds(n) {
                continue;
            }
            let nindex = local_index((n.0 as usize, n.1 as usize));
            let Some(nother) = self.kind[nindex] else { continue };
            let different = match (kind, nother) {
                (SeatKind::Outside, SeatKind::Inside { .. }) => true
                , (SeatKind::Inside { .. }, SeatKind::Outside) => true
                , _ => false
            };
            if different {
                return Some((pos, n));
            }
        }
        None
    }

    fn seat_is_period_edge(&self, pos: (i32, i32)) -> Option<((i32, i32), (i32, i32))> {
        let index = local_index((pos.0 as usize, pos.1 as usize));
        let Some(SeatKind::Inside { period: p }) = self.kind[index] else {
            return None;
        };
        if p == 0 {
            return None;
        }
        let neighbors = [
            (pos.0 + 1, pos.1)
            , (pos.0 - 1, pos.1)
            , (pos.0, pos.1 + 1)
            , (pos.0, pos.1 - 1)
        ];
        for n in neighbors {
            if !self.in_bounds(n) {
                continue;
            }
            let nindex = local_index((n.0 as usize, n.1 as usize));
            let Some(SeatKind::Inside { period: q }) = self.kind[nindex] else {
                continue;
            };
            if self.done[nindex] && q != 0 && q != p {
                return Some((pos, n));
            }
        }
        None
    }

    fn queue_contour_to(&mut self, pos1: (i32, i32), pos2: (i32, i32), period_edge: bool) {
        let neighbors: [(i32, i32); 8] = if (pos1.0 - pos2.0).abs() == 1 {
            if pos1.0 > pos2.0 {
                [
                    (pos1.0, pos1.1 + 1)
                    , (pos2.0, pos2.1 + 1)
                    , (pos2.0, pos2.1 - 1)
                    , (pos1.0, pos1.1 - 1)
                    , (pos1.0 + 1, pos1.1 + 1)
                    , (pos2.0 - 1, pos2.1 + 1)
                    , (pos2.0 - 1, pos2.1 - 1)
                    , (pos1.0 + 1, pos1.1 - 1)
                ]
            } else {
                [
                    (pos2.0, pos2.1 + 1)
                    , (pos1.0, pos1.1 + 1)
                    , (pos1.0, pos1.1 - 1)
                    , (pos2.0, pos2.1 - 1)
                    , (pos2.0 + 1, pos2.1 + 1)
                    , (pos1.0 - 1, pos1.1 + 1)
                    , (pos1.0 - 1, pos1.1 - 1)
                    , (pos2.0 + 1, pos2.1 - 1)
                ]
            }
        } else if pos1.1 > pos2.1 {
            [
                (pos1.0 + 1, pos1.1)
                , (pos2.0 + 1, pos2.1)
                , (pos1.0 - 1, pos1.1)
                , (pos2.0 - 1, pos2.1)
                , (pos1.0 + 1, pos1.1 + 1)
                , (pos2.0 + 1, pos2.1 - 1)
                , (pos2.0 - 1, pos2.1 - 1)
                , (pos1.0 - 1, pos1.1 + 1)
            ]
        } else {
            [
                (pos1.0 + 1, pos1.1)
                , (pos2.0 + 1, pos2.1)
                , (pos2.0 - 1, pos2.1)
                , (pos1.0 - 1, pos1.1)
                , (pos2.0 + 1, pos2.1 + 1)
                , (pos1.0 + 1, pos1.1 - 1)
                , (pos1.0 - 1, pos1.1 - 1)
                , (pos2.0 - 1, pos2.1 + 1)
            ]
        };
        for n in neighbors {
            if !self.in_bounds(n) {
                continue;
            }
            let index = local_index((n.0 as usize, n.1 as usize));
            if !self.done[index] {
                if period_edge {
                    self.period_edge_queue.push_front((n, 0));
                } else {
                    self.edge_queue.push_front((n, 0));
                }
            }
        }
    }

    fn flood_in_fill(&mut self, origin: (i32, i32), period: u32) -> Vec<((usize, usize), CalibratedAnswer)> {
        let mut out = Vec::new();
        if period == 0 {
            return out;
        }
        let mut stack = vec![origin];
        while let Some(pos) = stack.pop() {
            if !self.in_bounds(pos) {
                continue;
            }
            let local = (pos.0 as usize, pos.1 as usize);
            let index = local_index(local);
            match self.kind[index] {
                Some(SeatKind::Outside) => { continue; }
                Some(SeatKind::Inside { period: p }) if self.done[index] && p != 0 => {
                    continue;
                }
                _ => {}
            }
            self.done[index] = true;
            self.kind[index] = Some(SeatKind::Inside { period });
            self.active.remove(&local);
            let answer = inside_calibrated(period as u64);
            out.push((local, answer));
            let neighbors = [
                (pos.0 + 1, pos.1)
                , (pos.0 - 1, pos.1)
                , (pos.0, pos.1 + 1)
                , (pos.0, pos.1 - 1)
            ];
            for n in neighbors {
                stack.push(n);
            }
        }
        out
    }

    fn apply_finished(&mut self, local: (usize, usize), answer: CalibratedAnswer) {
        let pos = (local.0 as i32, local.1 as i32);
        let index = local_index(local);
        if self.done[index] {
            return;
        }
        let kind = seat_kind_from_calibrated(&answer);
        self.done[index] = true;
        self.kind[index] = Some(kind);
        self.active.remove(&local);
        if self.is_tile_edge_seat(pos) {
            self.tile_edge_remaining = self.tile_edge_remaining.saturating_sub(1);
        }
        match kind {
            SeatKind::Outside => {
                self.queue_incomplete_neighbors(pos, true);
            }
            SeatKind::Inside { period } => {
                if period != 0 {
                    self.queue_period_edge_neighbors(pos, period);
                    self.queue_flood_in_neighbors(pos, period);
                } else {
                    self.queue_incomplete_neighbors(pos, false);
                }
            }
        }
        if let Some(edge) = self.seat_is_edge(pos) {
            self.queue_contour_to(edge.0, edge.1, false);
        }
        if let Some(edge) = self.seat_is_period_edge(pos) {
            self.queue_contour_to(edge.0, edge.1, true);
        }
        // D-SCH-3: enqueue STE candidates from finished seats with meaningful small_time.
        if answer.min_magnitude_time.lower_bound > 0 {
            for n in [
                (pos.0 + 1, pos.1), (pos.0 - 1, pos.1),
                (pos.0, pos.1 + 1), (pos.0, pos.1 - 1),
            ] {
                if self.in_bounds(n) {
                    let ni = local_index((n.0 as usize, n.1 as usize));
                    if !self.done[ni] {
                        self.small_time_edge_queue.push_back((n, 0));
                    }
                }
            }
        }
        // Out-filament edge: period step between finished insides seeds out-filament queue.
        if let CalibratedMandelbrotResult::Inside { period } = answer.result {
            let p = period.lower_bound as u32;
            if p > 0 {
                for n in [
                    (pos.0 + 1, pos.1), (pos.0 - 1, pos.1),
                    (pos.0, pos.1 + 1), (pos.0, pos.1 - 1),
                ] {
                    if !self.in_bounds(n) { continue; }
                    let ni = local_index((n.0 as usize, n.1 as usize));
                    if let Some(SeatKind::Inside { period: op }) = self.kind[ni] {
                        if self.done[ni] && op != 0 && op != p {
                            self.out_filament_edge_queue.push_back((n, 0));
                        }
                    }
                }
            }
        }

    }

    fn fill_remaining_in(&mut self) {
        for y in 0..self.extent.1 {
            for x in 0..self.extent.0 {
                let index = local_index((x, y));
                if !self.done[index] && !self.active.contains(&(x, y)) {
                    self.in_queue.push_back(((x as i32, y as i32), 0));
                }
            }
        }
    }
}

impl OutfillInfillScheduler {
    pub fn init_for_tile_extent(
        extent: (usize, usize)
    ) -> OutfillInfillSchedulerState {
        let extent = (
            extent.0.min(TILE_EDGE_LENGTH)
            , extent.1.min(TILE_EDGE_LENGTH)
        );
        let mut state = OutfillInfillSchedulerState {
            done: [false; TILE_SEAT_COUNT]
            , kind: [None; TILE_SEAT_COUNT]
            , scredge: VecDeque::new()
            , edge_queue: VecDeque::new()
            , in_filament_edge_queue: VecDeque::new()
            , out_filament_edge_queue: VecDeque::new()
            , small_time_edge_queue: VecDeque::new()
            , out_queue: VecDeque::new()
            , period_edge_queue: VecDeque::new()
            , flood_in_queue: VecDeque::new()
            , in_queue: VecDeque::new()
            , tile_edge_remaining: 0
            , active: HashSet::new()
            , extent
            , hint_queue: VecDeque::new()
            , period_resolve_done: false
            , phase_jobs: PhaseJobTracker::idle()
            , out_fill_complete: false
        };
        for y in 0..TILE_EDGE_LENGTH {
            for x in 0..TILE_EDGE_LENGTH {
                if x >= extent.0 || y >= extent.1 {
                    let index = local_index((x, y));
                    state.done[index] = true;
                }
            }
        }
        state.seed_scredge();
        state.recount_tile_edge_remaining();
        state
    }

    pub fn absorb_known(
        state: &mut OutfillInfillSchedulerState
        , local: (usize, usize)
        , answer: CalibratedAnswer
    ) {
        if local.0 >= state.extent.0 || local.1 >= state.extent.1 {
            return;
        }
        let index = local_index(local);
        if state.done[index] {
            return;
        }
        state.apply_finished(local, answer);
    }

    pub fn reseed_after_absorb(state: &mut OutfillInfillSchedulerState) {
        state.seed_scredge();
        state.recount_tile_edge_remaining();
    }

    pub fn has_work(state: &OutfillInfillSchedulerState) -> bool {
        !state.hint_queue.is_empty()
            || !state.out_queue.is_empty()
            || !state.edge_queue.is_empty()
            || !state.scredge.is_empty()
            || !state.period_edge_queue.is_empty()
            || !state.flood_in_queue.is_empty()
            || !state.in_queue.is_empty()
            || !state.active.is_empty()
            || Self::needs_period_resolve(state)
    }

    /// Seats enter `active` when handed to a worker batch. If that batch is
    /// cleared without `apply_finished` (empty init, GPU miss, etc.), they stay
    /// in `active` forever: `get_next` skips them, `has_work` stays true, and
    /// the session either freezes or BeginTile-overwrites a begun tile.
    /// Re-queue unfinished orphans onto scredge (edge) or in_queue (interior).
    pub fn reclaim_orphaned_active(state: &mut OutfillInfillSchedulerState) -> bool {
        if state.active.is_empty() {
            return false;
        }
        let orphans: Vec<(usize, usize)> = state.active.iter().copied().collect();
        let mut any = false;
        for local in orphans {
            state.active.remove(&local);
            any = true;
            let index = local_index(local);
            if state.done[index] {
                continue;
            }
            let pos = (local.0 as i32, local.1 as i32);
            if state.is_tile_edge_seat(pos) {
                state.scredge.push_back(pos);
            } else {
                state.in_queue.push_back((pos, 0));
            }
        }
        if any {
            state.recount_tile_edge_remaining();
        }
        any
    }

    /// Recover when get_next is empty but unfinished seats remain: reclaim
    /// orphans, recount edge, reseed scredge, and fill interiors once the edge
    /// is complete. Returns true if queues may now offer work.
    pub fn force_progress(state: &mut OutfillInfillSchedulerState) -> bool {
        let mut any = Self::reclaim_orphaned_active(state);
        state.recount_tile_edge_remaining();
        if state.tile_edge_remaining > 0 {
            let before = state.scredge.len();
            state.seed_scredge();
            any |= state.scredge.len() > before;
        }
        if state.screen_edge_complete() {
            let before = state.in_queue.len();
            state.fill_remaining_in();
            any |= state.in_queue.len() > before;
        }
        any || Self::has_work(state)
    }

    pub fn needs_period_resolve(state: &OutfillInfillSchedulerState) -> bool {
        !state.period_resolve_done && state.screen_edge_complete()
    }

    pub fn screen_edge_complete(state: &OutfillInfillSchedulerState) -> bool {
        state.screen_edge_complete()
    }

    pub fn take_period_resolve_locals(
        state: &OutfillInfillSchedulerState
    ) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        for y in 0..state.extent.1 {
            for x in 0..state.extent.0 {
                let local = (x, y);
                let index = local_index(local);
                if !state.done[index] {
                    continue;
                }
                let Some(SeatKind::Inside { period: 0 }) = state.kind[index] else {
                    continue;
                };
                let pos = (x as i32, y as i32);
                if state.seat_is_edge(pos).is_some() {
                    out.push(local);
                }
            }
        }
        out
    }

    pub fn apply_period_resolved(
        state: &mut OutfillInfillSchedulerState
        , local: (usize, usize)
        , period: u32
    ) {
        if period == 0 {
            return;
        }
        let index = local_index(local);
        if !state.done[index] {
            return;
        }
        let Some(SeatKind::Inside { period: 0 }) = state.kind[index] else {
            return;
        };
        state.kind[index] = Some(SeatKind::Inside { period });
        let pos = (local.0 as i32, local.1 as i32);
        state.queue_period_edge_neighbors(pos, period);
        state.queue_flood_in_neighbors(pos, period);
    }

    pub fn mark_period_resolve_done(state: &mut OutfillInfillSchedulerState) {
        state.period_resolve_done = true;
    }
}

impl OutfillInfillSchedulerState {
    fn pop_matching(&mut self, local: (usize, usize)) {
        let pos = (local.0 as i32, local.1 as i32);
        self.scredge.retain(|p| *p != pos);
        self.edge_queue.retain(|(p, _)| *p != pos);
        self.in_filament_edge_queue.retain(|(p, _)| *p != pos);
        self.out_filament_edge_queue.retain(|(p, _)| *p != pos);
        self.small_time_edge_queue.retain(|(p, _)| *p != pos);
        self.out_queue.retain(|(p, _)| *p != pos);
        self.period_edge_queue.retain(|(p, _)| *p != pos);
        self.flood_in_queue.retain(|(p, _)| *p != pos);
        self.in_queue.retain(|(p, _)| *p != pos);
    }
}

#[cfg(test)]
mod d_sch1_tests {
    use super::*;

    #[test]
    fn reclaim_orphaned_active_requeues_edge_and_interior() {
        let mut state = OutfillInfillScheduler::init_for_tile_extent((4, 4));
        state.scredge.clear();
        state.out_queue.clear();
        state.edge_queue.clear();
        // Finish every seat except the two we will orphan in `active`.
        for y in 0..4usize {
            for x in 0..4usize {
                if (x, y) == (0, 0) || (x, y) == (1, 1) {
                    continue;
                }
                state.done[local_index((x, y))] = true;
            }
        }
        state.recount_tile_edge_remaining();
        // Simulate seats handed to a batch that was cleared without apply_finished.
        state.active.insert((0, 0)); // edge
        state.active.insert((1, 1)); // interior
        assert!(OutfillInfillScheduler::has_work(&state));
        let mut tile = Tile::new((0, 0), 0);
        let seats = OutfillInfillScheduler::get_next_n_seats::<4>(&mut state, &mut tile);
        assert!(
            seats.iter().all(|s| s.is_none())
            , "active seats must be invisible to get_next until reclaimed, got {seats:?}"
        );
        assert!(OutfillInfillScheduler::reclaim_orphaned_active(&mut state));
        assert!(state.active.is_empty());
        assert!(
            state.scredge.iter().any(|p| *p == (0, 0))
            , "edge orphan must return to scredge"
        );
        assert!(
            state.in_queue.iter().any(|(p, _)| *p == (1, 1))
            , "interior orphan must return to in_queue"
        );
        // Edge seat must be obtainable again (interior stays gated until edge done).
        let seats = OutfillInfillScheduler::get_next_n_seats::<4>(&mut state, &mut tile);
        assert!(
            seats.iter().any(|s| matches!(s, Some(((0, 0), None))))
            , "reclaimed edge must be offered as work, got {seats:?}"
        );
    }

    #[test]
    fn flood_in_period_edge_in_gated_until_screen_edge_complete() {
        let mut state = OutfillInfillScheduler::init_for_tile_extent((4, 4));
        assert!(!state.screen_edge_complete());
        state.scredge.clear();
        state.out_queue.clear();
        state.edge_queue.clear();
        for pos in tile_perimeter_local(state.extent) {
            state.done[local_index((pos.0 as usize, pos.1 as usize))] = true;
        }
        state.tile_edge_remaining = 1;
        state.flood_in_queue.push_back(((1, 1), 2));
        state.period_edge_queue.push_back(((2, 1), 3));
        state.in_queue.push_back(((1, 2), 0));
        assert!(
            state.pick_step().is_none()
            , "D-SCH-1: FloodIn/PeriodEdge/In must wait until screen_edge_complete"
        );
        state.tile_edge_remaining = 0;
        assert!(state.screen_edge_complete());
        let step = state.pick_step();
        assert!(matches!(
            step
            , Some((_, Step::PeriodEdge | Step::FloodIn | Step::In))
        ));
    }

    #[test]
    fn period_resolve_queues_flood_in_with_propagated_period() {
        let mut state = OutfillInfillScheduler::init_for_tile_extent((4, 4));
        let a = local_index((1, 1));
        let b = local_index((2, 1));
        state.done[a] = true;
        state.kind[a] = Some(SeatKind::Inside { period: 0 });
        state.done[b] = true;
        state.kind[b] = Some(SeatKind::Inside { period: 0 });
        state.tile_edge_remaining = 0;
        OutfillInfillScheduler::apply_period_resolved(&mut state, (1, 1), 5);
        assert!(
            state.flood_in_queue.iter().any(|(pos, period)| *pos == (2, 1) && *period == 5)
            , "D-SCH-2: period resolve must queue flood-in neighbors with the resolved period"
        );
    }
}

#[cfg(test)]
mod d_sch3_tests {
    use super::*;

    #[test]
    fn ste_rank_is_last() {
        assert!(Step::Out.prefers_over(Step::SmallTimeEdge));
        assert!(Step::Edge.prefers_over(Step::InFilamentEdge));
        assert!(Step::InFilamentEdge.prefers_over(Step::OutFilamentEdge));
        assert!(Step::In.prefers_over(Step::SmallTimeEdge));
    }

    #[test]
    fn phase_job_tracker_preempts_lower_with_higher() {
        let mut t = PhaseJobTracker::idle();
        t.start(Step::In.preference_rank());
        assert!(t.consider_incoming(Step::Out.preference_rank()));
        assert_eq!(t.active_rank(), Some(Step::Out.preference_rank()));
        assert_eq!(t.suspended_rank(), Some(Step::In.preference_rank()));
    }

    #[test]
    fn ste_queue_present_on_fresh_state() {
        let state = OutfillInfillScheduler::init_for_tile_extent((8, 8));
        assert!(state.small_time_edge_queue.is_empty());
        assert!(state.in_filament_edge_queue.is_empty());
        assert!(state.out_filament_edge_queue.is_empty());
        assert!(!state.out_fill_complete);
    }
}
