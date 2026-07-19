use std::collections::{HashMap, VecDeque};
use std::time::Instant;

use crate::assemblies::structs::*;
use crate::assemblies::workgroup::screen_worker::workshift::CompletedPoint;
use crate::assemblies::workgroup_new::temporary_color32_bridge::*;
use crate::assemblies::workgroup_new::structs::mandelbrotable::*;
use crate::assemblies::workgroup_new::workcore::mandelbrot::*;
use crate::assemblies::workgroup_new::workcore::mandelbrot::worker_implementations::naive_cpu_worker::*;
use crate::assemblies::workgroup_new::workcore::mandelbrot::worker_implementations::periodicity_detector::*;
use crate::constants::*;
use crate::intexp::*;
use crate::utils::ObjectivePosAndZoom;

#[derive(Clone, Copy)]
enum Step {
    Scredge
    , Edge
    , Out
    , PeriodEdge
    , FloodIn
    , In
}

#[derive(Clone, Copy)]
enum SeatKind {
    Outside
    , Inside { period: u32 }
}

pub struct TileSession {
    pub stencil: PointStencil
    , pub screen_res: (usize, usize)
    , pub location: ObjectivePosAndZoom
    , pub attention: (i32, i32)
    , active_tile: Tile<Answer>
    , tile_origins: Vec<(usize, usize)>
    , origin_index: usize
    , screen_done: Vec<bool>
    , screen_kind: Vec<Option<SeatKind>>
    , seats_done: usize
    , seats_total: usize
    , active_points: HashMap<usize, ActivePoint<f64, CpuPeriodicityDetector>>
    , scredge: VecDeque<(i32, i32)>
    , edge_queue: VecDeque<((i32, i32), u32)>
    , out_queue: VecDeque<((i32, i32), u32)>
    , period_edge_queue: VecDeque<((i32, i32), u32)>
    , flood_in_queue: VecDeque<((i32, i32), u32)>
    , in_queue: VecDeque<((i32, i32), u32)>
    , screen_edge_remaining: usize
    , worker_state: NaiveCpuWorkerState
    , workshifts: u32
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
        let tile_origins = tile_origins_covering(res);
        let first_origin = tile_origins.first().copied().unwrap_or((0, 0));
        let mut session = TileSession {
            stencil
            , screen_res: res
            , location: location.clone()
            , attention: ((res.0 / 2) as i32, (res.1 / 2) as i32)
            , active_tile: Tile::new(first_origin, location.zoom_pot)
            , tile_origins
            , origin_index: 0
            , screen_done: vec![false; res.0 * res.1]
            , screen_kind: vec![None; res.0 * res.1]
            , seats_done: 0
            , seats_total: res.0 * res.1
            , active_points: HashMap::new()
            , scredge: VecDeque::new()
            , edge_queue: VecDeque::new()
            , out_queue: VecDeque::new()
            , period_edge_queue: VecDeque::new()
            , flood_in_queue: VecDeque::new()
            , in_queue: VecDeque::new()
            , screen_edge_remaining: Self::screen_edge_seats(res).len()
            , worker_state: NaiveCpuWorkerState::default()
            , workshifts: 0
        };
        session.seed_scredge();
        session
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

    pub fn workshift(&mut self) -> Vec<(CompletedPoint, usize)> {
        let mut out = Vec::new();
        let started = Instant::now();
        while started.elapsed().as_millis() < 10 {
            if self.seats_done >= self.seats_total {
                break;
            }
            if self.queues_empty() {
                if !self.advance_active_tile() {
                    break;
                }
            }
            let completed = self.work_one_seat();
            if completed.is_empty() {
                if self.queues_empty() {
                    if !self.advance_active_tile() {
                        break;
                    }
                }
            } else {
                out.extend(completed);
            }
        }
        self.workshifts = self.workshifts.wrapping_add(1);
        out
    }

    fn queues_empty(&self) -> bool {
        self.scredge.is_empty()
            && self.edge_queue.is_empty()
            && self.out_queue.is_empty()
            && self.period_edge_queue.is_empty()
            && self.flood_in_queue.is_empty()
            && self.in_queue.is_empty()
    }

