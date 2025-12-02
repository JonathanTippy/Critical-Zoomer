use steady_state::*;

use rand::prelude::*;

use egui::{Color32, Pos2};
use std::cmp::min;

use crate::actor::window::*;
use crate::actor::colorer::*;
use crate::action::utils::*;

use rug::{Float, Integer};
use crate::actor::work_controller::PIXELS_PER_UNIT_POT;

#[derive(Clone, Debug)]
pub(crate) struct SamplingContext {
    pub(crate) screens: Vec<ZoomerScreen>
    , pub(crate) screen_size: (u32, u32)
    , pub(crate) location: ObjectivePosAndZoom
    , pub(crate) updated: bool
    , pub(crate) mouse_drag_start: Option<(ObjectivePosAndZoom, Pos2)>
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

    let size = context.screen_size;
    let min_side = min(context.screen_size.0, context.screen_size.1);
    // handle commands

    for command in &mut command_package {
        match command {
            ZoomerCommand::SetFocus{pixel_x, pixel_y} => {
            }
            ZoomerCommand::Zoom{pot, center_screenspace_pos} => {

                /*let center_centered_pos = (
                    center_screenspace_pos.0 + (context.screen_size.0/2) as i32
                    , center_screenspace_pos.1 + (context.screen_size.1/2) as i32
                );*/


                // adjust position & zoom based on zooming in 3 steps
                // step 1: move to zoom center
                // step 2: zoom
                // step 3: move back so zoom center falls on same screenspace location

                context.location.pos = (
                    context.location.pos.0.clone()
                        + IntExp{val: Integer::from(center_screenspace_pos.0), exp: -context.location.zoom_pot}.shift(-PIXELS_PER_UNIT_POT)
                    , context.location.pos.1.clone()
                        + IntExp{val: Integer::from(center_screenspace_pos.1), exp: -context.location.zoom_pot }.shift(-PIXELS_PER_UNIT_POT)
                );

                context.location.zoom_pot += *pot;


                context.location.pos = (
                    context.location.pos.0.clone()
                        - IntExp{val: Integer::from(center_screenspace_pos.0), exp: -context.location.zoom_pot}.shift(-PIXELS_PER_UNIT_POT)
                    , context.location.pos.1.clone()
                        - IntExp{val: Integer::from(center_screenspace_pos.1), exp: -context.location.zoom_pot }.shift(-PIXELS_PER_UNIT_POT)
                );

                // reset mouse drag start to the new screenspace location

                match &context.mouse_drag_start {
                    Some(d) => {
                        context.mouse_drag_start = Some(
                            (
                                ObjectivePosAndZoom{
                                    pos: context.location.pos.clone()
                                    , zoom_pot: context.location.zoom_pot
                                }
                                , egui::Pos2 {
                                x: center_screenspace_pos.0 as f32
                                , y: center_screenspace_pos.1 as f32
                            }
                            ));
                    }
                    None => {}
                }


                context.updated = true;

            }
            ZoomerCommand::SetZoom{pot} => {
                context.location.zoom_pot = *pot;
                context.updated = true;
            }
            ZoomerCommand::Move{pixels_x, pixels_y} => {
                context.location.pos = (
                    context.location.pos.0.clone() + IntExp::from(*pixels_x).shift(-context.location.zoom_pot).shift(-PIXELS_PER_UNIT_POT)
                    , context.location.pos.1.clone() + IntExp::from(*pixels_y).shift(-context.location.zoom_pot).shift(-PIXELS_PER_UNIT_POT)
                );
                context.updated = true;
            }
            ZoomerCommand::MoveTo{x, y} => {
                context.location.pos =
                    (x.clone(), y.clone());
                context.updated = true;
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

    let res = context.screen_size;

    let data_size = context.screens[0].screen_size.clone();

    let data_len = context.screens[0].pixels.len();

    let data = &context.screens[0].pixels;

    let relative_pos = (
        context.screens[0].objective_location.pos.0.clone()-context.location.pos.0.clone()
        , context.screens[0].objective_location.pos.1.clone()-context.location.pos.1.clone()
    );

    let relative_pos_in_pixels:(i32, i32) = (
        relative_pos.0.shift(context.location.zoom_pot).shift(PIXELS_PER_UNIT_POT).into()
, relative_pos.1.shift(context.location.zoom_pot).shift(PIXELS_PER_UNIT_POT).into()
        );

    let relative_zoom = context.location.zoom_pot - context.screens[0].objective_location.zoom_pot;

    let factor:f64;

    if relative_zoom > 0 {
        factor = (1<<relative_zoom) as f64;
    } else {
        factor =  1.0 / (1<<-relative_zoom) as f64;
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
                    , relative_pos_in_pixels
                    , relative_zoom as i64
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

pub(crate) fn update_sampling_context(context: &mut SamplingContext, screen: ZoomerScreen) {

    if context.location == screen.objective_location {
        context.updated = false;
    }
    
    
    if context.screens.len() != 0 {
        drop(context.screens.pop().unwrap());
        context.screens.push(screen);
    } else {
        context.screens.push(screen);
    }
    
    
}