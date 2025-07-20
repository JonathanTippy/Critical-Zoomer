use steady_state::*;

use rand::prelude::*;

use egui::{Color32, Vec2};
use std::cmp::min;

use crate::actor::window::*;
use crate::actor::colorer::*;

#[derive(Clone, Debug)]
pub(crate) struct SamplingContext {
    pub(crate) screens: Vec<ZoomerScreen>
    , pub(crate) sampling_size: (u32, u32)
    , pub(crate) relative_pos: (i32, i32) // these are updated in response to commands. pos is in terms of pixels on the screen.
    , pub(crate) relative_zoom_pot: i8
    , pub(crate) objective_zoom_pot: i64
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
    let min_side = min(context.sampling_size.0, context.sampling_size.1);
    // handle commands

    for command in &mut command_package {
        match command {
            ZoomerCommand::SetFocus{pixel_x, pixel_y} => {
            }
            ZoomerCommand::Zoom{pot, center_relative_relative_pos} => {

                let mut factor:f32;

                if *pot > 0 {
                    factor = (1<<*pot) as f32;
                } else {
                    factor =  1.0 / (1<<-*pot) as f32;
                }

                context.relative_zoom_pot =
                    context.relative_zoom_pot + *pot;




                if factor > 1.0 {
                    context.relative_pos = (
                        (((context.relative_pos.0 as f64) * factor as f64) - (center_relative_relative_pos.0) as f64) as i32 // + ((center_relative_relative_pos.0 - (1<<15)) as f64 / *factor as f64) as i32)
                        , (((context.relative_pos.1 as f64) * factor as f64 ) - (center_relative_relative_pos.1) as f64) as i32 //  + ((center_relative_relative_pos.1 - (1<<15)) as f64 / *factor as f64) as i32)
                    );
                } else {


                    // adjust position based on zooming (ridiculously hard to think about)
                    context.relative_pos = (
                        (((context.relative_pos.0 as f64) * factor as f64) + (center_relative_relative_pos.0 as f64 * (factor as f64))) as i32 // + ((center_relative_relative_pos.0 - (1<<15)) as f64 / *factor as f64) as i32)
                        , (((context.relative_pos.1 as f64) * factor as f64 ) + (center_relative_relative_pos.1 as f64 * (factor as f64))) as i32 //  + ((center_relative_relative_pos.1 - (1<<15)) as f64 / *factor as f64) as i32)
                    );

                    // if we are zooming out, we need to make sure we gently guide pixels to be in the proper position when they need to reach full resolution.

                    context.relative_pos = (
                        context.relative_pos.0 - context.relative_pos.0 % 2
                        , context.relative_pos.1 - context.relative_pos.1 % 2
                    );
                }

            }
            ZoomerCommand::SetZoom{factor} => {
            }
            ZoomerCommand::Move{pixels_x, pixels_y} => {
            }
            ZoomerCommand::MoveTo{x, y} => {
                context.relative_pos =
                    (*x, *y);
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

    // go over the sampling size in rows and seats, and sample the

    let data_size = context.screens[0].screen_size.clone();

    let data_len = data_size.0 * data_size.1;

    let data = &context.screens[0].pixels;

    let relative_pos = context.relative_pos;


    let mut factor:f64;

    if context.relative_zoom_pot > 0 {
        factor = (1<<context.relative_zoom_pot) as f64;
    } else {
        factor =  1.0 / (1<<-context.relative_zoom_pot) as f64;
    }

    let relative_zoom_recip = ((1.0 / factor) * ((1<<16) as f64)) as u32;

    let min_side_recip = (1<<32) / (min_side as i64);
    //let res_recip = (     (1<<16) / size.0,    (1<<16) / size.1    );


    //info!("data res: {}, {}", data_size.0, data_size.1);


    //let mut i = 0;
    for row in 0..size.1 as usize {
        for seat in 0..size.0 as usize {
            bucket.push(
                sample_color(
                    data
                    , data_size
                    , data_len
                    , row
                    , seat
                    //, res_recip
                    , min_side_recip
                    , relative_pos
                    , relative_zoom_recip
                )
            );
            //i+=1;
        }
    }
}

//screen space uses fixed point i32, 1<<16 is 1.
//multiplication results in an extra 1<<16 which means we have to >> 16
//addition is fine as long as all values invloved are already fixed points
//division cancels the 1<<16 so we have to add it back with << 16

#[inline]
fn sample_color(
    pixels: &Vec<(u8,u8,u8)>
    , data_res: (u32, u32)
    , data_len: u32
    , row: usize
    , seat: usize
    //, res_recip: (u32, u32)
    , min_side_recip: i64
    , relative_pos: (i32, i32)
    , relative_zoom_recip: u32
) -> Color32 {
    let color =
        pixels[
            index_from_fixed_point(
                relative_location_to_fixed_point(
                    transform_relative_location_i32(
                        relative_location_i32_row_and_seat(seat, row)
                        , (relative_pos.0, relative_pos.1)
                        , relative_zoom_recip
                    )
                    , min_side_recip
                )
                , data_res
                , data_len
            )
        ];
    Color32::from_rgb(color.0, color.1, color.2)
}


#[inline]
fn relative_location_i32_row_and_seat(seat: usize, row: usize) -> (i32, i32) {

    let seat = seat as u32;
    let row = row as u32;

    (
        seat as i32
        , row as i32
    )

}

#[inline]
fn relative_location_to_fixed_point(l: (i32, i32), min_side_recip: i64) -> (i64, i64) {

    (
        l.0 as i64 * min_side_recip
        , l.1 as i64 * min_side_recip
    )

}


#[inline]
fn index_from_fixed_point(l: (i64, i64), data_res: (u32, u32), data_length: u32) -> usize {

    //let data_res = (1024, 1024);
    //let data_length = (data_res.0 * data_res.0);

    let l = (
        ((l.0 * data_res.0 as i64) >> 32) as u32
            , ((l.1 * data_res.1 as i64) >> 32) as u32
    );


    let i =
        ((l.1 * data_res.0)
            + l.0) as u64
        ;

    //info!("data length: {}, data res: {}, {}", data_length, data_res.0, data_res.1);
    //info!("input: {}, {}\noutput: {}", l.0 as f64 / (1u64<<32) as f64, l.1 as f64 / (1u64<<32) as f64, i);

    if i < (data_length as u64) {i as usize} else {
        (i % (data_length as u64)) as usize
    }

}

#[inline]
fn transform_relative_location_i32(l: (i32, i32), m: (i32, i32), zoom_recip: u32) -> (i32, i32) {
    // move + zoom

    (
        (((l.0 - m.0) as i64 * zoom_recip as i64) >> 16)  as i32
        , (((l.1 - m.1) as i64 * zoom_recip as i64) >> 16) as i32
    )
}


pub(crate) fn update_sampling_context(context: &mut SamplingContext, screen: ZoomerScreen) {


    /*let new_relative_location;
    if screen.relative_zoom_of_predecessor < 0 {
        new_relative_location = (
            (context.relative_pos.0 << -screen.relative_zoom_of_predecessor) + screen.relative_location_of_predecessor.0
            , (context.relative_pos.1 << -screen.relative_zoom_of_predecessor) + screen.relative_location_of_predecessor.1
        );
    } else {
        new_relative_location = (
            (context.relative_pos.0 >> screen.relative_zoom_of_predecessor) + screen.relative_location_of_predecessor.0
            , (context.relative_pos.1 >> screen.relative_zoom_of_predecessor) + screen.relative_location_of_predecessor.1
        );
    }

    context.relative_pos = new_relative_location;
    context.relative_zoom_pot = (context.relative_zoom_pot as i64 - screen.relative_zoom_of_predecessor) as i8;
*/

    context.relative_pos = (0, 0);
    context.relative_zoom_pot = 0;

    if context.screens.len() != 0 {
        drop(context.screens.pop().unwrap());
    }
    context.screens.push(screen);
}