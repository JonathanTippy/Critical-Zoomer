pub(crate) const DEFAULT_WINDOW_RES:(u32, u32) = (800, 480);
pub(crate) const DEFAULT_FRAME_SIZE:usize = (DEFAULT_WINDOW_RES.0 * DEFAULT_WINDOW_RES.1) as usize;
pub(crate) const HOME_POSITION:(i32, i32, i32) = (-2, -2, -2);

/// Dark tile of the idk checkerboard (`ScreenValue::Idk`, pan holes, pre-first-frame fill).
pub(crate) const IDK_RGB: (u8, u8, u8) = (128, 0, 128);
/// Light tile of the idk checkerboard (Minecraft-style missing-texture pair with `IDK_RGB`).
pub(crate) const IDK_RGB_LIGHT: (u8, u8, u8) = (200, 128, 200);

/// Screen-space tile size in pixels for the idk checkerboard (8×8).
pub(crate) const IDK_CHECKER_TILE_SIZE: u32 = 8;

#[inline]
pub(crate) fn idk_checkerboard_rgb(x: u32, y: u32) -> (u8, u8, u8) {
    let dark = ((x / IDK_CHECKER_TILE_SIZE) + (y / IDK_CHECKER_TILE_SIZE)) & 1 == 0;
    if dark { IDK_RGB } else { IDK_RGB_LIGHT }
}
