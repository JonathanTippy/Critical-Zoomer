use crate::utils::IntExp;
use crate::range::Range;
use rug::Integer;
use crate::constants::*;
use std::cmp::*;

// Conventions:
// location.2 is magnification which is not the precision exponent.
// magnification goes up as you zoom in.
// Usually, when seat and row go in a tuple together, the order is seat then row.
// This is to align better with the x then y standard order.


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

// When filling one View from another, pixels are considered to represent:
// the area from their top left corner (inclusive) to their bottom right corner (limit).
// inexact mappings of larger to smaller are thusly fully defined.
// The complex plane is effectively divided into squares
// , where every smaller & larger pair where larger contains smaller can map small (choose top left) -> large or large (top left) -> many small

// The method to find at least one exactly mapped pixel if one exists is to check:
// 1. overlap (do the frame areas touch at all?) -> overlapping corner(s)
// 2. compatibility
// (does the relative offset contain units smaller than the smaller space? if so, no exact matches.)

// mapping is exact when one mapped exact pixel is identified,
// and the larger pixel step off of that pixel yields pixels still represented in the smaller pixel view.



#[derive(PartialEq)]
pub(crate) struct PixelStencil {
    location: (IntExp, IntExp, i32) // real, imag, magnification POT
    , resolution: (usize, usize)
}

impl PixelStencil {
    pub(crate) fn is_valid(&self) -> bool {
        self.location.0.exp == -(self.location.2 + PIXELS_PER_UNIT_POT)
        && self.location.0.exp == self.location.1.exp

        && self.resolution.0 < 2<<16 && self.resolution.1 < 2<<16
        && self.resolution.0 > 0 && self.resolution.1 > 0
    }
    fn index(&self, seat_and_row: (isize, isize)) -> usize {
        assert!(
            seat_and_row.0 >= 0 && seat_and_row.0 < self.resolution.0 as isize
            && seat_and_row.1 >= 0 && seat_and_row.1 < self.resolution.1 as isize
            , "Index Failure: nonexistent seat."
        );
        seat_and_row.1 as usize * self.resolution.0 + seat_and_row.0 as usize
    }
    fn clamp_seat_and_row(&self, seat_and_row: (isize, isize)) -> (isize, isize) {
        return (
            seat_and_row.0.clamp(0,self.resolution.0 as isize-1)
            , seat_and_row.1.clamp(0,self.resolution.1 as isize-1)
        );
    }
}

#[derive(PartialEq)]
pub(crate) struct View<T> {
    stencil: PixelStencil
    , data: Vec<(T, u8)> // value, 7: exact, 6: representative
}

const EXACT: u8 = 0b1000_0000;
const EST: u8 = 0b0100_0000;


impl<T: Copy> View<T> {
    pub(crate) fn is_valid(&self) -> bool {
        self.data.len() == self.stencil.resolution.0 * self.stencil.resolution.1
        && self.stencil.is_valid()
    }

