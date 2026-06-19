use steady_state::*;

use rand::prelude::*;

use egui::{Color32, Pos2};
use std::cmp::*;

//use crate::actor::window::*;
use crate::assemblies::shadergroup::colorer::*;
use crate::utils::*;

use rug::{Float, Integer};
use crate::constants::PIXELS_PER_UNIT_POT;

use crate::assemblies::structs::*;
pub enum ZoomerCommand {
    SetFocus { pixel_x: u32, pixel_y: u32 }
    ,
    SetZoom { pot: i32 }
    ,
    Zoom { pot: i32, center_screenspace_pos: (i32, i32) } // zoom in or out
    ,
    Move { pixels_x: IntExp, pixels_y: IntExp }
    ,
    MoveTo { x: IntExp, y: IntExp }
    ,
    SetPos { real: IntExp, imag: IntExp }
    ,
    TrackPoint { point_id: u64, point_real: IntExp, point_imag: IntExp }
    ,
    UntrackPoint { point_id: u64 }
    ,
    UntrackAllPoints
}
pub const NUMBER_OF_COMMANDS: u16 = 10;

#[derive(Clone, Debug)]
pub struct SamplingContext {
    pub screen: Option<View<Color32>>
    , pub screen_size: (u32, u32)
    , pub location: ObjectivePosAndZoom
    , pub updated: bool
    , pub mouse_drag_start: Option<(ObjectivePosAndZoom, Pos2)>
}

#[derive(Clone, Debug, PartialEq)]
pub struct ViewportLocation {
    pub pos: (i32, i32) // This is objective
    , pub zoom_pot: i32
    , pub counter: u64
}

pub fn sample(
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

                let pixel_width = IntExp{val: Integer::from(1), exp:-context.location.zoom_pot}.shift(-PIXELS_PER_UNIT_POT);

                context.location.pos = (
                    context.location.pos.0.clone()
                        + IntExp{val: Integer::from(center_screenspace_pos.0), exp: -context.location.zoom_pot}.shift(-PIXELS_PER_UNIT_POT)
                        - (pixel_width.clone() >> 1)
                    , context.location.pos.1.clone()
                        + IntExp{val: Integer::from(center_screenspace_pos.1), exp: -context.location.zoom_pot }.shift(-PIXELS_PER_UNIT_POT)
                        - (pixel_width.clone() >> 1)
                );

                context.location.zoom_pot += *pot;

                let pixel_width = IntExp{val: Integer::from(1), exp:-context.location.zoom_pot}.shift(-PIXELS_PER_UNIT_POT);

                context.location.pos = (
                    context.location.pos.0.clone()
                        - IntExp{val: Integer::from(center_screenspace_pos.0), exp: -context.location.zoom_pot}.shift(-PIXELS_PER_UNIT_POT)
                        + (pixel_width.clone() >> 1)
                    , context.location.pos.1.clone()
                        - IntExp{val: Integer::from(center_screenspace_pos.1), exp: -context.location.zoom_pot }.shift(-PIXELS_PER_UNIT_POT)
                        + (pixel_width.clone() >> 1)
                );

                // round position to not be more precise than necessary

                if *pot < 0 {
                    context.location.pos = (
                        context.location.pos.0.clone().round((-*pot) as usize)
                        , context.location.pos.1.clone().round((-*pot) as usize)
                    );
                }


                // reset mouse drag start to the new screenspace location
                // theoretically this is not necessary as objective position
                // of mouse drag start will always remain attached to mouse
                // current position.
                // mouse screenspace position should be invariant under zoom
                // as the mouse's screenspace position is the zoom center.

                /*match &context.mouse_drag_start {
                    Some(d) => {
                        context.mouse_drag_start = Some(
                            (
                                /*ObjectivePosAndZoom{
                                    pos: context.location.pos.clone()
                                    , zoom_pot: context.location.zoom_pot
                                }*/
                                d.0.clone()
                                , egui::Pos2 {
                                x: center_screenspace_pos.0 as f32
                                , y: center_screenspace_pos.1 as f32
                            }
                            ));
                    }
                    None => {}
                }*/


                context.updated = true;

            }
            ZoomerCommand::SetZoom{pot} => {
                context.location.zoom_pot = *pot;
                context.updated = true;
            }
            ZoomerCommand::Move{pixels_x, pixels_y} => {
                context.location.pos = (
                    context.location.pos.0.clone() + pixels_x.clone().shift(-context.location.zoom_pot).shift(-PIXELS_PER_UNIT_POT)
                    , context.location.pos.1.clone() + pixels_y.clone().shift(-context.location.zoom_pot).shift(-PIXELS_PER_UNIT_POT)
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


    if let Some(current_screen) = &context.screen {
        // go over the sampling size in rows and seats, and sample the colors

        /*info!("zoom: {}, location: {} + {}i"
        , current_screen.objective_location.zoom_pot
        , current_screen.objective_location.pos.0
        , current_screen.objective_location.pos.1
    );*/

        let res = context.screen_size;

        let data_size = current_screen.stencil.resolution.clone();

        let data_len = current_screen.data.len();

        let data = &current_screen.data;

        let relative_pos = (
            current_screen.stencil.location.0.clone()-context.location.pos.0.clone()
            , current_screen.stencil.location.1.clone()-context.location.pos.1.clone()
        );

        let relative_pos_in_pixels:(i32, i32) = (
            relative_pos.0.clone().shift(context.location.zoom_pot).shift(PIXELS_PER_UNIT_POT).into()
            , relative_pos.1.clone().shift(context.location.zoom_pot).shift(PIXELS_PER_UNIT_POT).into()
        );

        let relative_zoom = context.location.zoom_pot - current_screen.stencil.location.2;

        /*let relative_pos_in_pixels = (
            relative_pos_in_pixels.0 + shift(1, relative_zoom-1)
            , relative_pos_in_pixels.1 + shift(1, relative_zoom-1)
        );*/

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
                        , (data_size.0 as u32, data_size.1 as u32)
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
}

//screen space uses fixed point i32, 1<<16 is 1.
//multiplication results in an extra 1<<16 which means we have to >> 16
//addition is fine as long as all values invloved are already fixed points
//division cancels the 1<<16 so we have to add it back with << 16

#[inline]
fn sample_color(
    pixels: &Vec<Color32>
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
    color
}


#[inline]
pub fn relative_location_i32_row_and_seat(seat: usize, row: usize) -> (i32, i32) {

    let seat = seat as u32;
    let row = row as u32;

    (
        seat as i32
        , row as i32
    )

}

#[inline]
pub fn index_from_relative_location(l: (i32, i32), data_res: (u32, u32), data_length: usize) -> usize {

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
pub fn optional_index_from_relative_location(l: (i32, i32), data_res: (u32, u32), data_length: usize) -> Option<usize> {

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
pub fn transform_relative_location_i32(l: (i32, i32), m: (i32, i32), zoom: i64) -> (i32, i32) {
    // move + zoom

    (
        signed_shift(l.0 - m.0, -zoom)
        , signed_shift(l.1 - m.1, -zoom)
    )
}

pub fn update_sampling_context(context: &mut SamplingContext, screen: View<Color32>) {

    let l = ObjectivePosAndZoom {
        pos: (screen.stencil.clone().location.0, screen.stencil.clone().location.1)
        , zoom_pot: screen.stencil.clone().location.2
    };

    if context.location == l {
        context.updated = false;
    }
    
    /*if let Some(old_screen) = context.screen.take() {
        drop(old_screen);
    }*/
    context.screen = Some(screen);

}