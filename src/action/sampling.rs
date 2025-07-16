use steady_state::*;

use rand::prelude::*;

use egui::{Color32, Vec2};
use std::cmp::min;

use crate::actor::window::*;
use crate::actor::colorer::*;


#[derive(Clone, Debug)]
pub(crate) struct ZoomerWorldColors {
    screens: Vec<ZoomerScreen>,
    state_revision: u64,
}


pub(crate) struct ZoomedScreen {
    pixels: Vec<(u8, u8, u8)>
    , zoom_power: i32
}


#[derive(Clone, Debug)]
pub(crate) struct SamplingContext {
    pub(crate) used_screen: Vec<ZoomerScreen>
    , pub(crate) unused_screen: Vec<ZoomerScreen>
    , pub(crate) sampling_size: (u32, u32)
    /*pub(crate) world: ZoomerWorldColors
    , pub(crate) viewport_position_real: &'static str
    , pub(crate) viewport_position_imag: &'static str
    , pub(crate) viewport_zoom: &'static str
    , pub(crate) zoom_power_base: u8
    , pub(crate) window_res: (u32, u32)*/
}

pub(crate) fn sample(
    mut command_package: ZoomerCommandPackage,
    mut output_buffer: &mut Vec<Color32>,
    mut sampling_context: &mut SamplingContext
) {

    let mut bucket = output_buffer;
    let mut context = sampling_context;

    let size = context.sampling_size;
    // handle commands

    for command in &mut command_package.commands {
        match command {
            ZoomerCommand::SetAttention{pixel_x, pixel_y} => {
                // send the jobs and stuff
            }
            ZoomerCommand::ZoomClean{factor_power} => {
            }
            ZoomerCommand::SetZoomPowerBase{base} => {
            }
            ZoomerCommand::ZoomUnclean{factor} => {
            }
            ZoomerCommand::SetZoom{factor} => {
            }
            ZoomerCommand::MoveClean{pixels_x, pixels_y} => {
            }
            ZoomerCommand::SetPos{real, imag} => {
            }
            ZoomerCommand::TrackPoint{point_id, point_real, point_imag} => {
            }
            ZoomerCommand::UntrackPoint{point_id} => {
            }
            ZoomerCommand::UntrackAllPoints{} => {
            }
        }
    }

    let psize = size.0 * size.1;

    let data_size = context.used_screen[0].pixels.len()-1;

    let adjusted_rate = data_size as f64 / psize as f64;

    for i in 0..psize {
        bucket.push(Color32::BLACK);
    }

    for i in 0..data_size {
        let l = objective_location_from_index(i as u32, context.used_screen[0].screen_size);
        let color = context.used_screen[0].pixels[i];
        let j = index_from_objective_location(l, size) as usize;

        //if j < bucket.len() {
            bucket[j] = Color32::from_rgb(color.0, color.1, color.2);
       // }

    }
}

#[inline]
fn objective_location_from_index(i: u32, res: (u32, u32)) -> (f64, f64) {
    (
        (i % res.0) as f64 / res.0 as f64
        , (i / res.0) as f64 / res.1 as f64
    )
}
/*#[inline]
fn index_from_objective_location(l: (f64, f64), res: (u32, u32)) -> u32 {
    ((l.0 * res.0 as f64) + (l.1 * res.1 as f64) * res.0 as f64) as u32
}*/

#[inline]
fn index_from_objective_location(l: (f64, f64), res: (u32, u32)) -> u32 {
    let col = (l.0 * res.0 as f64).floor() as u32; // x * width, floored to get column
    let row = (l.1 * res.1 as f64).floor() as u32; // y * height, floored to get row
    row * res.0 + col                              // index = row * width + col
}