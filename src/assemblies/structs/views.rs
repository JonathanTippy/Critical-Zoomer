// See comment at end

use rug::Integer;
use crate::assemblies::structs::*;
use crate::constants::*;
use crate::intexp::*;

fn line_segments_overlap(a: (IntExp, IntExp), b: (IntExp, IntExp)) -> bool {
    // left edge inclusive right edge limit
    (a.0 >= b.0 && a.0 < b.1)
        || (a.1 > b.0 && a.1 < b.1)
}

fn line_segment_a_is_subset_of_b(a: (IntExp, IntExp), b: (IntExp, IntExp)) -> bool {
    // left edge inclusive right edge limit
    (a.0 >= b.0 && a.0 < b.1)
        && (a.1 > b.0 && a.1 < b.1)
}

impl PointStencil {

    pub fn correct_precision(self) -> Self {
        PointStencil {
            homothety:(self.homothety.0.clone().set_precision(PIXELS_PER_UNIT_POT+self.homothety.2)
                       , self.homothety.1.clone().set_precision(PIXELS_PER_UNIT_POT +self.homothety.2), self.homothety.2)
            , resolution: self.resolution
            , serial_number: self.serial_number
            , focus: self.focus
            , hover: self.hover
            , mag_velocity: self.mag_velocity
        }
    }
    pub fn assert_validity(&self) {
        assert!(
            self.homothety.0.exp == -(self.homothety.2 + PIXELS_PER_UNIT_POT)
            && self.homothety.0.exp == self.homothety.1.exp
            , "Invalid Stencil: POT zoom level and precision exponents must match."
        );
        assert!(
            self.resolution.0 < 1 << 16 && self.resolution.1 < 1 << 16
            , "Invalid Stencil: No resolution side length may exceed 2^16 pixels."
        );
        assert!(
            self.resolution.0 > 0 && self.resolution.1 > 0
            , "Invalid Stencil: No resolution side length may be 0 pixels."
        );
    }
    pub fn index(&self, seat_and_row: (isize, isize)) -> usize {
        debug_assert!(
            seat_and_row.0 >= 0 && seat_and_row.0 < self.resolution.0 as isize
                && seat_and_row.1 >= 0 && seat_and_row.1 < self.resolution.1 as isize
            , "Index Failure: nonexistent seat."
        );
        seat_and_row.1 as usize * self.resolution.0 + seat_and_row.0 as usize
    }
    pub fn seat_and_row(&self, index: usize) -> (usize, usize) {
        debug_assert!(
            index < self.resolution.0 * self.resolution.1
            , "Index Failure: nonexistent seat."
        );
        (index % self.resolution.0, index / self.resolution.0)
    }

    pub fn clamp_seat_and_row(&self, seat_and_row: (isize, isize)) -> (isize, isize) {
        return (
            seat_and_row.0.clamp(0, self.resolution.0 as isize - 1)
            , seat_and_row.1.clamp(0, self.resolution.1 as isize - 1)
        );
    }

    pub fn bottom_right_point(&self) -> (IntExp, IntExp) {
        let space = IntExp::from(1).shift(-self.homothety.2 - PIXELS_PER_UNIT_POT);
        return (
            self.homothety.0.clone() + space.clone() * IntExp::from(self.resolution.0-1)
            , self.homothety.1.clone() - space * IntExp::from(self.resolution.1-1)
        )
    }


    pub fn corners(&self) -> ((IntExp, IntExp), (IntExp, IntExp)) {
        let top_left: (IntExp, IntExp) = (self.homothety.0.clone(), self.homothety.1.clone());

        let bottom_right: (IntExp, IntExp) = (
            self.homothety.0.clone() + self.space() * IntExp::from(self.resolution.0)
            , self.homothety.1.clone() - self.space() * IntExp::from(self.resolution.1)
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

    fn subset_of(&self, other: &Self) -> bool {
        line_segment_a_is_subset_of_b(
            (self.corners().0.0, self.corners().1.0)
            , (other.corners().0.0, other.corners().1.0)
        ) && line_segment_a_is_subset_of_b(
            (self.corners().0.1, self.corners().1.1)
            , (other.corners().0.1, other.corners().1.1)
        )
    }
}


impl<T: Copy + Clone> View<T> {
    pub fn new(stencil: PointStencil, fill_value: T) -> View<T> {
        let returned = View {
            stencil: stencil.clone().correct_precision()
            ,
            data: vec!(fill_value; stencil.resolution.0 * stencil.resolution.1)
            ,
            alignment: vec!(0u8; stencil.resolution.0 * stencil.resolution.1)

        };
        returned.assert_validity();
        returned
    }

    pub fn new_custom(stencil: PointStencil, fill_value: T, fill_alignment: u8) -> View<T> {
        let returned = View {
            stencil: stencil.clone().correct_precision()
            , data: vec!(fill_value; stencil.resolution.0 * stencil.resolution.1)
            , alignment: vec!(fill_alignment; stencil.resolution.0 * stencil.resolution.1)
        };
        returned.assert_validity();
        returned
    }
}

impl<T: Copy> View<T> {
    pub fn assert_validity(&self) {
        self.stencil.assert_validity();
        assert_eq!(
            self.data.len(), self.stencil.resolution.0 * self.stencil.resolution.1
            , "Invalid View: Data length must equal seats times rows."
        );
        assert_eq!(
            self.data.len(),  self.alignment.len()
            , "Invalid View: Data length must equal bitmap length."
        )
    }
}

// Conventions:
// location.2 is magnification which is not the precision exponent.
// magnification goes up as you zoom in.
// Usually, when seat and row go in a tuple together, the order is seat then row.
// This is to align better with the x then y standard order.
// W/H and Width / Height are banned. This project uses seats and rows,
// and anytime both dimensions are together, a tuple called resolution.
//
// The stencil defines the set of complex points that make up a screen sample grid.
// Top-left sample is exactly at location.0, location.1; +seat → +real; +row → −imag.
// Spacing is 1/(2^(PIXELS_PER_UNIT_POT + mag_pot)).
//
// Zoom-fill / View::fill_from is a dead design idea. Tiles are static; each frame the
// sampling shader maps static tile data onto the stencil. Do not rewrite answer data
// under pan/zoom. Live path: ingest GPU tiles → sample → shade.
