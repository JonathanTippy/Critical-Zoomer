use crate::utils::IntExp;
use rug::Integer;
use crate::constants::*;
use std::cmp::*;
use std::time::Instant;


#[derive(PartialEq)]
pub(crate) struct PixelStencil {
    pub(crate) location: (IntExp, IntExp, i32) // real, imag, magnification POT
    , pub(crate) resolution: (usize, usize)
}

#[derive(PartialEq)]
pub(crate) struct View<T> {
    pub(crate) stencil: PixelStencil
    , pub(crate) data: Vec<(T)>
    , pub(crate) bitmap: Vec<(u8)>
    // value,
    // 7: exact
    // , 6: representative / estimate from parent pixel
    , pub(crate) updated_at: Instant
}




    pub(crate) const EXACT: u8 = 0b1000_0000;
pub(crate) const EST: u8 = 0b0100_0000;



pub(crate) struct Answer {
    pub(crate) result: MandelbrotResult
    , pub(crate) min_magnitude_time: u64
    , pub(crate) min_magnitude: f64
}

pub(crate) enum MandelbrotResult {
    Outside {
        escape_time: u64
        , escape_location: (f32, f32)
    }
    , Inside {
        period: u64
    }
}
#[derive(Copy, Clone)]
pub(crate) struct Color {
    pub(crate) rgb: (u8, u8, u8)
}