    pub(crate) fn fill_from(&mut self, source: &Self) {
        assert!(self.is_valid() && source.is_valid(), "Views must be the correct length and valid resolutions and locate themselves on pixel grid.");

        let screenspace_delta = (
            self.stencil.location.0.clone() - source.stencil.location.0.clone()
            , IntExp::ZERO - (self.stencil.location.1.clone() - source.stencil.location.1.clone())
            , self.stencil.location.2 - source.stencil.location.2
        );

        match screenspace_delta.2.cmp(&0) {
            Ordering::Equal => {

                let pan_pixel_delta: (isize, isize) = (
                    screenspace_delta.0.shift(self.stencil.location.2 + PIXELS_PER_UNIT_POT)
                        .clamp(IntExp::from(isize::MIN), IntExp::from(isize::MAX)).into()
                    , (screenspace_delta.1).shift(self.stencil.location.2 + PIXELS_PER_UNIT_POT)
                        .clamp(IntExp::from(isize::MIN), IntExp::from(isize::MAX)).into()
                );

                for row in 0..self.stencil.resolution.1 {
                    for seat in 0..self.stencil.resolution.0 {

                        let preferred_source_seat_row = (
                            seat as isize + pan_pixel_delta.0
                            , row as isize + pan_pixel_delta.1
                        );

                        let clamped_source_seat_row = source
                            .stencil
                            .clamp_seat_and_row(preferred_source_seat_row);

                        let representative = preferred_source_seat_row == clamped_source_seat_row;
                        let value = source.data[source.stencil.index(clamped_source_seat_row)];
                        let exact = representative && value.1 & EXACT == EXACT;

                        let new_value = (
                            value.0
                            , {if exact {EXACT} else {0}} + {if representative {EST} else {0}}
                        );

                        self.data[self.stencil.index((seat as isize, row as isize))]
                        = new_value;
                    }
                }
            }
            , Ordering::Greater => {
                if screenspace_delta.2 < 16 {

                    let pan_self_pixel_delta: (isize, isize) = (
                        screenspace_delta.0.clone().shift(self.stencil.location.2 + PIXELS_PER_UNIT_POT)
                            .clamp(IntExp::from(isize::MIN), IntExp::from(isize::MAX)).into()
                        , (screenspace_delta.1.clone()).shift(self.stencil.location.2 + PIXELS_PER_UNIT_POT)
                            .clamp(IntExp::from(isize::MIN), IntExp::from(isize::MAX)).into()
                    );

                    let pan_source_pixel_delta: (isize, isize) = (
                        screenspace_delta.0.clone().shift(source.stencil.location.2 + PIXELS_PER_UNIT_POT)
                            .clamp(IntExp::from(isize::MIN), IntExp::from(isize::MAX)).into()
                        , (screenspace_delta.1.clone()).shift(source.stencil.location.2 + PIXELS_PER_UNIT_POT)
                            .clamp(IntExp::from(isize::MIN), IntExp::from(isize::MAX)).into()
                    );

                    let phase = (
                        pan_self_pixel_delta.0 - (pan_source_pixel_delta.0 << screenspace_delta.2)
                        , pan_self_pixel_delta.1 - (pan_source_pixel_delta.1 << screenspace_delta.2)
                    );

                    let frequency = 1 << screenspace_delta.2;

                    for row in 0..self.stencil.resolution.1 {
                        for seat in 0..self.stencil.resolution.0 {

                            let preferred_source_seat_row = (
                                (seat as isize + pan_self_pixel_delta.0) >> screenspace_delta.2
                                , (row as isize + pan_self_pixel_delta.1) >> screenspace_delta.2
                            );

                            let aligned = (seat as isize - phase.0) % frequency == 0
                            && (row as isize - phase.1) % frequency == 0;

                            let clamped_source_seat_row = source
                                .stencil
                                .clamp_seat_and_row(preferred_source_seat_row);

                            let representative = preferred_source_seat_row == clamped_source_seat_row;
                            let value = source.data[source.stencil.index(clamped_source_seat_row)];
                            let exact = representative && representative && value.1 & EXACT == EXACT && aligned;

                            let new_value = (
                                value.0
                                , { if exact { EXACT } else { 0 } } + { if representative { EST } else { 0 } }
                            );

                            self.data[self.stencil.index((seat as isize, row as isize))]
                                = new_value;
                        }
                    }
                } else {

                }
            }
            , Ordering::Less => {
                if screenspace_delta.2 > -16 {

                    let pan_source_pixel_delta: (isize, isize) = (
                        screenspace_delta.0.clone().shift(source.stencil.location.2 + PIXELS_PER_UNIT_POT)
                            .clamp(IntExp::from(isize::MIN), IntExp::from(isize::MAX)).into()
                        , (screenspace_delta.1.clone()).shift(source.stencil.location.2 + PIXELS_PER_UNIT_POT)
                            .clamp(IntExp::from(isize::MIN), IntExp::from(isize::MAX)).into()
                    );

                    let pan_self_pixel_delta: (isize, isize) = (
                        screenspace_delta.0.clone().shift(self.stencil.location.2 + PIXELS_PER_UNIT_POT)
                            .clamp(IntExp::from(isize::MIN), IntExp::from(isize::MAX)).into()
                        , (screenspace_delta.1.clone()).shift(self.stencil.location.2 + PIXELS_PER_UNIT_POT)
                            .clamp(IntExp::from(isize::MIN), IntExp::from(isize::MAX)).into()
                    );

                    let phase = (
                        pan_source_pixel_delta.0 - (pan_self_pixel_delta.0 << screenspace_delta.2)
                        , pan_source_pixel_delta.1 - (pan_self_pixel_delta.1 << screenspace_delta.2)
                    );

                    let frequency = 1 << -screenspace_delta.2;

                    for row in 0..self.stencil.resolution.1 {
                        for seat in 0..self.stencil.resolution.0 {
                            let preferred_source_seat_row = (
                                (seat as isize + pan_self_pixel_delta.0) << -screenspace_delta.2
                                , (row as isize + pan_self_pixel_delta.1) << -screenspace_delta.2
                            );

                            let aligned = (preferred_source_seat_row.0 - phase.0) % frequency == 0
                                && (preferred_source_seat_row.1 - phase.1) % frequency == 0;

                            let clamped_source_seat_row = source
                                .stencil
                                .clamp_seat_and_row(preferred_source_seat_row);

                            let representative = preferred_source_seat_row == clamped_source_seat_row;
                            let value = source.data[source.stencil.index(clamped_source_seat_row)];
                            let exact = representative && representative && value.1 & EXACT == EXACT && aligned;

                            let new_value = (
                                value.0
                                , { if exact { EXACT } else { 0 } } + { if representative { EST } else { 0 } }
                            );

                            self.data[self.stencil.index((seat as isize, row as isize))]
                                = new_value;
                        }
                    }
                } else {

                }
            }
        }




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

#[test]
#[should_panic]
fn invalid_test_bad_data() {
    let mut a: View<i32> = View {
        stencil: PixelStencil {
            location: (
                IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT }
                , IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT }
                , 0
            )
            ,
            resolution: (2, 2)
        }
        ,
        data: vec!()
    };
    let b: View<i32> = View {
        stencil: PixelStencil {
            location: (
                IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT }
                , IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT }
                , 0
            )
            ,
            resolution: (2, 2)
        }
        ,
        data: vec!()
    };
    a.fill_from(&b);
}

