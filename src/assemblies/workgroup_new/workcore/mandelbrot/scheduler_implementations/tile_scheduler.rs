use std::collections::{HashMap, HashSet, VecDeque};

use crate::assemblies::structs::*;
use crate::constants::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TileEdgeCategory {
    Unknown
    , In
    , Out
    , Mixed
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TileSeatKind {
    Outside
    , Inside
}

#[derive(Clone, Copy, Debug)]
pub enum TileSchedulerNext {
    Scredge((usize, usize)),
    BeginTile(usize),
    /// Deeper-mag tile under attention (DFS column). Session resolves absolute origin.
    BeginLookahead { zoom_pot: i32 },
    Idle,
}

#[derive(Clone, Copy)]
enum EdgeId {
    Top = 0
    , Right = 1
    , Bottom = 2
    , Left = 3
}

struct TileRecord {
    origin: (usize, usize)
    , extent: (usize, usize)
    , edges: [TileEdgeCategory; 4]
    , edge_remaining: [usize; 4]
    , has_outside: bool
    , begun: bool
    , finished: bool
}

pub struct TileSchedulerState {
    pub screen_res: (usize, usize)
    , pub attention: (i32, i32)
    // Magnification velocity: >0 zooming in, 0 still, <0 zooming out (design).
    , pub mag_velocity: i32
    // Current stencil mag; lookahead column is base_mag+1 .. base_mag+8.
    , pub base_mag: i32
    // Next bump to emit (1..=8); 0 means column not started. After 8, column done.
    , pub lookahead_bump: u32
    , pub lookahead_column_done: bool
    , tiles: Vec<TileRecord>
    , scredge: VecDeque<(usize, usize)>
    , scredge_active: HashSet<(usize, usize)>
    , seat_kind: HashMap<(usize, usize), TileSeatKind>
}

impl TileSchedulerState {
    pub fn debug_tile_counts(&self) -> (u32, u32, u32) {
        let mut begun = 0u32;
        let mut finished = 0u32;
        let mut unbegun = 0u32;
        for t in &self.tiles {
            if t.finished {
                finished += 1;
            } else if t.begun {
                begun += 1;
            } else {
                unbegun += 1;
            }
        }
        (begun, finished, unbegun)
    }

    pub fn debug_scredge_lens(&self) -> (usize, usize) {
        (self.scredge.len(), self.scredge_active.len())
    }
}

pub struct TileScheduler;

impl TileScheduler {
    pub fn init(screen_res: (usize, usize)) -> TileSchedulerState {
        let origins = tile_origins_covering(screen_res);
        let mut tiles = Vec::with_capacity(origins.len());
        // D-SCH-1: walk the *screen* outer frame first (prototype), not every
        // tile perimeter. Seeding all tile rims delayed BeginTile until a full
        // edge-grid was done and made home fill feel frozen.
        let mut scredge_set: HashSet<(usize, usize)> = screen_border_seats(screen_res);
        for origin in origins {
            let extent = (
                (screen_res.0 - origin.0).min(TILE_EDGE_LENGTH)
                , (screen_res.1 - origin.1).min(TILE_EDGE_LENGTH)
            );
            let mut edge_remaining = [0usize; 4];
            for seat in tile_perimeter_seats(origin, screen_res) {
                let screen = (seat.0 as usize, seat.1 as usize);
                for edge in edges_for_seat(origin, extent, screen) {
                    edge_remaining[edge as usize] += 1;
                }
            }
            tiles.push(TileRecord {
                origin
                , extent
                , edges: [TileEdgeCategory::Unknown; 4]
                , edge_remaining
                , has_outside: false
                , begun: false
                , finished: false
            });
        }
        let mut scredge: VecDeque<(usize, usize)> = scredge_set.into_iter().collect();
        let attention = ((screen_res.0 / 2) as i32, (screen_res.1 / 2) as i32);
        sort_seats_foveated(&mut scredge, attention);
        // #region agent log
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/home/jonathan/git/Critical-Zoomer/.cursor/debug-4c6f94.log")
        {
            use std::io::Write;
            let _ = writeln!(
                f,
                "{{\"sessionId\":\"4c6f94\",\"runId\":\"post-fix\",\"hypothesisId\":\"H3\",\"location\":\"tile_scheduler.rs:init\",\"message\":\"scredge_seed\",\"data\":{{\"screen\":[{},{}],\"scredge_len\":{},\"tiles\":{}}},\"timestamp\":{}}}",
                screen_res.0,
                screen_res.1,
                scredge.len(),
                tiles.len(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis())
                    .unwrap_or(0)
            );
        }
        // #endregion
        TileSchedulerState {
            screen_res
            , attention
            , mag_velocity: 0
            , base_mag: 0
            , lookahead_bump: 0
            , lookahead_column_done: false
            , tiles
            , scredge
            , scredge_active: HashSet::new()
            , seat_kind: HashMap::new()
        }
    }

    pub fn set_attention(state: &mut TileSchedulerState, attention: (i32, i32)) {
        if state.attention != attention {
            // New fovea → new DFS column.
            Self::reset_lookahead_column(state);
        }
        state.attention = attention;
        sort_seats_foveated(&mut state.scredge, attention);
    }

    pub fn set_mag_velocity(state: &mut TileSchedulerState, mag_velocity: i32) {
        if state.mag_velocity < 0 && mag_velocity >= 0 {
            Self::reset_lookahead_column(state);
        }
        state.mag_velocity = mag_velocity;
    }

    pub fn set_base_mag(state: &mut TileSchedulerState, base_mag: i32) {
        if state.base_mag != base_mag {
            Self::reset_lookahead_column(state);
        }
        state.base_mag = base_mag;
    }

    pub fn reset_lookahead_column(state: &mut TileSchedulerState) {
        state.lookahead_bump = 0;
        state.lookahead_column_done = false;
    }

    pub fn next(state: &mut TileSchedulerState) -> TileSchedulerNext {
        // r[impl cz.seamless.foveated-mag-velocity+1]
        // Design (docs/design/tile_scheduler.md):
        // - mag_velocity > 0: focus on foveated lookahead (DFS column under attention)
        // - mag_velocity == 0: foveated screen fill; also lookahead (spiral first, then column)
        // - mag_velocity < 0: prefer low-res / scredge fill (backtracking)
        //
        // At rest, finishing the on-screen spiral before deeper lookahead avoids a
        // sparse foveal strip while the column eats workshifts.
        if state.mag_velocity < 0 {
            if let Some(seat) = Self::take_scredge(state) {
                return TileSchedulerNext::Scredge(seat);
            }
            if let Some(tile) = Self::take_unbegun_nearest(state, false) {
                state.tiles[tile].begun = true;
                return TileSchedulerNext::BeginTile(tile);
            }
            return TileSchedulerNext::Idle;
        }
        if state.mag_velocity == 0 {
            // Design: stationary = foveated screen fill (spiral from attention), then
            // lookahead. Tiles must begin immediately under the pointer/center —
            // do not wait for the full screen-border scredge to finish first.
            if let Some(tile) = Self::take_unbegun_nearest(state, true) {
                state.tiles[tile].begun = true;
                return TileSchedulerNext::BeginTile(tile);
            }
            if let Some(tile) = Self::take_unbegun_nearest(state, false) {
                state.tiles[tile].begun = true;
                return TileSchedulerNext::BeginTile(tile);
            }
            if let Some(seat) = Self::take_scredge(state) {
                return TileSchedulerNext::Scredge(seat);
            }
            if !state.lookahead_column_done {
                if state.lookahead_bump < 8 {
                    state.lookahead_bump += 1;
                    let zoom_pot = state.base_mag.saturating_add(state.lookahead_bump as i32);
                    return TileSchedulerNext::BeginLookahead { zoom_pot };
                }
                state.lookahead_column_done = true;
            }
            return TileSchedulerNext::Idle;
        }
        // mag_velocity > 0: lookahead column first, then same-mag spiral.
        if !state.lookahead_column_done {
            if state.lookahead_bump < 8 {
                state.lookahead_bump += 1;
                let zoom_pot = state.base_mag.saturating_add(state.lookahead_bump as i32);
                return TileSchedulerNext::BeginLookahead { zoom_pot };
            }
            state.lookahead_column_done = true;
        }
        if let Some(tile) = Self::take_unbegun_nearest(state, true) {
            state.tiles[tile].begun = true;
            return TileSchedulerNext::BeginTile(tile);
        }
        if let Some(tile) = Self::take_unbegun_nearest(state, false) {
            state.tiles[tile].begun = true;
            return TileSchedulerNext::BeginTile(tile);
        }
        if let Some(seat) = Self::take_scredge(state) {
            return TileSchedulerNext::Scredge(seat);
        }
        TileSchedulerNext::Idle
    }

    /// Designed multi-mag lookahead column: depth-first bumps below `base_mag`.
    pub fn lookahead_column_mags(base_mag: i32, bumps: u32) -> Vec<i32> {
        let bumps = bumps.min(8);
        (1..=bumps).map(|d| base_mag.saturating_add(d as i32)).collect()
    }

    pub fn take_scredge(state: &mut TileSchedulerState) -> Option<(usize, usize)> {
        let mut rotated = 0usize;
        while let Some(seat) = state.scredge.front().copied() {
            if state.seat_kind.contains_key(&seat) {
                state.scredge.pop_front();
                rotated = 0;
                continue;
            }
            if state.scredge_active.contains(&seat) {
                state.scredge.pop_front();
                state.scredge.push_back(seat);
                rotated += 1;
                if rotated >= state.scredge.len().max(1) {
                    break;
                }
                continue;
            }
            state.scredge.pop_front();
            state.scredge_active.insert(seat);
            return Some(seat);
        }
        None
    }

    pub fn take_scredge_for_origin(
        state: &mut TileSchedulerState
        , origin: (usize, usize)
        , screen_res: (usize, usize)
    ) -> Option<(usize, usize)> {
        let mut rotated = 0usize;
        while let Some(seat) = state.scredge.front().copied() {
            if state.seat_kind.contains_key(&seat) {
                state.scredge.pop_front();
                rotated = 0;
                continue;
            }
            if state.scredge_active.contains(&seat)
                || tile_origin_for_seat(seat, screen_res) != origin
            {
                state.scredge.pop_front();
                state.scredge.push_back(seat);
                rotated += 1;
                if rotated >= state.scredge.len().max(1) {
                    break;
                }
                continue;
            }
            state.scredge.pop_front();
            state.scredge_active.insert(seat);
            return Some(seat);
        }
        None
    }

    fn take_unbegun_out_tile(state: &TileSchedulerState) -> Option<usize> {
        Self::take_unbegun_nearest(state, true)
    }

    fn take_unbegun_other_tile(state: &TileSchedulerState) -> Option<usize> {
        Self::take_unbegun_nearest(state, false)
    }

    /// Next unbegun tile by Chebyshev distance of tile center to attention (spiral-out).
    fn take_unbegun_nearest(state: &TileSchedulerState, outside_only: bool) -> Option<usize> {
        let mut best: Option<(i32, usize)> = None;
        for (idx, tile) in state.tiles.iter().enumerate() {
            if tile.finished || tile.begun {
                continue;
            }
            if outside_only && !(tile.has_outside || tile_edges_suggest_outside(tile)) {
                continue;
            }
            let dist = tile_attention_distance(tile, state.attention);
            match best {
                Some((best_dist, _)) if best_dist <= dist => {}
                _ => best = Some((dist, idx)),
            }
        }
        best.map(|(_, idx)| idx)
    }

    /// Batch cleared without a finish (init miss, empty points): put the seat
    /// back on the scredge queue so take_scredge can offer it again.
    pub fn release_scredge_active(
        state: &mut TileSchedulerState
        , seat: (usize, usize)
    ) {
        state.scredge_active.remove(&seat);
        if state.seat_kind.contains_key(&seat) {
            return;
        }
        if !state.scredge.iter().any(|s| *s == seat) {
            state.scredge.push_back(seat);
        }
    }

    /// Recover seats left in scredge_active after a batch was dropped.
    pub fn reclaim_orphaned_scredge_active(state: &mut TileSchedulerState) -> bool {
        if state.scredge_active.is_empty() {
            return false;
        }
        let stuck: Vec<(usize, usize)> = state.scredge_active.iter().copied().collect();
        for seat in stuck {
            Self::release_scredge_active(state, seat);
        }
        true
    }

    pub fn note_finished(
        state: &mut TileSchedulerState
        , seat: (usize, usize)
        , kind: TileSeatKind
    ) {
        state.scredge_active.remove(&seat);
        if state.seat_kind.insert(seat, kind).is_some() {
            return;
        }
        state.scredge.retain(|s| *s != seat);
        for tile_index in 0..state.tiles.len() {
            let origin = state.tiles[tile_index].origin;
            let extent = state.tiles[tile_index].extent;
            if !contains_local(origin, extent, seat) {
                continue;
            }
            if matches!(kind, TileSeatKind::Outside) {
                state.tiles[tile_index].has_outside = true;
            }
            let edge_list = edges_for_seat(origin, extent, seat);
            for edge in edge_list {
                let remaining = &mut state.tiles[tile_index].edge_remaining[edge as usize];
                if *remaining == 0 {
                    continue;
                }
                *remaining -= 1;
                if *remaining == 0 {
                    let category = categorize_edge(
                        origin
                        , extent
                        , edge
                        , &state.seat_kind
                    );
                    state.tiles[tile_index].edges[edge as usize] = category;
                }
            }
        }
    }

    pub fn note_tile_has_outside(state: &mut TileSchedulerState, tile_index: usize) {
        if let Some(tile) = state.tiles.get_mut(tile_index) {
            tile.has_outside = true;
        }
    }

    pub fn note_tile_finished(state: &mut TileSchedulerState, tile_index: usize) {
        if let Some(tile) = state.tiles.get_mut(tile_index) {
            tile.finished = true;
        }
    }

    /// Drop a begun-but-incomplete tile so `next` can offer it again (or others).
    pub fn release_begun_tile(state: &mut TileSchedulerState, tile_index: usize) {
        if let Some(tile) = state.tiles.get_mut(tile_index) {
            tile.begun = false;
            tile.finished = false;
        }
    }

    /// Any begun/finished tile whose screen seats are not all done can be retried.
    pub fn reopen_incomplete_tiles(
        state: &mut TileSchedulerState
        , screen_done: &[bool]
        , screen_width: usize
    ) -> bool {
        let mut any = false;
        for tile in state.tiles.iter_mut() {
            if !tile.begun && !tile.finished {
                continue;
            }
            let mut incomplete = false;
            for ly in 0..tile.extent.1 {
                for lx in 0..tile.extent.0 {
                    let sx = tile.origin.0 + lx;
                    let sy = tile.origin.1 + ly;
                    let idx = sy * screen_width + sx;
                    if idx >= screen_done.len() || !screen_done[idx] {
                        incomplete = true;
                        break;
                    }
                }
                if incomplete {
                    break;
                }
            }
            if incomplete {
                tile.begun = false;
                tile.finished = false;
                any = true;
            }
        }
        any
    }

    pub fn tile_origin(state: &TileSchedulerState, tile_index: usize) -> (usize, usize) {
        state.tiles[tile_index].origin
    }

    pub fn tile_extent(state: &TileSchedulerState, tile_index: usize) -> (usize, usize) {
        state.tiles[tile_index].extent
    }

    pub fn tile_edge_category(
        state: &TileSchedulerState
        , tile_index: usize
        , edge: usize
    ) -> TileEdgeCategory {
        state.tiles[tile_index].edges[edge]
    }
}

fn tile_edges_suggest_outside(tile: &TileRecord) -> bool {
    tile.edges.iter().any(|e| matches!(e, TileEdgeCategory::Out | TileEdgeCategory::Mixed))
}

fn contains_local(origin: (usize, usize), extent: (usize, usize), seat: (usize, usize)) -> bool {
    seat.0 >= origin.0
        && seat.1 >= origin.1
        && seat.0 < origin.0 + extent.0
        && seat.1 < origin.1 + extent.1
}

fn edges_for_seat(
    origin: (usize, usize)
    , extent: (usize, usize)
    , seat: (usize, usize)
) -> Vec<EdgeId> {
    let mut edges = Vec::new();
    if extent.0 == 0 || extent.1 == 0 {
        return edges;
    }
    let x1 = origin.0 + extent.0 - 1;
    let y1 = origin.1 + extent.1 - 1;
    if seat.1 == origin.1 {
        edges.push(EdgeId::Top);
    }
    if seat.1 == y1 {
        edges.push(EdgeId::Bottom);
    }
    if seat.0 == origin.0 {
        edges.push(EdgeId::Left);
    }
    if seat.0 == x1 {
        edges.push(EdgeId::Right);
    }
    edges
}

fn edge_seats(
    origin: (usize, usize)
    , extent: (usize, usize)
    , edge: EdgeId
) -> Vec<(usize, usize)> {
    let mut seats = Vec::new();
    if extent.0 == 0 || extent.1 == 0 {
        return seats;
    }
    let x0 = origin.0;
    let y0 = origin.1;
    let x1 = origin.0 + extent.0;
    let y1 = origin.1 + extent.1;
    match edge {
        EdgeId::Top => {
            for x in x0..x1 {
                seats.push((x, y0));
            }
        }
        EdgeId::Bottom => {
            for x in x0..x1 {
                seats.push((x, y1 - 1));
            }
        }
        EdgeId::Left => {
            for y in y0..y1 {
                seats.push((x0, y));
            }
        }
        EdgeId::Right => {
            for y in y0..y1 {
                seats.push((x1 - 1, y));
            }
        }
    }
    seats
}

fn tile_attention_distance(tile: &TileRecord, attention: (i32, i32)) -> i32 {
    let cx = (tile.origin.0 + tile.extent.0 / 2) as i32;
    let cy = (tile.origin.1 + tile.extent.1 / 2) as i32;
    let dx = (cx - attention.0).abs();
    let dy = (cy - attention.1).abs();
    dx.max(dy)
}

/// Outer frame of the viewport only (not every tile rim).
fn screen_border_seats(screen_res: (usize, usize)) -> HashSet<(usize, usize)> {
    let mut seats = HashSet::new();
    let (w, h) = screen_res;
    if w == 0 || h == 0 {
        return seats;
    }
    for x in 0..w {
        seats.insert((x, 0));
        seats.insert((x, h - 1));
    }
    for y in 0..h {
        seats.insert((0, y));
        seats.insert((w - 1, y));
    }
    seats
}

fn seat_attention_distance(seat: (usize, usize), attention: (i32, i32)) -> i32 {
    let dx = (seat.0 as i32 - attention.0).abs();
    let dy = (seat.1 as i32 - attention.1).abs();
    dx.max(dy)
}

fn sort_seats_foveated(seats: &mut VecDeque<(usize, usize)>, attention: (i32, i32)) {
    seats
        .make_contiguous()
        .sort_by_key(|s| seat_attention_distance(*s, attention));
}

fn categorize_edge(
    origin: (usize, usize)
    , extent: (usize, usize)
    , edge: EdgeId
    , seat_kind: &HashMap<(usize, usize), TileSeatKind>
) -> TileEdgeCategory {
    let mut saw_in = false;
    let mut saw_out = false;
    for seat in edge_seats(origin, extent, edge) {
        match seat_kind.get(&seat) {
            Some(TileSeatKind::Outside) => { saw_out = true; }
            Some(TileSeatKind::Inside) => { saw_in = true; }
            None => { return TileEdgeCategory::Unknown; }
        }
    }
    match (saw_in, saw_out) {
        (true, true) => TileEdgeCategory::Mixed
        , (false, true) => TileEdgeCategory::Out
        , (true, false) => TileEdgeCategory::In
        , (false, false) => TileEdgeCategory::Unknown
    }
}

#[cfg(test)]
mod tile_scheduler_tests {
    use super::*;

    #[test]
    fn scredge_seeds_screen_border_only() {
        let state = TileScheduler::init((800, 480));
        let expected = 2 * (800 + 480) - 4;
        assert_eq!(
            state.scredge.len()
            , expected
            , "scredge must be the outer frame, not every tile rim"
        );
        for &(x, y) in &state.scredge {
            assert!(
                x == 0 || y == 0 || x == 799 || y == 479
                , "interior rim seat in scredge: ({x},{y})"
            );
        }
    }

    #[test]
    fn init_covers_screen_with_tiles() {
        let state = TileScheduler::init((130, 70));
        assert!(!state.tiles.is_empty());
        // Every screen seat belongs to at least one tile AABB.
        for y in 0..70 {
            for x in 0..130 {
                assert!(
                    state.tiles.iter().any(|t| {
                        x >= t.origin.0
                            && y >= t.origin.1
                            && x < t.origin.0 + t.extent.0
                            && y < t.origin.1 + t.extent.1
                    }),
                    "uncovered seat ({x},{y})"
                );
            }
        }
    }

    #[test]
    fn next_prefers_tile_near_attention() {
        let mut state = TileScheduler::init((96, 96));
        TileScheduler::set_base_mag(&mut state, 0);
        state.lookahead_column_done = true;
        state.lookahead_bump = 8;
        TileScheduler::set_attention(&mut state, (72, 72));
        TileScheduler::set_mag_velocity(&mut state, 0);
        let mut saw_near = false;
        for _ in 0..40 {
            match TileScheduler::next(&mut state) {
                TileSchedulerNext::BeginTile(i) => {
                    let origin = TileScheduler::tile_origin(&state, i);
                    let extent = TileScheduler::tile_extent(&state, i);
                    let cx = (origin.0 + extent.0 / 2) as i32;
                    let cy = (origin.1 + extent.1 / 2) as i32;
                    let dist = (cx - 72).abs().max((cy - 72).abs());
                    if dist <= 64 {
                        saw_near = true;
                        break;
                    }
                    TileScheduler::note_tile_finished(&mut state, i);
                }
                TileSchedulerNext::BeginLookahead { .. } => {}
                TileSchedulerNext::Scredge(seat) => {
                    TileScheduler::note_finished(&mut state, seat, TileSeatKind::Outside);
                }
                TileSchedulerNext::Idle => break,
            }
        }
        assert!(saw_near, "expected a near-attention tile among early BeginTile picks");
    }

    #[test]
    fn scredge_orders_by_attention_distance() {
        let mut state = TileScheduler::init((128, 128));
        TileScheduler::set_attention(&mut state, (10, 10));
        let first = TileScheduler::take_scredge(&mut state).expect("scredge");
        let dist_first = (first.0 as i32 - 10).abs().max((first.1 as i32 - 10).abs());
        assert!(
            dist_first <= 64,
            "first scredge {first:?} should be near attention, dist={dist_first}"
        );
    }

    #[test]
    fn at_rest_begins_foveated_tile_before_draining_scredge() {
        let mut state = TileScheduler::init((128, 96));
        TileScheduler::set_mag_velocity(&mut state, 0);
        TileScheduler::set_attention(&mut state, (64, 48));
        state.lookahead_column_done = true;
        state.lookahead_bump = 8;
        // First offer must be a near-attention tile, not a full border walk.
        match TileScheduler::next(&mut state) {
            TileSchedulerNext::BeginTile(i) => {
                let o = TileScheduler::tile_origin(&state, i);
                let e = TileScheduler::tile_extent(&state, i);
                let cx = (o.0 + e.0 / 2) as i32;
                let cy = (o.1 + e.1 / 2) as i32;
                let dist = (cx - 64).abs().max((cy - 48).abs());
                assert!(
                    dist <= 64
                    , "first BeginTile should be near attention, dist={dist} origin={o:?}"
                );
            }
            other => panic!("expected immediate BeginTile at rest, got {other:?}"),
        }
    }

    #[test]
    fn release_scredge_active_requeues_unfinished() {
        let mut state = TileScheduler::init((64, 64));
        let seat = TileScheduler::take_scredge(&mut state).expect("seat");
        assert!(state.scredge_active.contains(&seat));
        TileScheduler::release_scredge_active(&mut state, seat);
        assert!(!state.scredge_active.contains(&seat));
        assert!(
            state.scredge.iter().any(|s| *s == seat)
            , "released seat must return to the scredge queue"
        );
        // Drain until the released seat is offered again (foveated order may
        // prefer other unfinished seats first).
        let mut found = false;
        for _ in 0..state.scredge.len().saturating_add(8) {
            let Some(next) = TileScheduler::take_scredge(&mut state) else {
                break;
            };
            if next == seat {
                found = true;
                break;
            }
            TileScheduler::note_finished(&mut state, next, TileSeatKind::Outside);
        }
        assert!(found, "released seat {seat:?} must be takeable again");
    }

    #[test]
    fn reopen_incomplete_tiles_clears_begun_finished() {
        let mut state = TileScheduler::init((64, 64));
        state.tiles[0].begun = true;
        state.tiles[0].finished = true;
        let mut done = vec![true; 64 * 64];
        // One seat in tile 0 still NORES.
        done[0] = false;
        assert!(TileScheduler::reopen_incomplete_tiles(&mut state, &done, 64));
        assert!(!state.tiles[0].begun);
        assert!(!state.tiles[0].finished);
    }

    #[test]
    fn eventually_idles_after_draining() {
        let mut state = TileScheduler::init((64, 64));
        state.lookahead_column_done = true;
        state.lookahead_bump = 8;
        let mut guard = 0;
        loop {
            match TileScheduler::next(&mut state) {
                TileSchedulerNext::Idle => break,
                TileSchedulerNext::BeginTile(i) => {
                    TileScheduler::note_tile_finished(&mut state, i);
                }
                TileSchedulerNext::BeginLookahead { .. } => {}
                TileSchedulerNext::Scredge(seat) => {
                    TileScheduler::note_finished(
                        &mut state,
                        seat,
                        TileSeatKind::Outside,
                    );
                }
            }
            guard += 1;
            assert!(guard < 20_000, "scheduler did not idle");
        }
    }

    // r[verify cz.seamless.foveated-mag-velocity+1]
    #[test]
    fn zoom_in_emits_lookahead_column_before_scredge() {
        let mut state = TileScheduler::init((128, 128));
        TileScheduler::set_base_mag(&mut state, 3);
        TileScheduler::set_attention(&mut state, (64, 64));
        TileScheduler::set_mag_velocity(&mut state, 1);
        match TileScheduler::next(&mut state) {
            TileSchedulerNext::BeginLookahead { zoom_pot } => {
                assert_eq!(zoom_pot, 4);
            }
            other => panic!("zoom-in should BeginLookahead first, got {other:?}"),
        }
    }

    // r[verify cz.seamless.foveated-mag-velocity+1]
    #[test]
    fn stationary_prefers_screen_fill_before_lookahead() {
        let mut state = TileScheduler::init((128, 128));
        TileScheduler::set_base_mag(&mut state, 0);
        TileScheduler::set_attention(&mut state, (20, 20));
        TileScheduler::set_mag_velocity(&mut state, 0);
        match TileScheduler::next(&mut state) {
            TileSchedulerNext::BeginTile(_) => {}
            other => panic!(
                "stationary must BeginTile (foveated fill) before lookahead/scredge, got {other:?}"
            ),
        }
    }

    // r[verify cz.seamless.foveated-mag-velocity+1]
    #[test]
    fn zoom_out_prefers_scredge_before_unbegun() {
        let mut state = TileScheduler::init((128, 128));
        TileScheduler::set_attention(&mut state, (64, 64));
        TileScheduler::set_mag_velocity(&mut state, -1);
        match TileScheduler::next(&mut state) {
            TileSchedulerNext::Scredge(_) => {}
            other => panic!("zoom-out should Scredge first, got {other:?}"),
        }
    }

    // r[verify cz.seamless.foveated-mag-velocity+1]
    #[test]
    fn lookahead_column_is_depth_first_eight_bumps() {
        let mags = TileScheduler::lookahead_column_mags(3, 8);
        assert_eq!(mags, vec![4, 5, 6, 7, 8, 9, 10, 11]);
        let capped = TileScheduler::lookahead_column_mags(0, 100);
        assert_eq!(capped.len(), 8, "design caps lookahead at 8 bumps");
        assert!(capped.windows(2).all(|w| w[1] == w[0] + 1));
    }

    // r[verify cz.seamless.foveated-mag-velocity+1]
    #[test]
    fn lookahead_column_emits_eight_increasing_mags_then_begin_tile() {
        let mut state = TileScheduler::init((128, 128));
        TileScheduler::set_base_mag(&mut state, 2);
        TileScheduler::set_mag_velocity(&mut state, 1);
        let mut mags = Vec::new();
        for _ in 0..8 {
            match TileScheduler::next(&mut state) {
                TileSchedulerNext::BeginLookahead { zoom_pot } => mags.push(zoom_pot),
                other => panic!("expected BeginLookahead, got {other:?}"),
            }
        }
        assert_eq!(mags, TileScheduler::lookahead_column_mags(2, 8));
        match TileScheduler::next(&mut state) {
            TileSchedulerNext::BeginTile(_) | TileSchedulerNext::Scredge(_) => {}
            other => panic!("after column expected same-mag work, got {other:?}"),
        }
    }

    // r[verify cz.seamless.foveated-mag-velocity+1]
    #[test]
    fn attention_change_resets_lookahead_column() {
        let mut state = TileScheduler::init((128, 128));
        TileScheduler::set_base_mag(&mut state, 0);
        TileScheduler::set_mag_velocity(&mut state, 1);
        let _ = TileScheduler::next(&mut state);
        assert_eq!(state.lookahead_bump, 1);
        TileScheduler::set_attention(&mut state, (10, 10));
        assert_eq!(state.lookahead_bump, 0);
        assert!(!state.lookahead_column_done);
        match TileScheduler::next(&mut state) {
            TileSchedulerNext::BeginLookahead { zoom_pot } => assert_eq!(zoom_pot, 1),
            other => panic!("reset column should restart at +1, got {other:?}"),
        }
    }
}
