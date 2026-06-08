use crate::utils::IntExp;
use crate::range::Range;
use rug::Integer;
use crate::constants::*;
use std::cmp::*;

// The stencil defines the set of complex points that make up a view.
// It is used with a vec equal to resolution.0 * resolution.1 in length.
// The top left sample of the view is taken exactly at location.0, location.1
// Other samples apply a regular grid,
// following imaginary coordinates: down is negative and right is positive.
// In complex plane terms: +seat moves +real; +row moves −imag.
// Scanning map between vec and pixels is done right then down, like a CRT.
// The points are equally spaced vertically and horizontally.
// The default points per unit is defined by the PIXELS_PER_UNIT_POT constant.
// The zoom level (location.2) is added to the constant to get the current PPU POT.
// The actual spacing distance between points is given by 1/(2^(PPU POT)).

fn line_segments_overlap(a: (IntExp, IntExp), b: (IntExp, IntExp)) -> bool {
    // left edge inclusive right edge limit
    (a.0 >= b.0 && a.0 < b.1)
    || (a.1 > b.0 && a.1 < b.1)
}


pub(crate) struct PixelStencil {
    location: (IntExp, IntExp, i32) // real, imag, magnification POT
    , resolution: (usize, usize)
}

impl PixelStencil {
    fn space(&self) -> IntExp {
        let one = IntExp::from(1);
        one.shift(-(self.location.2 + PIXELS_PER_UNIT_POT))
    }

    fn corners(&self) -> ((IntExp, IntExp), (IntExp, IntExp)) {

        let top_left: (IntExp, IntExp) = (self.location.0.clone(), self.location.1.clone());

        let bottom_right: (IntExp, IntExp) = (
            self.location.0.clone() + self.space() * IntExp::from(self.resolution.0)
                , self.location.1.clone() - self.space() * IntExp::from(self.resolution.1)
                );
        (
            top_left
                , bottom_right
                )

    }
    fn overlaps(&self, other: &Self) -> bool {
        line_segments_overlap(
            (self.corners().0.0, self.corners().1.0)
                , (other.corners().0.0, other.corners().1.0)
                ) && line_segments_overlap(
            (self.corners().0.1, self.corners().1.1)
                , (other.corners().0.1, other.corners().1.1)
                )
    }
}

pub(crate) struct View<T> {
    stencil: PixelStencil
    , data: Vec<(T, bool, bool)> // value, exact, representative
}

impl<T: Copy> View<T> {
    pub(crate) fn is_valid(&self) -> bool {
        self.data.len() == self.stencil.resolution.0 * self.stencil.resolution.1
    }

    pub(crate) fn fill_from(&mut self, new: &Self) {
        assert!(self.is_valid() && new.is_valid());

        if !(
            self.stencil.space() > new.stencil.space()
                * IntExp::from(max(new.stencil.resolution.0, new.stencil.resolution.1))
            || new.stencil.space() > self.stencil.space()
                * IntExp::from(max(self.stencil.resolution.0, self.stencil.resolution.1))
        ) {

        } else {

        }

        let delta = (
            new.stencil.location.0.clone() - self.stencil.location.0.clone()
            , new.stencil.location.1.clone() - self.stencil.location.1.clone()
            , new.stencil.location.2 - self.stencil.location.2
        );

        let (pan_pixel_delta, ratio_step): ((isize, isize), usize)
            = (
                (
                    delta.0.shift(self.stencil.location.2 + PIXELS_PER_UNIT_POT).into()
                    , (IntExp::from(0) - delta.1).shift(self.stencil.location.2 + PIXELS_PER_UNIT_POT).into()
                )
                , if delta.2 == 0 {
                    1
                } else if delta.2 > 0 {
                    1 << delta.2
                } else {
                    1 << -delta.2
                }
            );

        if self_to_new.0 == 1 {

        } else {

        }

        for row in 0..self.stencil.resolution.1 {
            for seat in 0..self.stencil.resolution.0 {
                self.data[row * self.stencil.resolution.0 + seat]
                = new.data[
                    row * self.stencil.resolution.0 + seat
                    ]
            }
        }
    }

    fn seat_to_index_clamped(stencil:PixelStencil, seat_and_row: (isize, isize)) -> usize {
        let seat_and_row = (
            seat_and_row.0.clamp(0, stencil.resolution.0 as isize - 1) as usize
            , seat_and_row.1.clamp(0, stencil.resolution.1 as isize - 1) as usize
            );
        let index = seat_and_row.1 * stencil.resolution.0 + seat_and_row.0;
        index
    }
}

pub(crate) struct Answer {
    pub(crate) result: MandelbrotResult
    ,
    pub(crate) min_magnitude_time: u64
    ,
    pub(crate) min_magnitude: f64
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

pub(crate) struct Color {
    pub(crate) rgb: (u8, u8, u8)
}