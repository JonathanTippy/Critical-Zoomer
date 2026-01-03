use crate::action::utils::*;

pub(crate) struct WorkContext {
    active_point: PointInProgress
    , pos: (i32, i32)
    , res: (u32, u32)
}

use std::cmp::*;
pub(crate) fn get_pixel_side_length(res: (i32, i32), zoom: i32) -> f64 {
    let significant_res = min(res.0, res.1);
    let scaling_factor_pot = significant_res.ilog2() as i32;
    2.0f64.powf(-((zoom + scaling_factor_pot) as f64))
}


pub(crate) struct PointBuilder {
    screen_top_left_corner_location:(f64, f64)
    , pixel_side_length: f64
}

impl PointBuilder {
    pub(crate) fn new(view: ObjectivePosAndZoom, res: (i32, i32)) -> PointBuilder {
        PointBuilder {
            screen_top_left_corner_location: (view.pos.0.into(), view.pos.1.into())
            , pixel_side_length: get_pixel_side_length(res, view.zoom_pot)
        }
    }
    pub(crate) fn start_point(&self, screen_pos: (i32, i32)) -> PointInProgress {
        PointInProgress::new(
            (
                self.screen_top_left_corner_location.0 + self.pixel_side_length * screen_pos.0 as f64
                , self.screen_top_left_corner_location.1 - self.pixel_side_length * screen_pos.1 as f64
            )
        )
    }
}

#[derive(PartialEq)]
pub(crate) struct Z {
    real: f64
    , imag: f64
    , real_squared: f64
    , imag_squared: f64
}

pub(crate) enum IterationResult {
    Incomplete{}
    , Escaped{}
    , Repeated{}
}

impl IterationResult {
    fn is_incomplete(&self) -> bool {
        match self {
            IterationResult::Incomplete{..}=>{true}
            _=>{false}
        }
    }
}

struct Halter {
    location: (f64, f64)
    , iteration_count: u64
}

pub(crate) struct PointInProgress {
    c: (f64, f64)
    , z: Z
    , iteration_count: u64
    , halter: Halter
    , min_magnitude_squared: f64
    , fat_period: u64
}

impl PointInProgress {

    fn new(c:(f64,f64)) -> Self {
        PointInProgress {
            c
            , z:Z{real:0.0, imag:0.0, real_squared: 0.0, imag_squared: 0.0}
            , iteration_count: 0
            , halter: Halter{location: (0.0,0.0), iteration_count: 0}
            , min_magnitude_squared: f64::INFINITY
            , fat_period: 1
        }
    }

    fn iterate(&mut self, bailout_radius_squared:f64) -> IterationResult {

        self.z.real = self.z.real_squared - self.z.imag_squared
            + self.c.0;
        self.z.imag = self.z.real * self.z.imag
            + self.c.1;

        self.z.real_squared = self.z.real * self.z.real;
        self.z.imag_squared = self.z.imag * self.z.imag;

        if !self.did_escape(bailout_radius_squared) {} else {return IterationResult::Escaped{}}
        self.update_smallness();

        if !self.did_repeat() {} else {return IterationResult::Repeated{}}
        self.update_halter();

        IterationResult::Incomplete{}
    }

    fn did_escape(&self, bailout_radius_squared:f64) -> bool {
        let magnitude_squared = self.z.real_squared + self.z.imag_squared;
        return magnitude_squared > bailout_radius_squared
    }

    fn did_repeat(&self) -> bool {
        (self.z.real, self.z.imag) == self.halter.location
    }

    fn update_halter(&mut self) {
        if self.iteration_count >> 1 >= self.halter.iteration_count {
            self.halter.location = (self.z.real, self.z.imag)
        }
    }

    fn update_smallness(&mut self) {
        let magnitude_squared = self.z.real_squared + self.z.imag_squared;
        if magnitude_squared >= self.min_magnitude_squared {} else {
            self.min_magnitude_squared = magnitude_squared;
            self.fat_period = self.iteration_count;
        }
    }
}

