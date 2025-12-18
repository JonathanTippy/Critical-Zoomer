use steady_state::*;

use rand::prelude::*;

use egui::{Color32, Pos2};
use std::cmp::*;

use crate::actor::window::*;
use crate::actor::colorer::*;
use crate::action::utils::*;

use crate::actor::work_controller::PIXELS_PER_UNIT_POT;

#[derive(Clone, Debug)]
pub(crate) struct SamplingContext {
    pub(crate) screen: Option<ZoomerScreen>
    , pub(crate) screen_size: (usize, usize)
    , pub(crate) total_relative_pos: (i32, i32)
    , pub(crate) relative_nuggets: Vec<((i32, i32), i32)>
    , pub(crate) restarted: bool
    , pub(crate) zoom_pot: i32
    , pub(crate) updated: bool
    , pub(crate) mouse_drag_start: Option<((i32, i32), Pos2)>
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ViewportLocation {
    pub(crate) pos: (i32, i32) // This is objective
    , pub(crate) zoom_pot: i32
    , pub(crate) counter: u64
}

pub(crate) fn sample(
    output_buffer: &mut Vec<Color32>,
    sampling_context: &mut SamplingContext
) {

    let bucket = output_buffer;
    let context = sampling_context;

    let size = context.screen_size;
    let min_side = min(context.screen_size.0, context.screen_size.1);
    
    if let Some(current_screen) = &context.screen {

        let res = context.screen_size;

        let data_size = current_screen.res.clone();

        let data_len = current_screen.pixels.len();

        let data = &current_screen.pixels;

        let relative_pos = (
            current_screen.objective_location.pos.0.clone()-context.location.pos.0.clone()
            , current_screen.objective_location.pos.1.clone()-context.location.pos.1.clone()
        );

        let relative_pos_in_pixels:(i32, i32) = (
            relative_pos.0.clone().shift(context.location.zoom_pot).shift(PIXELS_PER_UNIT_POT).into()
            , relative_pos.1.clone().shift(context.location.zoom_pot).shift(PIXELS_PER_UNIT_POT).into()
        );

        let relative_zoom = context.location.zoom_pot - current_screen.objective_location.zoom_pot;

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