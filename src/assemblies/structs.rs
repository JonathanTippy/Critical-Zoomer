use crate::utils::IntExp;
use rug::Integer;
use crate::constants::*;
use std::cmp::*;
use std::time::Instant;


#[derive(PartialEq)]
pub struct PixelStencil {
    pub location: (IntExp, IntExp, i32) // real, imag, magnification POT
    , pub resolution: (usize, usize)
}

#[derive(PartialEq)]
pub struct View<T> {
    pub stencil: PixelStencil
    , pub data: Vec<(T)>
    , pub bitmap: Vec<(u8)>
    // value,
    // 7: exact
    // , 6: representative / estimate from parent pixel
    , pub updated_at: Instant
}




    pub const EXACT: u8 = 0b1000_0000;
pub const EST: u8 = 0b0100_0000;



pub struct Answer {
    pub result: MandelbrotResult
    , pub min_magnitude_time: u64
    , pub min_magnitude: f64
}

pub enum MandelbrotResult {
    Outside {
        escape_time: u64
        , escape_location: (f32, f32)
    }
    , Inside {
        period: u64
    }
}
#[derive(Copy, Clone)]
pub struct Color {
    pub rgb: (u8, u8, u8)
}