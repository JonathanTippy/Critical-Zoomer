use crate::assemblies::structs::*;
use crate::constants::*;

#[derive(Clone, Copy, Debug)]
pub struct Tile<T: Copy> {
    pub origin_seat: (usize, usize)
    , pub magnification_pot: i32
    , pub data: [Option<T>; TILE_SEAT_COUNT]
}

impl<T: Copy> Tile<T> {
    pub fn new(origin_seat: (usize, usize), magnification_pot: i32) -> Self {
        Tile {
            origin_seat
            , magnification_pot
            , data: [None; TILE_SEAT_COUNT]
        }
    }

    pub fn in_tile_index(local_seat: (usize, usize)) -> usize {
        local_seat.1 * TILE_EDGE_LENGTH + local_seat.0
    }

    pub fn screen_seat(&self, local_seat: (usize, usize)) -> (usize, usize) {
        (
            self.origin_seat.0 + local_seat.0
            , self.origin_seat.1 + local_seat.1
        )
    }

    pub fn contains_screen_seat(&self, seat: (usize, usize)) -> bool {
        seat.0 >= self.origin_seat.0
            && seat.0 < self.origin_seat.0 + TILE_EDGE_LENGTH
            && seat.1 >= self.origin_seat.1
            && seat.1 < self.origin_seat.1 + TILE_EDGE_LENGTH
    }

    pub fn local_seat(&self, seat: (usize, usize)) -> Option<(usize, usize)> {
        if !self.contains_screen_seat(seat) {
            return None;
        }
        Some((seat.0 - self.origin_seat.0, seat.1 - self.origin_seat.1))
    }

    pub fn set(&mut self, local_seat: (usize, usize), value: T) {
        self.data[Self::in_tile_index(local_seat)] = Some(value);
    }

    pub fn get(&self, local_seat: (usize, usize)) -> Option<T> {
        self.data[Self::in_tile_index(local_seat)]
    }

    pub fn seats_done(&self) -> usize {
        self.data.iter().filter(|a| a.is_some()).count()
    }
}

pub fn tile_origins_covering(resolution: (usize, usize)) -> Vec<(usize, usize)> {
    let mut origins = Vec::new();
    let mut y = 0usize;
    while y < resolution.1 {
        let mut x = 0usize;
        while x < resolution.0 {
            origins.push((x, y));
            x += TILE_EDGE_LENGTH;
        }
        y += TILE_EDGE_LENGTH;
    }
    origins
}

pub fn tile_perimeter_seats(
    origin: (usize, usize)
    , screen_res: (usize, usize)
) -> Vec<(i32, i32)> {
    let mut seats = Vec::new();
    let x0 = origin.0;
    let y0 = origin.1;
    let x1 = (origin.0 + TILE_EDGE_LENGTH).min(screen_res.0);
    let y1 = (origin.1 + TILE_EDGE_LENGTH).min(screen_res.1);
    if x0 >= x1 || y0 >= y1 {
        return seats;
    }
    for x in x0..x1 {
        seats.push((x as i32, y0 as i32));
        if y1 > y0 + 1 {
            seats.push((x as i32, (y1 - 1) as i32));
        }
    }
    for y in (y0 + 1)..(y1.saturating_sub(1)) {
        seats.push((x0 as i32, y as i32));
        if x1 > x0 + 1 {
            seats.push(((x1 - 1) as i32, y as i32));
        }
    }
    seats
}