    fn seed_scredge(&mut self) {
        self.scredge.clear();
        for pos in Self::screen_edge_seats(self.screen_res) {
            let index = linear_index((pos.0 as usize, pos.1 as usize), self.screen_res.0);
            if !self.screen_done[index] {
                self.scredge.push_back(pos);
            }
        }
    }

    fn screen_edge_seats(res: (usize, usize)) -> Vec<(i32, i32)> {
        let mut seats = Vec::new();
        if res.0 == 0 || res.1 == 0 {
            return seats;
        }
        for i in 0..(res.0 as i32) {
            seats.push((i, 0));
            if res.1 > 1 {
                seats.push((i, (res.1 - 1) as i32));
            }
        }
        for i in 1..(res.1.saturating_sub(1) as i32) {
            seats.push((0, i));
            if res.0 > 1 {
                seats.push(((res.0 - 1) as i32, i));
            }
        }
        seats
    }

    fn is_screen_edge_seat(&self, pos: (i32, i32)) -> bool {
        pos.0 == 0
            || pos.1 == 0
            || pos.0 as usize + 1 == self.screen_res.0
            || pos.1 as usize + 1 == self.screen_res.1
    }

    fn advance_active_tile(&mut self) -> bool {
        let start = self.origin_index;
        loop {
            self.origin_index += 1;
            if self.origin_index >= self.tile_origins.len() {
                self.origin_index = 0;
            }
            if self.origin_index == start && self.tile_incomplete_count(self.origin_index) == 0 {
                return self.fill_remaining_interior();
            }
            if self.tile_incomplete_count(self.origin_index) > 0 {
                let origin = self.tile_origins[self.origin_index];
                self.active_tile = Tile::new(origin, self.location.zoom_pot);
                if self.screen_edge_remaining > 0 {
                    self.seed_scredge();
                }
                if self.queues_empty() {
                    self.fill_tile_interior_seeds();
                }
                return !self.queues_empty();
            }
            if self.origin_index == start {
                return self.fill_remaining_interior();
            }
        }
    }

    fn tile_incomplete_count(&self, origin_index: usize) -> usize {
        let origin = self.tile_origins[origin_index];
        let mut n = 0usize;
        for local_y in 0..TILE_EDGE_LENGTH {
            for local_x in 0..TILE_EDGE_LENGTH {
                let seat = (origin.0 + local_x, origin.1 + local_y);
                if seat.0 >= self.screen_res.0 || seat.1 >= self.screen_res.1 {
                    continue;
                }
                let index = linear_index(seat, self.screen_res.0);
                if !self.screen_done[index] {
                    n += 1;
                }
            }
        }
        n
    }

    fn fill_tile_interior_seeds(&mut self) {
        let origin = self.active_tile.origin_seat;
        for local_y in 0..TILE_EDGE_LENGTH {
            for local_x in 0..TILE_EDGE_LENGTH {
                let seat = (origin.0 + local_x, origin.1 + local_y);
                if seat.0 >= self.screen_res.0 || seat.1 >= self.screen_res.1 {
                    continue;
                }
                let index = linear_index(seat, self.screen_res.0);
                if !self.screen_done[index] {
                    self.in_queue.push_back(((seat.0 as i32, seat.1 as i32), 0));
                }
            }
        }
    }

    fn fill_remaining_interior(&mut self) -> bool {
        let mut any = false;
        for y in 0..self.screen_res.1 {
            for x in 0..self.screen_res.0 {
                let index = linear_index((x, y), self.screen_res.0);
                if !self.screen_done[index] {
                    self.in_queue.push_back(((x as i32, y as i32), 0));
                    any = true;
                }
            }
        }
        any
    }

