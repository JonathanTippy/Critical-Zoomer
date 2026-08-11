
// UNDERIVED CONSTANTS

pub const DEFAULT_WINDOW_RES:(u32, u32) = (854, 480);
/// Standard lib-test screen size. Live app keeps `DEFAULT_WINDOW_RES`.
pub const TEST_SCREEN_RES: (u32, u32) = (40, 71);
pub const HOME_POSITION:(i32, i32, i32) = (-2, -2, -2);
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
        assert_eq!(TEST_SCREEN_RES, (40, 71));
        assert_ne!(TEST_SCREEN_RES, DEFAULT_WINDOW_RES);
        assert!(MOVE_SPEED_PPS > 0);
        assert!(SCROLL_SPEED > 0.0);
        // Pan/scroll rates — delete / *→/ / sign flips change feel and break inputs.
        assert_eq!(MOVE_SPEED_IN_SCREENS, 0.42);
        assert!(MOVE_SPEED_IN_SCREENS > 0.0 && MOVE_SPEED_IN_SCREENS < 1.0);
        assert_ne!(MOVE_SPEED_IN_SCREENS, 0.0);
        assert_eq!(SCROLL_SPEED, 40.0);
        assert_ne!(SCROLL_SPEED, -40.0);
        assert_ne!(SCROLL_SPEED, 0.0);
    }
}