#[test]
#[should_panic]
fn invalid_test_misaligned() {
    let mut a: View<i32> = View {
        stencil: PixelStencil {
            location: (
                IntExp::ZERO
                , IntExp::ZERO
                , 0
            )
            ,
            resolution: (2, 2)
        }
        ,
        data: vec!()
    };
    let b: View<i32> = View {
        stencil: PixelStencil {
            location: (
                IntExp::ZERO
                , IntExp::ZERO
                , 0
            )
            ,
            resolution: (2, 2)
        }
        ,
        data: vec!()
    };
    a.fill_from(&b);
}

#[test]
fn identity_test() {
    let mut a: View<i32> = View {
        stencil: PixelStencil {
            location: (
                IntExp {val:Integer::ZERO,exp:-PIXELS_PER_UNIT_POT}
                , IntExp {val:Integer::ZERO,exp:-PIXELS_PER_UNIT_POT}
                , 0
            )
            , resolution: (2, 2)
        }
        ,
        data: vec!((1, EXACT+EST), (2, EXACT + EST), (3, EXACT + EST), (4, EXACT + EST))
    };
    let b: View<i32> = View {
        stencil: PixelStencil {
            location: (
                IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT }
                , IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT }
                , 0
            )
            , resolution: (2, 2)
        }
        ,
        data: vec!((1, EXACT + EST), (2, EXACT + EST), (3, EXACT + EST), (4, EXACT + EST))
    };
    a.fill_from(&b);
    if a != b {
        eprintln!("actual: {:?}", a.data);
        eprintln!("expect: {:?}", b.data);
    }
    assert!(a == b);
}

#[test]
fn improve_test() {
    let mut a: View<i32> = View {
        stencil: PixelStencil {
            location: (
                IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT }
                , IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT }
                , 0
            )
            ,
            resolution: (2, 2)
        }
        ,
        data: vec!((0, EST), (0, EST), (0, EST), (0, EST))
    };
    let b: View<i32> = View {
        stencil: PixelStencil {
            location: (
                IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT }
                , IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT }
                , 0
            )
            ,
            resolution: (2, 2)
        }
        ,
        data: vec!((1, EXACT + EST), (2, EXACT + EST), (3, EXACT + EST), (4, EXACT + EST))
    };
    a.fill_from(&b);
    if a != b {
        eprintln!("actual: {:?}", a.data);
        eprintln!("expect: {:?}", b.data);
    }
    assert!(a == b);
}

#[test]
fn zoom_in_test() {
    let mut a: View<i32> = View {
        stencil: PixelStencil {
            location: (
                IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT - 1 }
                , IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT - 1 }
                , 1
            )
            ,
            resolution: (2, 2)
        }
        ,
        data: vec!((0, 0), (0, 0), (0, 0), (0, 0))
    };
    let b: View<i32> = View {
        stencil: PixelStencil {
            location: (
                IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT }
                , IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT }
                , 0
            )
            ,
            resolution: (2, 2)
        }
        ,
        data: vec!((1, EXACT + EST), (2, EXACT + EST), (3, EXACT + EST), (4, EXACT + EST))
    };

    let expect: View<i32> = View {
        stencil: PixelStencil {
            location: (
                IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT - 1 }
                , IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT - 1 }
                , 1
            )
            ,
            resolution: (2, 2)
        }
        ,
        data: vec!((1, EXACT + EST), (1, EST), (1, EST), (1, EST))
    };
    a.fill_from(&b);
    if a != expect {
        eprintln!("actual: {:?}", a.data);
        eprintln!("expect: {:?}", expect.data);
    }
    assert!(a == expect);
}

