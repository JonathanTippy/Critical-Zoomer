use crate::assemblies::structs::*;
use crate::constants::*;
use crate::utils::ObjectivePosAndZoom;

#[derive(Copy, Clone, Debug)]
pub struct GPUAnswer {
    pub result: MandelbrotResult
    , pub min_magnitude_time: u64
    , pub min_magnitude: f64
}

impl From<Answer> for GPUAnswer {
    fn from(answer: Answer) -> Self {
        GPUAnswer {
            result: answer.result
            , min_magnitude_time: answer.min_magnitude_time
            , min_magnitude: answer.min_magnitude
        }
    }
}

impl From<GPUAnswer> for Answer {
    fn from(answer: GPUAnswer) -> Self {
        Answer {
            result: answer.result
            , min_magnitude_time: answer.min_magnitude_time
            , min_magnitude: answer.min_magnitude
        }
    }
}

#[derive(Clone, Debug)]
pub struct GPUTile {
    pub origin_seat: (usize, usize)
    , pub magnification_pot: i32
    , pub screen_res: (usize, usize)
    , pub location: ObjectivePosAndZoom
    , pub data: [Option<GPUAnswer>; TILE_SEAT_COUNT]
}

impl GPUTile {
    pub fn from_answer_tile(
        tile: &Tile<Answer>
        , screen_res: (usize, usize)
        , location: ObjectivePosAndZoom
    ) -> Self {
        let mut data = [None; TILE_SEAT_COUNT];
        for i in 0..TILE_SEAT_COUNT {
            data[i] = tile.data[i].map(GPUAnswer::from);
        }
        GPUTile {
            origin_seat: tile.origin_seat
            , magnification_pot: tile.magnification_pot
            , screen_res
            , location
            , data
        }
    }

    pub fn get(&self, local_seat: (usize, usize)) -> Option<GPUAnswer> {
        self.data[Tile::<()>::in_tile_index(local_seat)]
    }

    pub fn set(&mut self, local_seat: (usize, usize), value: GPUAnswer) {
        self.data[Tile::<()>::in_tile_index(local_seat)] = Some(value);
    }

    pub fn screen_seat(&self, local_seat: (usize, usize)) -> (usize, usize) {
        (
            self.origin_seat.0 + local_seat.0
            , self.origin_seat.1 + local_seat.1
        )
    }
}
