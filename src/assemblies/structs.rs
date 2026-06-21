use crate::utils::IntExp;
use rug::Integer;
use crate::constants::*;
use std::cmp::*;
use std::time::Instant;


#[derive(PartialEq, Clone, Debug)]
pub struct PointStencil {
    pub location: (IntExp, IntExp, i32) // real, imag, magnification POT
    , pub resolution: (usize, usize)
    , pub serial_number: u64
}

#[derive(PartialEq, Clone, Debug)]
pub struct View<T> {
    pub stencil: PointStencil
    , pub data: Vec<(T)>
    , pub bitmap: Vec<(u8)>
    // value,
    // 7: exact
    // , 6: representative / estimate from parent pixel
    // , 5: result is final/done/complete
}




pub const EXACT: u8 = 0b1000_0000;
pub const PROX: u8 = 0b0100_0000;
pub const DONE: u8 = 0b0010_0000;

#[derive(Copy, Clone)]

pub struct Answer {
    pub result: MandelbrotResult
    , pub min_magnitude_time: u64
    , pub min_magnitude: f64
}

impl Answer {
    pub const TESTVAL: Answer = Answer {
        result: MandelbrotResult::Outside {
            escape_time_r2: 0
            , escape_z: (0.0, 0.0)
        }
        , min_magnitude_time: 0
        , min_magnitude: 0.0
    };
}

#[derive(Copy, Clone)]
pub enum MandelbrotResult {
    Outside {
        escape_time_r2: u64
        , escape_z: (f32, f32)
    }
    , Inside {
        period: u64
    }
}