use std::convert::TryFrom;
use std::ops::*;

use crate::intexp::*;

pub trait Mandelbrotable:
Copy
+ PartialOrd
+ Add<Output=Self>
+ Sub<Output=Self>
+ Mul<Output=Self>
+ From<IntExp>
{
    const ZERO: Self;
    const ONE: Self;
    const TWO: Self;

    fn from_u16(value: u16) -> Self;
    fn to_f32(self) -> f32;
    fn to_f64(self) -> f64;
}

impl Mandelbrotable for f32 {
    const ZERO: Self = 0.0;
    const ONE: Self = 1.0;
    const TWO: Self = 2.0;

    fn from_u16(value: u16) -> Self {
        value as f32
    }

    fn to_f32(self) -> f32 {
        self
    }

    fn to_f64(self) -> f64 {
        self as f64
    }
}


impl Mandelbrotable for f64 {
    const ZERO: Self = 0.0;
    const ONE: Self = 1.0;
    const TWO: Self = 2.0;

    fn from_u16(value: u16) -> Self {
        value as f64
    }

    fn to_f32(self) -> f32 {
        self as f32
    }

    fn to_f64(self) -> f64 {
        self
    }

}

