use crate::assemblies::structs::*;

// UNDERIVED CONSTANTS
pub const DEFAULT_WINDOW_RES:(u32, u32) = (854, 480);
pub const HOME_POSITION:(i32, i32, i32) = (-2, -2, -2);
pub const MOVE_SPEED_PPS: i32 = 200;
pub const MOVE_SPEED_IN_SCREENS: f32 = 0.42;
pub const PIXELS_PER_UNIT_POT:i32 = 9;

pub const SCROLL_SPEED:f32 = 40.0;

pub const NORES_ANSWER:Answer = Answer{
    result: MandelbrotResult::Outside{
        escape_time_r2: 1
        , escape_z: (-f32::INFINITY, f32::INFINITY)
    }
    , min_magnitude_time: 0
    , min_magnitude: f64::INFINITY
    , highlights: NODE+IN
};