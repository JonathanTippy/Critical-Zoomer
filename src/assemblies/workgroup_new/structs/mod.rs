pub mod sparse_views;
pub mod mandelbrotable;

use std::cmp::Ordering;
use crate::assemblies::structs::*;
use std::collections::*;
use crate::constants::PIXELS_PER_UNIT_POT;
use crate::intexp::*;
use crate::assemblies::workgroup_new::structs::mandelbrotable::*;
use crate::range::*;
#[derive(PartialEq, Clone, Debug)]

pub struct SparseView<T> {
    stencil: PointStencil
    , points: Vec<(T, u8, (usize, usize))>
    , map: HashMap<(usize, usize), usize>
}


impl<T: Copy + Clone> From<View<T>> for SparseView<T> {
    fn from(input: View<T>) -> SparseView<T> {
        let mut returned = SparseView::new(input.stencil);
        for i in 0..input.data.len() {
            if input.bitmap[i] != 0 {
                let value = input.data[i];
                let align = input.bitmap[i];
                returned.insert_with_align((value, align, returned.stencil.seat_and_row(i)));
            }
        }
        returned
    }
}

pub enum SerialWorkUpdate<T> {
    NewStencil {
        stencil: PointStencil
    }
    , PointUpdate{
        update: T
        , seat: (usize, usize)
    }
}

#[derive(Clone, Copy, Debug)]

pub struct CalibratedAnswer {
    pub result: CalibratedMandelbrotResult
    , pub min_magnitude_time: Range<f64, true>
    , pub min_magnitude: Range<f64, false>
}
#[derive(Clone, Copy, Debug)]
pub enum CalibratedMandelbrotResult {
    Agnostic{
        period: Range<f64, true>
        , escape_time_r2: Range<f64, true>
        , escape_z: (Range<f32, false>, Range<f32, false>)
    }
    , Inside{
        period: Range<f64, true>
    }
    , Outside{
        escape_time_r2: Range<f64, true>
        , escape_z: (Range<f32, false>, Range<f32, false>)
    }
}

impl CalibratedAnswer {
    fn guess(&self, bias: Answer) -> Answer {
        let result = match self.result {
            CalibratedMandelbrotResult::Agnostic{period, escape_time_r2, escape_z} => {
                match bias.result {
                    MandelbrotResult::Inside { period: bias_period } => {
                        MandelbrotResult::Inside {
                            period: guess_integer_value(period, bias_period)
                        }
                    }
                    ,
                    MandelbrotResult::Outside { escape_time_r2:bias_escape_time_r2, escape_z:bias_escape_z } => {
                        MandelbrotResult::Outside {
                            escape_time_r2: guess_integer_value(
                                escape_time_r2, bias_escape_time_r2
                            )
                            , escape_z: (
                                guess_value(escape_z.0, bias_escape_z.0)
                                , guess_value(escape_z.1, bias_escape_z.1)
                            )
                        }
                    }
                }
            }
            , CalibratedMandelbrotResult::Inside{period} => {
                match bias.result {
                    MandelbrotResult::Inside{period: bias_period} => {
                        MandelbrotResult::Inside{
                            period: guess_integer_value(period, bias_period)
                        }
                    }
                    , MandelbrotResult::Outside{
                        escape_time_r2: bias_escape_time_r2, escape_z: bias_escape_z
                    } => {
                        MandelbrotResult::Inside {
                            period: period.guess_left() as u64
                        }
                    }
                }
            }
            , CalibratedMandelbrotResult::Outside{ escape_time_r2, escape_z} => {
                match bias.result {
                    MandelbrotResult::Inside { period: bias_period} => {
                        MandelbrotResult::Outside {
                            escape_time_r2: escape_time_r2.guess_left() as u64
                            , escape_z: (
                                escape_z.0.guess_left()
                                , escape_z.1.guess_left()
                            )
                        }
                    }
                    , MandelbrotResult::Outside { escape_time_r2: bias_escape_time_r2, escape_z: bias_escape_z} => {
                        MandelbrotResult::Outside {
                            escape_time_r2: guess_integer_value(
                                escape_time_r2, bias_escape_time_r2
                            )
                            ,
                            escape_z: (
                                guess_value(escape_z.0, bias_escape_z.0)
                                , guess_value(escape_z.1, bias_escape_z.1)
                            )
                        }
                    }
                }
            }
        };
        Answer{
            result
            , min_magnitude_time: guess_integer_value(
                self.min_magnitude_time, bias.min_magnitude_time
            )
            , min_magnitude: guess_value(
                self.min_magnitude, bias.min_magnitude
            )
        }
    }
}

fn guess_integer_value(input: Range<f64, true>, bias: u64) -> u64 {
    let bias_range = Range{
        lower_bound: bias as f64
        , upper_bound: bias as f64
    };
    if input.is_agnostic() {
        return bias
    }
    if input.can_eq(bias_range) {
        return bias
    }
    if input.must_gt(bias_range) {
        return input.guess_left() as u64
    }
    if input.must_lt(bias_range) {
        return input.guess_right() as u64
    }
    panic!("this should be impossible...")
}

fn guess_value<T: Value>(input: Range<T, false>, bias: T) -> T {
    let bias_range = Range {
        lower_bound: bias
        ,
        upper_bound: bias
    };
    if input.is_agnostic() {
        return bias
    }
    if input.can_eq(bias_range) {
        return bias
    }
    if input.must_gt(bias_range) {
        return input.guess_left()
    }
    if input.must_lt(bias_range) {
        return input.guess_right()
    }
    panic!("this should be impossible...")
}