use steady_state::*;

use rand::prelude::*;

use egui::{Color32, Vec2};
use std::cmp::min;

use crate::actor::window::*;
use crate::actor::colorer::*;
use crate::action::utils::*;

#[derive(Clone, Debug)]
pub(crate) struct SamplingContext {
    pub(crate) screens: Vec<ZoomerScreen>
    , pub(crate) sampling_size: (u32, u32)
    , pub(crate) relative_transforms: SamplingRelativeTransforms
}

#[derive(Clone, Debug)]
pub(crate) struct SamplingRelativeTransforms {
    pub(crate) pos: (i32, i32) // this is in pixels. (pixels is pixels is pixels. (sometimes))
    , pub(crate) zoom_pot: i64 // this is evaluated after the relative position during sampling
    , pub(crate) counter: u64
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

                context.relative_transforms.zoom_pot =
                    context.relative_transforms.zoom_pot + *pot;




                if factor > 1.0 {
                    context.relative_transforms.pos = (
                        (((context.relative_transforms.pos.0 as f64) * factor as f64) - (center_relative_relative_pos.0) as f64) as i32 // + ((center_relative_relative_pos.0 - (1<<15)) as f64 / *factor as f64) as i32)
                        , (((context.relative_transforms.pos.1 as f64) * factor as f64 ) - (center_relative_relative_pos.1) as f64) as i32 //  + ((center_relative_relative_pos.1 - (1<<15)) as f64 / *factor as f64) as i32)
                    );
                } else {


                    // adjust position based on zooming (ridiculously hard to think about)
                    context.relative_transforms.pos = (
                        (((context.relative_transforms.pos.0 as f64) * factor as f64) + (center_relative_relative_pos.0 as f64 * (factor as f64))) as i32 // + ((center_relative_relative_pos.0 - (1<<15)) as f64 / *factor as f64) as i32)
                        , (((context.relative_transforms.pos.1 as f64) * factor as f64 ) + (center_relative_relative_pos.1 as f64 * (factor as f64))) as i32 //  + ((center_relative_relative_pos.1 - (1<<15)) as f64 / *factor as f64) as i32)
                    );

                    // if we are zooming out, we need to make sure we gently guide pixels to be in the proper position when they need to reach full resolution.

                    context.relative_transforms.pos = (
                        context.relative_transforms.pos.0 - context.relative_transforms.pos.0 % 2
                        , context.relative_transforms.pos.1 - context.relative_transforms.pos.1 % 2
                    );
                }

            }
            ZoomerCommand::SetZoom{factor} => {
            }
            ZoomerCommand::Move{pixels_x, pixels_y} => {
                context.relative_transforms.pos = (context.relative_transforms.pos.0 + *pixels_x, context.relative_transforms.pos.1 + *pixels_y)
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

    // go over the sampling size in rows and seats, and sample the colors

    let res = context.sampling_size;

    let data_size = context.screens[0].screen_size.clone();

    let data_len = context.screens[0].pixels.len();

    let data = &context.screens[0].pixels;

    let relative_pos = context.relative_transforms.pos;


    let mut factor:f64;

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
fn relative_location_i32_row_and_seat(seat: usize, row: usize) -> (i32, i32) {

    let seat = seat as u32;
    let row = row as u32;

    (
        seat as i32
        , row as i32
    )

}

#[inline]
fn index_from_relative_location(l: (i32, i32), data_res: (u32, u32), data_length: usize) -> usize {

    let i =
        (
            (min(l.1 as u32, data_res.1-1) * data_res.0)
            + min(l.0 as u32, data_res.0-1)
        ) as usize;

    min(i, data_length-1)
    // ^ technically this min can be removed but somehow it makes it feel smoother
}

#[inline]
fn transform_relative_location_i32(l: (i32, i32), m: (i32, i32), zoom: i64) -> (i32, i32) {
    // move + zoom

    (
        signed_shift(l.0 - m.0, -zoom)
        , signed_shift(l.1 - m.1, -zoom)
    )
}

pub(crate) fn update_sampling_context(state: &mut WindowState, screen: ZoomerScreen) {

    if !screen.dummy {

        if state.sampling_context.relative_transforms.counter == screen.originating_relative_transforms.counter {

            let screen_originating_relative_zoom = zoom_from_pot(screen.originating_relative_transforms.zoom_pot);

            state.sampling_context.relative_transforms.zoom_pot = state.sampling_context.relative_transforms.zoom_pot - screen.originating_relative_transforms.zoom_pot;

            let zoom = zoom_from_pot(state.sampling_context.relative_transforms.zoom_pot);

            info!("updating relative pos to {}, {} based on counter number {}"
            , state.sampling_context.relative_transforms.pos.0 - screen.originating_relative_transforms.pos.0
            , state.sampling_context.relative_transforms.pos.1 - screen.originating_relative_transforms.pos.1
            , screen.originating_relative_transforms.counter
            );

            state.sampling_context.relative_transforms.pos = (
                // take the pre-existing offset, and move it to where the old data is now.
                state.sampling_context.relative_transforms.pos.0 - screen.originating_relative_transforms.pos.0// as f64 / zoom) as i32
                , state.sampling_context.relative_transforms.pos.1 - screen.originating_relative_transforms.pos.1// as f64 / zoom) as i32
            );



            match &state.mouse_drag_start {
                Some(d) => {
                    // take the pre-existing drag start point, and move it to where the old data is now.

                    state.mouse_drag_start = Some( MouseDragStart{
                        screenspace_drag_start: d.screenspace_drag_start
                        , relative_transforms: SamplingRelativeTransforms{
                            pos: (
                                d.relative_transforms.pos.0 - screen.originating_relative_transforms.pos.0
                                ,   d.relative_transforms.pos.1 - screen.originating_relative_transforms.pos.1
                            )
                            , zoom_pot: d.relative_transforms.zoom_pot
                            , counter: state.sampling_context.relative_transforms.counter
                        }
                    });
                }
                None => {}
            }

            if state.sampling_context.screens.len() != 0 {
                drop(state.sampling_context.screens.pop().unwrap());
                state.sampling_context.screens.push(screen);
            } else {
                state.sampling_context.screens.push(screen);
            }

            info!("updating transform revision counter to {}", state.sampling_context.relative_transforms.counter + 1);
            state.sampling_context.relative_transforms.counter = state.sampling_context.relative_transforms.counter + 1;

        } else {
            info!("counter mismatch; waiting for worker to catch up.")
        }
    } else {
        state.sampling_context.relative_transforms.counter = state.sampling_context.relative_transforms.counter + 1;
    }
}