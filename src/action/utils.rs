use rug::*;
use std::cmp::*;
use std::ops::*;

#[inline]
pub(crate) fn zoom_from_pot(zoom: i32) -> f64 {
    if zoom > 0 {(1 << zoom) as f64} else {1.0 / (1<<-zoom) as f64}
}

#[inline]
pub(crate) fn signed_shift(input: i32, shift: i64) -> i32 {
    (input << ((shift + (shift.abs()))>>1)) >> (-((shift - (shift.abs()))>>1))
    /*if shift >= 0 {
        input << shift
    } else {
        input >> (-shift)
    }*/
}

#[inline]
pub(crate) fn shift(input:i32, shift:i32) -> i32 {
    if shift >= 0 {
        input << shift as u32
    } else {
        input >> (-shift) as u32
    }
}

pub(crate) fn f32_to_i16(input: f32) -> i16 {
    let p = input * (2<<12) as f32;
    p as i16
}

pub(crate) fn i16_to_f32(input: i16) -> f32 {
    let p:f32 = input as f32 / (2<<12) as f32;
    p
}