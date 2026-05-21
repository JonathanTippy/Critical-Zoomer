pub(crate) const DEFAULT_WINDOW_RES:(u32, u32) = (800, 480);
pub(crate) const DEFAULT_FRAME_SIZE:usize = (DEFAULT_WINDOW_RES.0 * DEFAULT_WINDOW_RES.1) as usize;
pub(crate) const HOME_POSITION:(i32, i32, i32) = (-2, -2, -2);

/// Flat purple placeholder before the colorer sends a frame (window / sampling preview holes only).
pub(crate) const WINDOW_IDK_RGB: (u8, u8, u8) = (128, 0, 128);

/// Minecraft missing-texture checkerboard (`ScreenValue::Idk`, colorer only): magenta `#FF00FF`.
pub(crate) const IDK_RGB: (u8, u8, u8) = (255, 0, 255);
/// Minecraft missing-texture checkerboard: black `#000000`.
pub(crate) const IDK_RGB_LIGHT: (u8, u8, u8) = (0, 0, 0);

/// Screen-space tile size in pixels for the idk checkerboard (64×64, Minecraft-style).
pub(crate) const IDK_CHECKER_TILE_SIZE: u32 = 64;

#[inline]
pub(crate) fn idk_checkerboard_rgb(x: u32, y: u32) -> (u8, u8, u8) {
    let dark = ((x / IDK_CHECKER_TILE_SIZE) + (y / IDK_CHECKER_TILE_SIZE)) & 1 == 0;
    if dark { IDK_RGB } else { IDK_RGB_LIGHT }
}
