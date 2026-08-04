// read delivery.md for project context
use rug::Integer;
use crate::assemblies::headgroup::window::sampling::ZoomerCommand;
use crate::constants::PIXELS_PER_UNIT_POT;
use crate::intexp::*;
use crate::utils::ObjectivePosAndZoom;

const MAX_PAN_PIXELS: i32 = 48;

pub fn advance_navigation(
    target: &(IntExp, IntExp, i32)
    , location: &ObjectivePosAndZoom
    , screen_size: (usize, usize)
) -> Option<Vec<ZoomerCommand>> {
    let target_pos = (target.0.clone(), IntExp::ZERO - target.1.clone());
    let center = (
        screen_size.0 as i32 / 2
        , screen_size.1 as i32 / 2
    );
    if location.zoom_pot < target.2 {
        return Some(vec![ZoomerCommand::Zoom {
            pot: 1
            , center_screenspace_pos: center
        }]);
    }
    if location.zoom_pot > target.2 {
        return Some(vec![ZoomerCommand::Zoom {
            pot: -1
            , center_screenspace_pos: center
        }]);
    }
    let delta = (
        target_pos.0.clone() - location.pos.0.clone()
        , target_pos.1.clone() - location.pos.1.clone()
    );
    if delta.0 == IntExp::ZERO && delta.1 == IntExp::ZERO {
        return None;
    }
    let pixels_x = delta.0.clone().shift(location.zoom_pot).shift(PIXELS_PER_UNIT_POT);
    let pixels_y = delta.1.clone().shift(location.zoom_pot).shift(PIXELS_PER_UNIT_POT);
    let px = clamp_pixel_step(pixels_x.into());
    let py = clamp_pixel_step(pixels_y.into());
    if px == 0 && py == 0 {
        return Some(vec![ZoomerCommand::SetPos {
            real: target.0.clone()
            , imag: target.1.clone()
        }]);
    }
    Some(vec![ZoomerCommand::Move {
        pixels_x: IntExp { val: Integer::from(px), exp: 0 }
        , pixels_y: IntExp { val: Integer::from(py), exp: 0 }
    }])
}

fn clamp_pixel_step(v: i32) -> i32 {
    if v > MAX_PAN_PIXELS {
        MAX_PAN_PIXELS
    } else if v < -MAX_PAN_PIXELS {
        -MAX_PAN_PIXELS
    } else {
        v
    }
}
