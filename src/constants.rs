
// UNDERIVED CONSTANTS

pub const DEFAULT_WINDOW_RES:(u32, u32) = (854, 480);
/// Standard lib-test screen size. Live app keeps `DEFAULT_WINDOW_RES`.
/// Kept small so craftsmanship fills finish in ≪1s wall (~100ms quiet machine).
pub const TEST_SCREEN_RES: (u32, u32) = (16, 17);
pub const HOME_POSITION:(i32, i32, i32) = (-2, -2, -2);
/// North-tip filament (“high heel”). Picture-sanity latch: magnification is the
/// axis that fails globally; this location is just somewhere with visible
/// structure. Harder minibrots stay for compute-health, not everyday pins.
pub const NORTH_TIP_RE: &str = "-0.161913425661";
pub const NORTH_TIP_IM: &str = "1.035546905361";
/// Headed 2026-08-13 mag 2^43 (`stack:i64`). Black gone; still flat grey; HUD `ipp:0`.
pub const HEADED_I64_GREY_RE: &str = "-0.2067325560057166";
pub const HEADED_I64_GREY_IM: &str = "1.1075689870974698";
pub const HEADED_I64_GREY_MAG: i32 = 43;
pub const MOVE_SPEED_PPS: i32 = 200;
pub const MOVE_SPEED_IN_SCREENS: f32 = 0.42;
pub const PIXELS_PER_UNIT_POT:i32 = 9;

pub const SCROLL_SPEED:f32 = 40.0;

#[cfg(test)]
mod mutant_kill {
    use super::*;

    /// Thought-killed pins for `constants.rs` caught mutants (delete `-` on home UL).
    #[test]
    fn home_position_and_screen_constants_signed() {
        assert_eq!(HOME_POSITION, (-2, -2, -2));
        assert_ne!(HOME_POSITION.0, 2);
        assert_ne!(HOME_POSITION.1, 2);
        assert_ne!(HOME_POSITION.2, 2);
        assert!(HOME_POSITION.0 < 0 && HOME_POSITION.1 < 0 && HOME_POSITION.2 < 0);
        assert_eq!(PIXELS_PER_UNIT_POT, 9);
        assert_eq!(TEST_SCREEN_RES, (16, 17));
        assert_ne!(TEST_SCREEN_RES, DEFAULT_WINDOW_RES);
        assert_eq!(TEST_SCREEN_RES.1 % 2, 1); // odd height for real-axis craft pins
        assert!(MOVE_SPEED_PPS > 0);
        assert!(SCROLL_SPEED > 0.0);
        // Pan/scroll rates — delete / *→/ / sign flips change feel and break inputs.
        assert_eq!(MOVE_SPEED_IN_SCREENS, 0.42);
        assert!(MOVE_SPEED_IN_SCREENS > 0.0 && MOVE_SPEED_IN_SCREENS < 1.0);
        assert_ne!(MOVE_SPEED_IN_SCREENS, 0.0);
        assert_eq!(NORTH_TIP_RE, "-0.161913425661");
        assert_eq!(NORTH_TIP_IM, "1.035546905361");
        assert_eq!(HEADED_I64_GREY_RE, "-0.2067325560057166");
        assert_eq!(HEADED_I64_GREY_IM, "1.1075689870974698");
        assert_eq!(HEADED_I64_GREY_MAG, 43);
        assert_eq!(SCROLL_SPEED, 40.0);
        assert_ne!(SCROLL_SPEED, -40.0);
        assert_ne!(SCROLL_SPEED, 0.0);
    }
}