#[test]
fn zoom_out_test() {
    let mut a: View<i32> = View {
        stencil: PixelStencil {
            location: (
                IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT + 1 }
                , IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT + 1 }
                , -1
            )
            ,
            resolution: (2, 2)
        }
        ,
        data: vec!((0, 0), (0, 0), (0, 0), (0, 0))
    };
    let b: View<i32> = View {
        stencil: PixelStencil {
            location: (
                IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT }
                , IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT }
                , 0
            )
            ,
            resolution: (2, 2)
        }
        ,
        data: vec!((1, EXACT + EST), (2, EXACT + EST), (3, EXACT + EST), (4, EXACT + EST))
    };

    let expect: View<i32> = View {
        stencil: PixelStencil {
            location: (
                IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT + 1 }
                , IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT + 1 }
                , -1
            )
            ,
            resolution: (2, 2)
        }
        ,
        data: vec!((1, EXACT + EST), (2, 0), (3, 0), (4, 0))
    };
    a.fill_from(&b);
    if a != expect {
        eprintln!("actual: {:?}", a.data);
        eprintln!("expect: {:?}", expect.data);
    }
    assert!(a == expect);
}

#[test]
fn pan_one_test() {
    let mut a: View<i32> = View {
        stencil: PixelStencil {
            location: (
                IntExp { val: Integer::ONE.clone(), exp: -PIXELS_PER_UNIT_POT }
                , IntExp { val: Integer::ONE.clone(), exp: -PIXELS_PER_UNIT_POT }
                , 0
            )
            ,
            resolution: (2, 2)
        }
        ,
        data: vec!((0, 0), (0, 0), (0, 0), (0, 0))
    };
    let b: View<i32> = View {
        stencil: PixelStencil {
            location: (
                IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT }
                , IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT }
                , 0
            )
            ,
            resolution: (2, 2)
        }
        ,
        data: vec!((1, EXACT + EST), (2, EXACT + EST), (3, EXACT + EST), (4, EXACT + EST))
    };

    let expect: View<i32> = View {
        stencil: PixelStencil {
            location: (
                IntExp { val: Integer::ONE.clone(), exp: -PIXELS_PER_UNIT_POT }
                , IntExp { val: Integer::ONE.clone(), exp: -PIXELS_PER_UNIT_POT }
                , 0
            )
            ,
            resolution: (2, 2)
        }
        ,
        data: vec!((2, 0), (2, 0), (2, EXACT+EST), (2, 0))
    };
    a.fill_from(&b);
    if a != expect {
        eprintln!("actual: {:?}", a.data);
        eprintln!("expect: {:?}", expect.data);
    }
    assert!(a == expect);
}

#[test]
fn nonzero_phase_test() {
    let mut a: View<i32> = View {
        stencil: PixelStencil {
            location: (
                IntExp { val: Integer::ONE.clone(), exp: -PIXELS_PER_UNIT_POT - 1 }
                , IntExp { val: -Integer::ONE.clone(), exp: -PIXELS_PER_UNIT_POT - 1 }
                , 1
            )
            ,
            resolution: (2, 2)
        }
        ,
        data: vec!((0, 0), (0, 0), (0, 0), (0, 0))
    };
    let b: View<i32> = View {
        stencil: PixelStencil {
            location: (
                IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT }
                , IntExp { val: Integer::ZERO, exp: -PIXELS_PER_UNIT_POT }
                , 0
            )
            ,
            resolution: (2, 2)
        }
        ,
        data: vec!((1, EXACT + EST), (2, EXACT + EST), (3, EXACT + EST), (4, EXACT + EST))
    };

    let expect: View<i32> = View {
        stencil: PixelStencil {
            location: (
                IntExp { val: Integer::ONE.clone(), exp: -PIXELS_PER_UNIT_POT - 1 }
                , IntExp { val: -Integer::ONE.clone(), exp: -PIXELS_PER_UNIT_POT - 1 }
                , 1
            )
            ,
            resolution: (2, 2)
        }
        ,
        data: vec!((1, EST), (2, EST), (3, EST), (4, EXACT + EST))
    };
    a.fill_from(&b);
    if a != expect {
        eprintln!("actual: {:?}", a.data);
        eprintln!("expect: {:?}", expect.data);
    }
    assert!(a == expect);
}