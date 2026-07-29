use crate::intexp::*;
use rug::Integer;
use crate::constants::*;
use std::cmp::*;
pub mod views;
pub mod stencil;
pub mod tile;
pub mod gpu_tile;

pub use tile::*;
pub use gpu_tile::*;

use crate::range::*;

#[derive(PartialEq, Clone, Debug)]
pub struct PointStencil {
    pub homothety: (IntExp, IntExp, i32) // real, imag, magnification POT
    , pub resolution: (usize, usize)
    , pub serial_number: u64
    , pub focus: Option<(usize, usize)>
    , pub hover: Option<(usize, usize)>
    , pub mag_velocity: f64 // D-STEN-1: EWMA magnification velocity (bumps/s).
}

impl PointStencil {
    pub fn retarget_with_seq(
        &mut self,
        homothety: (crate::intexp::IntExp, crate::intexp::IntExp, i32),
        resolution: (usize, usize),
        hover: Option<(usize, usize)>,
        mag_velocity: f64,
    ) {
        let changed = self.homothety != homothety
            || self.resolution != resolution
            || self.hover != hover
            || self.mag_velocity != mag_velocity;
        self.homothety = homothety;
        self.resolution = resolution;
        self.hover = hover;
        self.mag_velocity = mag_velocity;
        if changed {
            self.serial_number = self.serial_number.saturating_add(1);
        }
    }
}


#[derive(PartialEq, Clone, Debug)]
pub struct View<T> {
    pub stencil: PointStencil
    , pub data: Vec<(T)>
    , pub alignment: Vec<(u8)>
    // 7: exact / original: aligned with original C value
    // , 6: representative / proximate estimate: not aligned with original C value
    // , 5: native: was computed at the stencil magnification
    // , 4: done: all fields were completed at some magnification
}

pub const EXACT: u8 = 0b1000_0000;
pub const PROX: u8 = 0b0100_0000;
pub const NATIVE: u8 = 0b0010_0000;
pub const DONE: u8 = 0b0001_0000;


#[derive(Copy, Clone, Debug)]

pub struct Answer {
    pub result: MandelbrotResult
    , pub min_magnitude_time: u64
    , pub min_magnitude: f64
    // Derivative-magnitude slope angle (D-SCH-3); 0..=255, 0 when unknown.
    , pub escape_time_angle: u8
    , pub min_magnitude_angle: u8
}

impl Answer {
    pub const TESTVAL: Answer = Answer {
        result: MandelbrotResult::Outside {
            escape_time_r2: 0
            , escape_z: (0.0, 0.0)
        }
        , min_magnitude_time: 0
        , min_magnitude: 0.0
        , escape_time_angle: 0
        , min_magnitude_angle: 0
    };
}

#[derive(Copy, Clone, Debug)]
pub enum MandelbrotResult {
    Outside {
        escape_time_r2: u64
        , escape_z: (f32, f32)
    }
    , Inside {
        period: u64
    }
}