    fn pick_step(&mut self) -> Option<((i32, i32), Step)> {
        if self.screen_edge_remaining > 0 && self.scredge.is_empty() {
            self.seed_scredge();
        }
        let try_order: &[Step] = &[
            Step::Out
            , Step::Edge
            , Step::Scredge
            , Step::PeriodEdge
            , Step::FloodIn
            , Step::In
        ];
        for step in try_order {
            match step {
                Step::Out => {
                    if let Some((pos, _)) = self.out_queue.front().copied() {
                        return Some((pos, Step::Out));
                    }
                }
                Step::Edge => {
                    if let Some((pos, _)) = self.edge_queue.front().copied() {
                        return Some((pos, Step::Edge));
                    }
                }
                Step::Scredge => {
                    if let Some(pos) = self.scredge.front().copied() {
                        return Some((pos, Step::Scredge));
                    }
                }
                Step::PeriodEdge => {
                    if let Some((pos, _)) = self.period_edge_queue.front().copied() {
                        return Some((pos, Step::PeriodEdge));
                    }
                }
                Step::FloodIn => {
                    if self.screen_edge_remaining > 0 {
                        continue;
                    }
                    if let Some((pos, _)) = self.flood_in_queue.front().copied() {
                        return Some((pos, Step::FloodIn));
                    }
                }
                Step::In => {
                    if let Some((pos, _)) = self.in_queue.front().copied() {
                        return Some((pos, Step::In));
                    }
                }
            }
        }
        None
    }

    fn pop_step(&mut self, step: Step) {
        match step {
            Step::Scredge => { self.scredge.pop_front(); }
            Step::Edge => { self.edge_queue.pop_front(); }
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
            Step::PeriodEdge => {
                if let Some(item) = self.period_edge_queue.pop_front() {
                    self.period_edge_queue.push_back(item);
                }
            }
            Step::FloodIn => {
                if let Some(item) = self.flood_in_queue.pop_front() {
                    self.flood_in_queue.push_back(item);
                }
            }
            Step::In => {
                if let Some(item) = self.in_queue.pop_front() {
                    self.in_queue.push_back(item);
                }
            }
            Step::Edge => {
                if let Some(item) = self.edge_queue.pop_front() {
                    self.edge_queue.push_back(item);
                }
            }
            Step::Scredge => {
                if let Some(item) = self.scredge.pop_front() {
                    self.scredge.push_back(item);
                }
            }
        }
    }

    fn work_one_seat(&mut self) -> Vec<(CompletedPoint, usize)> {
        let Some((pos, step)) = self.pick_step() else {
            return Vec::new();
        };
        if pos.0 < 0
            || pos.1 < 0
            || pos.0 as usize >= self.screen_res.0
            || pos.1 as usize >= self.screen_res.1
        {
            self.pop_step(step);
            return Vec::new();
        }
        let seat = (pos.0 as usize, pos.1 as usize);
        let index = linear_index(seat, self.screen_res.0);
        if matches!(step, Step::FloodIn) {
            let period = self.flood_in_queue.front().map(|(_, p)| *p).unwrap_or(0);
            self.pop_step(step);
            let filled = self.flood_in_fill(pos, period);
            if filled.is_empty() && !self.screen_done[index] {
                self.in_queue.push_back((pos, period));
            }
            return filled;
        }
        if self.screen_done[index] {
            self.pop_step(step);
            return Vec::new();
        }
        if !self.active_points.contains_key(&index) {
            let Some(point) = self.make_active_point(seat) else {
                self.pop_step(step);
                return Vec::new();
            };
            self.active_points.insert(index, point);
        }
        let epsilon = {
            let point = self.active_points.get(&index).unwrap();
            1e-12f64.max(point.c.0.abs().max(point.c.1.abs()) * 1e-6)
        };
        {
            let point = self.active_points.get_mut(&index).unwrap();
            iterate_point_bout(
                point
                , self.worker_state.bailout_radius_squared
                , epsilon
                , self.worker_state.iterations_per_bout
            );
        }
        let finished = self.active_points.get(&index).unwrap().finished;
        if !finished {
            if !matches!(step, Step::Edge | Step::PeriodEdge) {
                self.rotate_step(step);
            }
            return Vec::new();
        }
        self.pop_step(step);
        let point = self.active_points.remove(&index).unwrap();
        let answer = point_to_answer(&point);
        let kind = match answer.result {
            MandelbrotResult::Outside { .. } => SeatKind::Outside
            , MandelbrotResult::Inside { period } => SeatKind::Inside {
                period: period.min(u32::MAX as u64) as u32
            }
        };
        self.screen_done[index] = true;
        self.screen_kind[index] = Some(kind);
        self.seats_done += 1;
        if self.is_screen_edge_seat(pos) {
            self.screen_edge_remaining = self.screen_edge_remaining.saturating_sub(1);
        }
        if let Some(local) = self.active_tile.local_seat(seat) {
            self.active_tile.set(local, answer);
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
            self.queue_edge_neighbors(edge.0, edge.1);
        }
        if let Some(edge) = self.seat_is_period_edge(pos) {
            self.queue_period_edge_contour(edge.0, edge.1);
        }
        let start = (point.c.0, point.c.1);
        vec![(answer_to_completed_point(answer, start), index)]
    }

