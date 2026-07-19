// TEMPORARY throwaway — deleted at Phase 4 cutover.
// Adapts Tile<Answer> seats into CompletedPoint updates for the live
// collector → escaper → colorer Color32 path.

use crate::assemblies::structs::*;
use crate::assemblies::workgroup::screen_worker::workshift::CompletedPoint;
use crate::constants::*;

pub fn answer_to_completed_point(answer: Answer, start_location: (f64, f64)) -> CompletedPoint {
    match answer.result {
        MandelbrotResult::Outside { escape_time_r2, escape_z } => {
            CompletedPoint::Escapes {
                escape_time: escape_time_r2.min(u32::MAX as u64) as u32
                , escape_location: (escape_z.0 as f64, escape_z.1 as f64)
                , start_location
                , smallness: answer.min_magnitude
                , small_time: answer.min_magnitude_time.min(u32::MAX as u64) as u32
            }
        }
        , MandelbrotResult::Inside { period } => {
            CompletedPoint::Repeats {
                period: period.min(u32::MAX as u64) as u32
                , smallness: answer.min_magnitude
                , small_time: answer.min_magnitude_time.min(u32::MAX as u64) as u32
            }
        }
    }
}

pub fn linear_index(seat: (usize, usize), screen_width: usize) -> usize {
    seat.1 * screen_width + seat.0
}

pub fn tile_answers_to_completed_points(
    tile: &Tile<Answer>
    , screen_res: (usize, usize)
    , start_locations: &dyn Fn((usize, usize)) -> (f64, f64)
) -> Vec<(CompletedPoint, usize)> {
    let mut out = Vec::new();
    for local_y in 0..TILE_EDGE_LENGTH {
        for local_x in 0..TILE_EDGE_LENGTH {
            let local = (local_x, local_y);
            let Some(answer) = tile.get(local) else { continue };
            let seat = tile.screen_seat(local);
            if seat.0 >= screen_res.0 || seat.1 >= screen_res.1 {
                continue;
            }
            let index = linear_index(seat, screen_res.0);
            out.push((
                answer_to_completed_point(answer, start_locations(seat))
                , index
            ));
        }
    }
    out
}
