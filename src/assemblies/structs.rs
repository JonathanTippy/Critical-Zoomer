use crate::utils::IntExp;
use crate::range::Range;
use rug::Integer;

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

pub(crate) struct PixelStencil {
    location: (IntExp, IntExp, i32)
    , resolution: (usize, usize)
}

pub(crate) struct View<T> {
    stencil: PixelStencil
    , data: Vec<T>
}

impl<T: Copy> View<T> {
    pub(crate) fn is_valid(&self) -> bool {
        self.data.len() == self.stencil.resolution.0 * self.stencil.resolution.1
    }

    pub(crate) fn fill_from(&mut self, new: &Self) {
        assert!(self.is_valid() && new.is_valid());

        let delta = (
            new.stencil.location.0.clone() - self.stencil.location.0.clone()
            , new.stencil.location.1.clone() - self.stencil.location.1.clone()
            , new.stencil.location.2 - self.stencil.location.2
        );


        let (x_pixel_delta, y_pixel_delta, skip): (isize, isize, usize)
            = if delta.2 == 0 {
            (
                delta.0
                ,
                , 1
            )
        } else if delta.2 > 0 {

        } else {

        };

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