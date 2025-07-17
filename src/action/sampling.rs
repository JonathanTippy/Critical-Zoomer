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


#[derive(Clone, Debug)]
pub(crate) struct SamplingContext {
    pub(crate) used_screen: Vec<ZoomerScreen>
    , pub(crate) unused_screen: Vec<ZoomerScreen>
    , pub(crate) sampling_size: (u32, u32)
    , pub(crate) relative_pos: (i32, i32) // these are updated in response to commands
    , pub(crate) relative_zoom: f64
    , pub(crate) objective_pos: (String, String) // these are just retrieved from the data
    , pub(crate) objective_zoom: (String)
    //pub(crate) world: ZoomerWorldColors
    //, pub(crate) zoom_power_base: u8
}

pub(crate) fn sample(
    mut command_package: Vec<ZoomerCommand>,
    mut output_buffer: &mut Vec<Color32>,
    mut sampling_context: &mut SamplingContext
) {

    let mut bucket = output_buffer;
    let mut context = sampling_context;

    let size = context.sampling_size;
    // handle commands

    for command in &mut command_package {
        match command {
            ZoomerCommand::SetFocus{pixel_x, pixel_y} => {
            }
            ZoomerCommand::ZoomClean{factor, center_relative_relative_pos} => {

                context.relative_zoom =
                    context.relative_zoom * *factor as f64;

                let center_relative_pos = (
                    context.relative_pos + *center_relative_relative_pos.0
                    , context.relative_pos + *center_relative_relative_pos.1
                );

                context.relative_pos = (
                    (context.relative_pos.0 - center_relative_pos) * factor + center_relative_pos.0
                    , (context.relative_pos.1 - center_relative_pos) * factor + center_relative_pos.1
                )

            }
            ZoomerCommand::ZoomUnclean{factor} => {

            }
            ZoomerCommand::SetZoom{factor} => {
            }
            ZoomerCommand::Move{pixels_x, pixels_y} => {


            }
            ZoomerCommand::MoveTo{x, y} => {



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

    //let mut i = 0;
    for row in 0..size.1 as usize {
        for seat in 0..size.0 as usize {
            bucket.push(get_color(
                &context.used_screen[0].pixels
                , context.used_screen[0].screen_size
                , context.sampling_size
                , row
                , seat
                , (     (1<<16) / size.0,    (1<<16) / size.1    )
                , context.sampling_relative_pos
                , context.sampling_relative_zoom
            ));
            //i+=1;
        }
    }
}

//screen space uses fixed point i32, 1<<16 is 1.
//multiplication results in an extra 1<<16 which means we have to >> 16
//addition is fine as long as all values invloved are already fixed points
//division cancels the 1<<16 so we have to add it back with << 16

#[inline]
fn get_color(pixels: &Vec<(u8,u8,u8)>, data_res: (u32, u32), res: (u32, u32), row: usize, seat: usize, res_recip: (u32, u32), relative_pos: (i32, i32), relative_zoom: f64) -> Color32 {
    let color = pixels
        [
            index_from_relative_location_i32(
                transform_relative_location_i32(
                    relative_location_i32_row_and_seat(res_recip, seat, row)
                    , (relative_pos.0, relative_pos.1)
                    , relative_zoom
                )
                , data_res
            )
        ];
    Color32::from_rgb(color.0, color.1, color.2)
}


#[inline]
fn relative_location_i32_row_and_seat(res_recip: (u32, u32), seat: usize, row: usize) -> (i32, i32) {

    let seat = seat as u32;
    let row = row as u32;

    (
        (seat * res_recip.0) as i32
        , (row * res_recip.1) as i32
    )

}

#[inline]
fn index_from_relative_location_i32(l: (i32, i32), res: (u32, u32)) -> usize {

    let l = (
        l.0 % (1<<16)
        , l.1 % (1<<16)
    );

    let pixel_l = (
        (l.0 as u32 * res.0) >> 16
        , (l.1 as u32 * res.1) >> 16
    );

    ((
        pixel_l.1 * res.0
            + pixel_l.0
    ) % (res.0 * res.1)) as usize

}

#[inline]
fn transform_relative_location_i32(l: (i32, i32), m: (i32, i32), zoom: f64) -> (i32, i32) {


    // move + apply modulo

    let l = (
        (l.0 - m.0) % (1<<16)
        , (l.1 - m.1) % (1<<16)
    );

    let centered = (
        l.0 - zc.0
        , l.1 - zc.1
    );

    let centered_zoomed= (
        (centered.0 as f64 * zoom) as i32
        , (centered.1 as f64 * zoom) as i32
    );

    let uncentered_zoomed =(
        (centered_zoomed.0 + zc.0) % (1<<16)
        , (centered_zoomed.1 + zc.1) % (1<<16)
    );

    uncentered_zoomed
}