use rand::prelude::SliceRandom;
use crate::action::sampling::*;
use crate::action::serialize::*;
use crate::action::utils::*;
use crate::action::workshift::*;
use crate::actor::work_collector::*;

pub(crate) struct NewPoint<T> {
    value: T
    , index: usize
}

pub(crate) struct Collector<T> {
    pub(crate) frame: Frame<T>
    , pub(crate) buffer_frame: Frame<T>
}

impl<T: Default + Copy> Default for Collector<T> {
    fn default() -> Self {

        let mut buffer_frame = Frame::default();
        buffer_frame.pixels.clear();

        Collector{
            frame: Frame::default()
            , buffer_frame
        }
    }
}

impl<T: Clone+Default+Copy> Collector<T> {

    fn collect(&mut self, value: Serial<NewPoint<T>>) {
        match value {
            Serial::Stream{value} => {
                self.insert(value);
            }
            Serial::Resize{res} => {
                println!("buffer is being resized. If you are not resizing the window, this is a bug.");
                let res = *res;
                self.sample_old_values_resize(res);
                self.frame.res = res;
            }
            Serial::Move{loc} => {
                let loc = *loc;
                self.sample_old_values_move(&loc);
                self.frame.objective_location = loc;
            }
        }
    }
    fn insert(&mut self, new: NewPoint<T>) {
        self.frame.pixels[new.index] = new.value
    }

    pub(crate) fn sample_old_values_move(&mut self, new_location: &ObjectivePosAndZoom) {
        self.buffer_frame.pixels.clear();

        //let old_package = self.frame;
        //let new_res = old_package.res;

        let relative_pos = (
            self.frame.objective_location.pos.0.clone()-new_location.pos.0.clone()
            , self.frame.objective_location.pos.1.clone()-new_location.pos.1.clone()
        );

        let relative_pos_in_pixels:(i32, i32) = (
            relative_pos.0.clone().shift(new_location.zoom_pot).shift(crate::actor::work_controller::PIXELS_PER_UNIT_POT).into()
            , relative_pos.1.clone().shift(new_location.zoom_pot).shift(crate::actor::work_controller::PIXELS_PER_UNIT_POT).into()
        );

        let relative_zoom = new_location.zoom_pot - self.frame.objective_location.zoom_pot;

        /*let relative_pos_in_pixels = (
            relative_pos_in_pixels.0 - shift(1, relative_zoom-1)
            , relative_pos_in_pixels.1 - shift(1, relative_zoom-1)
        );*/

        for row in 0..self.frame.res.1 as usize {
            for seat in 0..self.frame.res.0 as usize {
                self.buffer_frame.pixels.push(
                    sample_value(
                        &self.frame.pixels
                        , self.frame.res
                        , self.frame.size
                        , row
                        , seat
                        , relative_pos_in_pixels
                        , relative_zoom as i64
                    )
                );
            }
        }
        std::mem::swap(&mut self.frame, &mut self.buffer_frame);
        self.buffer_frame.pixels.clear();
    }

    pub(crate) fn sample_old_values_resize(&mut self, new_res: (u32, u32)) {

        let size = (new_res.0 * new_res.1) as usize;
        self.buffer_frame = Frame::with_size(size);

        //let old_package_pixel_width = old_package.location.zoom_pot

        let relative_pos_in_pixels:(i32, i32) = (
            0, 0
        );

        let relative_zoom = 0;

        /*let relative_pos_in_pixels = (
            relative_pos_in_pixels.0 - shift(1, relative_zoom-1)
            , relative_pos_in_pixels.1 - shift(1, relative_zoom-1)
        );*/

        for row in 0..new_res.1 as usize {
            for seat in 0..new_res.0 as usize {
                self.buffer_frame.pixels.push(
                    sample_value(
                        &self.frame.pixels
                        , self.frame.res
                        , self.frame.size
                        , row
                        , seat
                        , relative_pos_in_pixels
                        , relative_zoom as i64
                    )
                );
            }
        }
        std::mem::swap(&mut self.frame, &mut self.buffer_frame);
        self.buffer_frame = Frame::with_size(size);
    }
}






#[inline]
fn sample_value<T: Clone>(
    pixels: &Vec<T>
    , data_res: (u32, u32)
    , data_len: usize
    , row: usize
    , seat: usize
    , relative_pos: (i32, i32)
    , relative_zoom_pot: i64
) -> T {
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
            ].clone();
    color
}