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

#[cfg(test)]
mod gpu_tile_upload_tests {
    use super::*;
    use crate::constants::NORES_ANSWER;
    use crate::intexp::IntExp;

    fn loc() -> ObjectivePosAndZoom {
        ObjectivePosAndZoom {
            pos: (IntExp::ZERO, IntExp::ZERO),
            zoom_pot: 0,
        }
    }

    // r[verify cz.display.nores-when-no-proximate+1]
    // r[verify cz.tenacious.nores-not-flat-black+1]
    #[test]
    fn from_answer_tile_preserves_nores() {
        let mut tile = Tile::new((0, 0), 0);
        tile.set((0, 0), NORES_ANSWER);
        let gpu = GPUTile::from_answer_tile(&tile, (64, 64), loc());
        let a = gpu.get((0, 0)).expect("seat");
        match a.result {
            MandelbrotResult::Outside { escape_time_r2, .. } => assert_eq!(escape_time_r2, 1),
            MandelbrotResult::Inside { .. } => panic!("NORES must stay Outside"),
        }
        assert!(a.min_magnitude.is_infinite());
    }

    #[test]
    fn from_answer_tile_roundtrips_answer_fields() {
        let answer = Answer {
            result: MandelbrotResult::Outside {
                escape_time_r2: 12,
                escape_z: (3.0, 4.0),
            },
            min_magnitude_time: 2,
            min_magnitude: 0.5,
        };
        let mut tile = Tile::new((64, 0), 3);
        tile.set((1, 2), answer);
        let gpu = GPUTile::from_answer_tile(&tile, (128, 128), loc());
        assert_eq!(gpu.origin_seat, (64, 0));
        assert_eq!(gpu.magnification_pot, 3);
        let back: Answer = gpu.get((1, 2)).unwrap().into();
        match back.result {
            MandelbrotResult::Outside {
                escape_time_r2,
                escape_z,
            } => {
                assert_eq!(escape_time_r2, 12);
                assert_eq!(escape_z, (3.0, 4.0));
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(back.min_magnitude_time, 2);
        assert_eq!(back.min_magnitude, 0.5);
    }

    #[test]
    fn empty_seats_stay_none_through_upload() {
        let tile = Tile::<Answer>::new((0, 0), 0);
        let gpu = GPUTile::from_answer_tile(&tile, (64, 64), loc());
        assert!(gpu.get((0, 0)).is_none());
        assert!(gpu.get((63, 63)).is_none());
    }
}