    fn make_active_point(
        &self
        , seat: (usize, usize)
    ) -> Option<ActivePoint<f64, CpuPeriodicityDetector>> {
        let generator = self.stencil.get_c_generator::<f64>()?;
        let c = generator.get_c((
            seat.0.min(u16::MAX as usize) as u16
            , seat.1.min(u16::MAX as usize) as u16
        ));
        let z = (f64::ZERO, f64::ZERO);
        let derivative = (f64::ONE, f64::ZERO);
        Some(ActivePoint {
            c
            , z
            , derivative
            , real_squared: f64::ZERO
            , imag_squared: f64::ZERO
            , real_imag: f64::ZERO
            , iteration_count: 0
            , min_magnitude: f64::MAX
            , min_magnitude_time: 0
            , periodicity_detector: CpuPeriodicityDetector::init(0, z, derivative)
            , escaped: false
            , finished: false
        })
    }

    fn queue_incomplete_neighbors(&mut self, pos: (i32, i32), outside: bool) {
        let neighbors = [
            (pos.0 + 1, pos.1)
            , (pos.0 - 1, pos.1)
            , (pos.0, pos.1 + 1)
            , (pos.0, pos.1 - 1)
        ];
        for n in neighbors {
            if n.0 < 0
                || n.1 < 0
                || n.0 as usize >= self.screen_res.0
                || n.1 as usize >= self.screen_res.1
            {
                continue;
            }
            let index = linear_index((n.0 as usize, n.1 as usize), self.screen_res.0);
            if self.screen_done[index] {
                continue;
            }
            if outside {
                self.out_queue.push_back((n, 0));
            } else {
                self.in_queue.push_back((n, 0));
            }
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
            if n.0 < 0
                || n.1 < 0
                || n.0 as usize >= self.screen_res.0
                || n.1 as usize >= self.screen_res.1
            {
                continue;
            }
            let index = linear_index((n.0 as usize, n.1 as usize), self.screen_res.0);
            if let Some(SeatKind::Inside { period: p }) = self.screen_kind[index] {
                if self.screen_done[index] && p != 0 && p != period {
                    self.period_edge_queue.push_back((n, p));
                }
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
            if n.0 < 0
                || n.1 < 0
                || n.0 as usize >= self.screen_res.0
                || n.1 as usize >= self.screen_res.1
            {
                continue;
            }
            let index = linear_index((n.0 as usize, n.1 as usize), self.screen_res.0);
            match self.screen_kind[index] {
                Some(SeatKind::Outside) => { continue; }
                Some(SeatKind::Inside { period: p })
                    if self.screen_done[index] && p != 0 && p != period =>
                {
                    continue;
                }
                Some(SeatKind::Inside { period: p })
                    if self.screen_done[index] && p != 0 && p == period =>
                {
                    continue;
                }
                _ => {}
            }
            self.flood_in_queue.push_back((n, period));
        }
    }

    fn flood_in_fill(
        &mut self
        , origin: (i32, i32)
        , period: u32
    ) -> Vec<(CompletedPoint, usize)> {
        let mut out = Vec::new();
        if period == 0 {
            return out;
        }
        let mut stack = vec![origin];
        while let Some(pos) = stack.pop() {
            if pos.0 < 0
                || pos.1 < 0
                || pos.0 as usize >= self.screen_res.0
                || pos.1 as usize >= self.screen_res.1
            {
                continue;
            }
            let seat = (pos.0 as usize, pos.1 as usize);
            let index = linear_index(seat, self.screen_res.0);
            match self.screen_kind[index] {
                Some(SeatKind::Outside) => { continue; }
                Some(SeatKind::Inside { period: p })
                    if self.screen_done[index] && p != 0 && p != period =>
                {
                    continue;
                }
                Some(SeatKind::Inside { period: p })
                    if self.screen_done[index] && p == period =>
                {
                    continue;
                }
                _ => {}
            }
            let (min_magnitude, min_magnitude_time, start) = if let Some(point) = self.active_points.remove(&index) {
                (point.min_magnitude, point.min_magnitude_time, (point.c.0, point.c.1))
            } else {
                let Some(generator) = self.stencil.get_c_generator::<f64>() else {
                    continue;
                };
                let c = generator.get_c((
                    seat.0.min(u16::MAX as usize) as u16
                    , seat.1.min(u16::MAX as usize) as u16
                ));
                (f64::INFINITY, 0, c)
            };
            let answer = Answer {
                result: MandelbrotResult::Inside {
                    period: period as u64
                }
                , min_magnitude_time
                , min_magnitude
            };
            let newly_done = !self.screen_done[index];
            self.screen_done[index] = true;
            self.screen_kind[index] = Some(SeatKind::Inside { period });
            if newly_done {
                self.seats_done += 1;
            }
            if let Some(local) = self.active_tile.local_seat(seat) {
                self.active_tile.set(local, answer);
            }
            out.push((answer_to_completed_point(answer, start), index));
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

    fn seat_is_edge(&self, pos: (i32, i32)) -> Option<((i32, i32), (i32, i32))> {
        let index = linear_index((pos.0 as usize, pos.1 as usize), self.screen_res.0);
        let Some(kind) = self.screen_kind[index] else { return None };
        let neighbors = [
            (pos.0 + 1, pos.1)
            , (pos.0 - 1, pos.1)
            , (pos.0, pos.1 + 1)
            , (pos.0, pos.1 - 1)
        ];
        for n in neighbors {
            if n.0 < 0
                || n.1 < 0
                || n.0 as usize >= self.screen_res.0
                || n.1 as usize >= self.screen_res.1
            {
                continue;
            }
            let nindex = linear_index((n.0 as usize, n.1 as usize), self.screen_res.0);
            let Some(nother) = self.screen_kind[nindex] else { continue };
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
        let index = linear_index((pos.0 as usize, pos.1 as usize), self.screen_res.0);
        let Some(SeatKind::Inside { period: p }) = self.screen_kind[index] else {
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
            if n.0 < 0
                || n.1 < 0
                || n.0 as usize >= self.screen_res.0
                || n.1 as usize >= self.screen_res.1
            {
                continue;
            }
            let nindex = linear_index((n.0 as usize, n.1 as usize), self.screen_res.0);
            let Some(SeatKind::Inside { period: q }) = self.screen_kind[nindex] else {
                continue;
            };
            if self.screen_done[nindex] && q != 0 && q != p {
                return Some((pos, n));
            }
        }
        None
    }

    fn queue_period_edge_contour(&mut self, pos1: (i32, i32), pos2: (i32, i32)) {
        self.queue_contour_to(pos1, pos2, true);
    }

    fn queue_edge_neighbors(&mut self, pos1: (i32, i32), pos2: (i32, i32)) {
        self.queue_contour_to(pos1, pos2, false);
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
        } else if pos1.0 > pos2.0 {
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
            if n.0 < 0
                || n.1 < 0
                || n.0 as usize >= self.screen_res.0
                || n.1 as usize >= self.screen_res.1
            {
                continue;
            }
            let index = linear_index((n.0 as usize, n.1 as usize), self.screen_res.0);
            if !self.screen_done[index] {
                if period_edge {
                    self.period_edge_queue.push_front((n, 0));
                } else {
                    self.edge_queue.push_front((n, 0));
                }
            }
        }
    }
}
