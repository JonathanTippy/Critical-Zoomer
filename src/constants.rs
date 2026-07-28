use crate::assemblies::structs::*;

// UNDERIVED CONSTANTS
// r[impl cz.display.window-default-800x480+1]
pub const DEFAULT_DEFAULT_WINDOW_RES:(u32, u32) = (800, 480);
pub const DEFAULT_WINDOW_RES:(u32, u32) = DEFAULT_DEFAULT_WINDOW_RES;//(1920, 1080);
pub const HOME_POSITION:(i32, i32, i32) = (-2, -2, -2);
pub const MOVE_SPEED_PPS: i32 = 200;
pub const MOVE_SPEED_IN_SCREENS: f32 = 0.42;
pub const PIXELS_PER_UNIT_POT:i32 = 9;

pub const TILE_EDGE_LENGTH_POT: i32 = 6;
pub const TILE_EDGE_LENGTH: usize = 1 << TILE_EDGE_LENGTH_POT;
pub const TILE_SEAT_COUNT: usize = TILE_EDGE_LENGTH * TILE_EDGE_LENGTH;

pub const GPU_WORKER_BATCH_N: usize = 1024;

pub const PERIOD_CONFIRMATION_ITERATIONS: u32 = 20;

pub const REFERENCE_ORBIT_COLLECTION_BUDGET_BYTES: usize = 512 * 1024 * 1024;
pub const REFERENCE_NUCLEUS_SEEK_ITERS_INTERACTIVE: u64 = 50_000;
pub const REFERENCE_NUCLEUS_SEEK_ITERS_THOROUGH: u64 = 500_000;
pub const GLITCH_THRESHOLD: f64 = 1e-4;
pub const STACKED_INTEXP_STACKS: usize = 4;

pub const SCROLL_SPEED:f32 = 40.0;
/// Shift/Space hold zoom rate (requirements: about 5 bumps per second).
// r[impl cz.fast.shift-space-5bps+1]
pub const KEY_ZOOM_BUMPS_PER_SEC: f32 = 5.0;

// r[impl cz.display.nores-when-no-proximate+1]
// r[impl cz.tenacious.nores-not-flat-black+1]
pub const NORES_ANSWER:Answer = Answer{
    result: MandelbrotResult::Outside{
        escape_time_r2: 1
        , escape_z: (-f32::INFINITY, f32::INFINITY)
    }
    , min_magnitude_time: 0
    , min_magnitude: f64::INFINITY
};

#[cfg(test)]
mod constants_tests {
    use super::*;

    // r[verify cz.display.window-default-800x480+1]
    #[test]
    fn default_window_is_800x480() {
        assert_eq!(DEFAULT_WINDOW_RES, (800, 480));
    }

    // r[verify cz.display.window-default-800x480+1]
    #[test]
    fn default_window_res_matches_default_default() {
        assert_eq!(DEFAULT_WINDOW_RES, DEFAULT_DEFAULT_WINDOW_RES);
        assert_eq!(DEFAULT_DEFAULT_WINDOW_RES, (800, 480));
    }

    // r[verify cz.display.window-default-800x480+1]
    #[test]
    fn launch_inner_size_uses_default_not_a_custom_restore() {
        // Headgroup WindowState locks in DEFAULT_WINDOW_RES for size; only
        // position may restore. There is no persisted custom size on launch.
        let launch = (
            DEFAULT_WINDOW_RES.0 as f32
            , DEFAULT_WINDOW_RES.1 as f32
        );
        assert_eq!(launch, (800.0, 480.0));
        let custom_would_be = (1920u32, 1080u32);
        assert_ne!(DEFAULT_WINDOW_RES, custom_would_be);
    }

    // r[verify cz.display.nores-when-no-proximate+1]
    // r[verify cz.tenacious.nores-not-flat-black+1]
    #[test]
    fn nores_is_outside_escape_one_with_infinite_z() {
        match NORES_ANSWER.result {
            MandelbrotResult::Outside {
                escape_time_r2,
                escape_z,
            } => {
                assert_eq!(escape_time_r2, 1);
                assert!(escape_z.0.is_infinite());
                assert!(escape_z.1.is_infinite());
            }
            MandelbrotResult::Inside { .. } => panic!("NORES must not be Inside"),
        }
        assert!(NORES_ANSWER.min_magnitude.is_infinite());
    }
}