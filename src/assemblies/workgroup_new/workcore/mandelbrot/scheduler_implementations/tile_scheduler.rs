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
    Scredge((usize, usize))
    , BeginTile(usize)
    , Idle
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
    , tiles: Vec<TileRecord>
    , scredge: VecDeque<(usize, usize)>
    , scredge_active: HashSet<(usize, usize)>
    , seat_kind: HashMap<(usize, usize), TileSeatKind>
}

pub struct TileScheduler;

impl TileScheduler {
    pub fn init(screen_res: (usize, usize)) -> TileSchedulerState {
        let origins = tile_origins_covering(screen_res);
        let mut tiles = Vec::with_capacity(origins.len());
        let mut scredge_set: HashSet<(usize, usize)> = HashSet::new();
        for origin in origins {
            let extent = (
                (screen_res.0 - origin.0).min(TILE_EDGE_LENGTH)
                , (screen_res.1 - origin.1).min(TILE_EDGE_LENGTH)
            );
            let mut edge_remaining = [0usize; 4];
            for seat in tile_perimeter_seats(origin, screen_res) {
                let screen = (seat.0 as usize, seat.1 as usize);
                scredge_set.insert(screen);
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
        scredge.make_contiguous().sort_by_key(|s| (std::cmp::Reverse(s.1), s.0));
        TileSchedulerState {
            screen_res
            , tiles
            , scredge
            , scredge_active: HashSet::new()
            , seat_kind: HashMap::new()
        }
    }

    pub fn next(state: &mut TileSchedulerState) -> TileSchedulerNext {
        if let Some(tile) = Self::take_unbegun_out_tile(state) {
            state.tiles[tile].begun = true;
            return TileSchedulerNext::BeginTile(tile);
        }
        if let Some(seat) = Self::take_scredge(state) {
            return TileSchedulerNext::Scredge(seat);
        }
        if let Some(tile) = Self::take_unbegun_other_tile(state) {
            state.tiles[tile].begun = true;
            return TileSchedulerNext::BeginTile(tile);
        }
        TileSchedulerNext::Idle
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
        state.tiles.iter().position(|tile| {
            !tile.finished
                && !tile.begun
                && (tile.has_outside || tile_edges_suggest_outside(tile))
        })
    }

    fn take_unbegun_other_tile(state: &TileSchedulerState) -> Option<usize> {
        state.tiles.iter().position(|tile| !tile.finished && !tile.begun)
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
