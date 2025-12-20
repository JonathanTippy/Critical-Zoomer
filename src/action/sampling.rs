use steady_state::*;

use rand::prelude::*;

use egui::{Color32, Pos2};
use std::cmp::*;

use crate::actor::window::*;
use crate::actor::colorer::*;
use crate::action::utils::*;

use rug::{Float, Integer};
use crate::actor::work_controller::PIXELS_PER_UNIT_POT;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SamplingRelativeTransforms {
    pub(crate) pos: (i32, i32) // this is in pixels. (pixels is pixels is pixels. (sometimes))
    , pub(crate) zoom_pot: i64 // this is evaluated after the relative position during sampling
    , pub(crate) counter: u64
}
#[derive(Clone, Debug)]
pub(crate) struct SamplingContext {
    pub(crate) screen: Option<ZoomerScreen>
    , pub(crate) res: (u32, u32)
    , pub(crate) relative_transforms: SamplingRelativeTransforms
    , pub(crate) mouse_drag_start: Option<MouseDragStart>
    , pub(crate) updated: bool
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ViewportLocation {
    pub(crate) pos: (i32, i32) // This is objective
    , pub(crate) zoom_pot: i32
    , pub(crate) counter: u64
}

pub(crate) fn sample(
    mut command_package: Vec<ZoomerCommand>,
    output_buffer: &mut Vec<Color32>,
    sampling_context: &mut SamplingContext
) {

    let bucket = output_buffer;
    let context = sampling_context;

    let size = context.res;
    let min_side = min(context.res.0, context.res.1);
    // handle commands

    for command in &mut command_package {
        match command {
            ZoomerCommand::SetFocus{pixel_x, pixel_y} => {
            }
            ZoomerCommand::Zoom{pot, center_relative_relative_pos} => {

                let factor:f32;

                if *pot > 0 {
                    factor = (1<<*pot) as f32;
                } else {
                    factor =  1.0 / (1<<-*pot) as f32;
                }

                context.relative_transforms.zoom_pot =
                    context.relative_transforms.zoom_pot + *pot;




                if factor > 1.0 {
                    // adjust position based on zooming (ridiculously hard to think about)
                    context.relative_transforms.pos = (
                        (
                            ((context.relative_transforms.pos.0 as f64) * factor as f64)
                                - (center_relative_relative_pos.0) as f64
                        ) as i32
                        , (
                            ((context.relative_transforms.pos.1 as f64) * factor as f64 )
                                - (center_relative_relative_pos.1) as f64
                        ) as i32
                    );

                } else {

                    // adjust position based on zooming (ridiculously hard to think about)
                    context.relative_transforms.pos = (
                        (
                            ((context.relative_transforms.pos.0 as f64) * factor as f64)
                                + (center_relative_relative_pos.0 as f64 * (factor as f64))
                        ) as i32
                        , (
                            ((context.relative_transforms.pos.1 as f64) * factor as f64 )
                                + (center_relative_relative_pos.1 as f64 * (factor as f64))
                        ) as i32
                    );

                    // if we are zooming out, drop the smallest bit from the transform.

                    context.relative_transforms.pos = (
                        context.relative_transforms.pos.0 - context.relative_transforms.pos.0 % 2
                        , context.relative_transforms.pos.1 - context.relative_transforms.pos.1 % 2
                    );
                }

                match &context.mouse_drag_start {
                    Some(d) => {
                        context.mouse_drag_start = Some(
                            MouseDragStart {
                                screenspace_drag_start: egui::Pos2 {
                                    x: center_relative_relative_pos.0 as f32
                                    , y: center_relative_relative_pos.1 as f32
                                }
                                , relative_transforms: SamplingRelativeTransforms {
                                    pos: context.relative_transforms.pos
                                    , zoom_pot: 0
                                    , counter: 0
                                }
                            });
                    }
                    None => {}
                }

            }
            ZoomerCommand::SetZoom{factor} => {
            }
            ZoomerCommand::Move{pixels_x, pixels_y} => {
                context.relative_transforms.pos = (
                    context.relative_transforms.pos.0
                        + *pixels_x, context.relative_transforms.pos.1
                        + *pixels_y
                )
            }
            ZoomerCommand::MoveTo{x, y} => {
                context.relative_transforms.pos =
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


    if let Some(current_screen) = &context.screen {
        // go over the sampling size in rows and seats, and sample the colors

        /*info!("zoom: {}, location: {} + {}i"
        , current_screen.objective_location.zoom_pot
        , current_screen.objective_location.pos.0
        , current_screen.objective_location.pos.1
    );*/

        // go over the sampling size in rows and seats, and sample the colors

        let res = context.res;

        let data_size = current_screen.screen_size.clone();

        let data_len = current_screen.pixels.len();

        let data = &current_screen.pixels;

        let relative_pos = context.relative_transforms.pos;

        let factor:f64;

        if context.relative_transforms.zoom_pot > 0 {
            factor = (1<<context.relative_transforms.zoom_pot) as f64;
        } else {
            factor =  1.0 / (1<<-context.relative_transforms.zoom_pot) as f64;
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
                        , min_side
                        , data_size
                        , data_len
                        , row
                        , seat
                        //, res_recip
                        , min_side_recip
                        , relative_pos
                        , context.relative_transforms.zoom_pot
                    )
                );
                //i+=1;
            }
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
    , min_side: u32
    , data_res: (u32, u32)
    , data_len: usize
    , row: usize
    , seat: usize
    //, res_recip: (u32, u32)
    , min_side_recip: i64
    , relative_pos: (i32, i32)
    , relative_zoom_pot: i64
) -> Color32 {
    let color =
        pixels[
            index_from_relative_location(
                transform_relative_location_i32(
                    relative_location_i32_row_and_seat(seat, row)
                    , (relative_pos.0, relative_pos.1)
                    , relative_zoom_pot
                )
                , data_res
                , data_len
            )
        ];
    Color32::from_rgb(color.0, color.1, color.2)
}


#[inline]
pub(crate) fn relative_location_i32_row_and_seat(seat: usize, row: usize) -> (i32, i32) {

    let seat = seat as u32;
    let row = row as u32;

    (
        seat as i32
        , row as i32
    )

}

#[inline]
pub(crate) fn index_from_relative_location(l: (i32, i32), data_res: (u32, u32), data_length: usize) -> usize {

    let normalized_l = (
        max(min(l.0, (data_res.0-1) as i32), 0)
        , max(min(l.1, (data_res.1-1) as i32), 0)
        );

    let i =
        (
            (normalized_l.1 as u32 * data_res.0)
            + normalized_l.0 as u32
        ) as usize;

    i
}

#[inline]
pub(crate) fn optional_index_from_relative_location(l: (i32, i32), data_res: (u32, u32), data_length: usize) -> Option<usize> {

    if l.0 >= 0 && l.0 <= (data_res.0-1) as i32 && l.1 >= 0 && l.1 <= (data_res.1-1) as i32 {
        let i =
            (
                (l.1 as u32 * data_res.0)
                    + l.0 as u32
            ) as usize;

        Some(i)
    } else {None}

}

#[inline]
pub(crate) fn transform_relative_location_i32(l: (i32, i32), m: (i32, i32), zoom: i64) -> (i32, i32) {
    // move + zoom

    (
        signed_shift(l.0 - m.0, -zoom)
        , signed_shift(l.1 - m.1, -zoom)
    )
}

pub(crate) fn update_sampling_context(context: &mut SamplingContext, screen: ZoomerScreen) {

    if context.location == screen.objective_location {
        context.updated = false;
    }
    
    /*if let Some(old_screen) = context.screen.take() {
        drop(old_screen);
    }*/
    context.screen = Some(screen);

}