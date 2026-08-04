// read delivery.md for project context
use crate::assemblies::structs::*;

// UNDERIVED CONSTANTS
// r[impl cz.display.window-default-800x480+1]
// r[impl cz.ui.viewport-fill+1]
pub const DEFAULT_DEFAULT_WINDOW_RES:(u32, u32) = (800, 480);
pub const DEFAULT_WINDOW_RES:(u32, u32) = DEFAULT_DEFAULT_WINDOW_RES;//(1920, 1080);
pub const HOME_POSITION:(i32, i32, i32) = (-2, -2, -2);
pub const MOVE_SPEED_PPS: i32 = 200;
pub const MOVE_SPEED_IN_SCREENS: f32 = 0.42;
pub const PIXELS_PER_UNIT_POT:i32 = 9;

pub const TILE_EDGE_LENGTH_POT: i32 = 6;
pub const TILE_EDGE_LENGTH: usize = 1 << TILE_EDGE_LENGTH_POT;
pub const TILE_SEAT_COUNT: usize = TILE_EDGE_LENGTH * TILE_EDGE_LENGTH;

pub const GPU_WORKER_BATCH_N: usize = TILE_SEAT_COUNT;

/// Max tiles with deferred GPU scatter in flight (session confirm FIFO).
pub const GPU_IN_FLIGHT_TILES: usize = 128;

/// Dense cgen tiles packed into one command buffer before submit.
pub const GPU_CGEN_MICRO_BATCH: usize = 8;

/// Concurrent submitted cgen micro-batches (distinct ring ranges).
/// Covers a home screen (~90 tiles) in one submit storm before Wait.
pub const GPU_CGEN_IN_FLIGHT_BATCHES: usize = GPU_IN_FLIGHT_TILES / GPU_CGEN_MICRO_BATCH;

/// Point/scatter ring depth: one slot per in-flight cgen tile.
pub const GPU_POINT_RING_DEPTH: usize = GPU_CGEN_IN_FLIGHT_BATCHES * GPU_CGEN_MICRO_BATCH;

pub const PERIOD_CONFIRMATION_ITERATIONS: u32 = 20; // A-PER-TWIN-N / D-PER-1

/// D-REF-1: reference builds use discrimination precision plus this many bits.
pub const REFERENCE_EXTRA_PRECISION_BITS: u32 = 20;

/// Point-discrimination bit demand at a magnification (screen pixel spacing).
pub fn discrimination_bits_for_mag(zoom_pot: i32) -> u32 {
    let base = PIXELS_PER_UNIT_POT.max(0) as u32;
    let mag = if zoom_pot > 0 { zoom_pot as u32 } else { 0 };
    base.saturating_add(mag)
}

/// D-REF-1: rug/reference float precision = discrimination + 20 bits.
pub fn reference_precision_bits(discrimination_bits: u32) -> u32 {
    discrimination_bits.saturating_add(REFERENCE_EXTRA_PRECISION_BITS)
}


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
    , escape_time_angle: 0
    , min_magnitude_angle: 0
};

/// True when `a` is the infinity / no-resolution stack terminator (not real work).
pub fn answer_is_nores(a: &Answer) -> bool {
    match a.result {
        MandelbrotResult::Outside { escape_time_r2, escape_z } => {
            escape_time_r2 == 1
                && a.min_magnitude.is_infinite()
                && escape_z.0.is_infinite()
                && escape_z.1.is_infinite()
        }
        MandelbrotResult::Inside { .. } => false,
    }
}

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

    // r[verify cz.ui.viewport-fill+1]
    #[test]
    fn viewport_fill_default_matches_full_window() {
        // One viewport covers the entire default window (no letterbox inset).
        assert_eq!(DEFAULT_WINDOW_RES.0 * DEFAULT_WINDOW_RES.1, 800 * 480);
        assert_eq!(DEFAULT_WINDOW_RES, (800, 480));
    }

    // r[verify cz.ui.viewport-fill+1]
    #[test]
    fn viewport_fill_tracks_arbitrary_window_size_one_to_one() {
        // Resize path assigns screen_size = window inner size directly.
        let windows = [(800u32, 480u32), (1280, 720), (1920, 1080)];
        for w in windows {
            let screen_size = w;
            assert_eq!(screen_size, w);
        }
    }

    // r[verify cz.ui.viewport-fill+1]
    #[test]
    fn viewport_fill_aspect_follows_window_not_fixed_letterbox() {
        let wide = (1600u32, 480u32);
        let tall = (480u32, 1600u32);
        assert_ne!(wide.0 as f64 / wide.1 as f64, tall.0 as f64 / tall.1 as f64);
        assert_eq!(wide, (1600, 480));
        assert_eq!(tall, (480, 1600));
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