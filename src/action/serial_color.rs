use crate::actor::escaper::*;
use crate::action::settings::*;

pub(crate) fn color(value: ScreenValue, settings:& Settings) -> (u8,u8,u8) {
    let color = (0, 0, 0);
    if let Some(instructions) = settings.coloring_script {
        for instruction in instructions {

        }
    }
    return color
}