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
            if input.alignment[i] != 0 {
                let value = input.data[i];
                let align = input.alignment[i];
                returned.insert_with_align((value, align, returned.stencil.seat_and_row(i)));
            }
        }
        returned
    }
}

pub struct PointUpdate<T>{
    update: T
    , seat: (usize, usize)
    , stencil_serial_number: u64
}

#[derive(Clone, Copy, Debug)]

pub struct CalibratedAnswer {
    pub result: CalibratedMandelbrotResult
    , pub min_magnitude_time: Range<u64>
    , pub min_magnitude: Range<f64>
    , pub highlights: CalibratedHighlights
}

#[derive(Copy, Clone, Debug)]
pub struct CalibratedHighlights {
    pub in_filament: Range<bool>
    , pub out_filament: Range<bool>
    , pub small_time_edge: Range<bool>
    , pub node: Range<bool>
}

#[derive(Clone, Copy, Debug)]
pub enum CalibratedMandelbrotResult {
    Agnostic{
        period: Range<u64>
        , escape_time_r2: Range<u64>
        , escape_z: (Range<f32>, Range<f32>)
    }
    , Inside{
        period: Range<u64>
    }
    , Outside{
        escape_time_r2: Range<u64>
        , escape_z: (Range<f32>, Range<f32>)
    }
}

impl CalibratedAnswer {
    fn guess_biased(&self, bias: Answer) -> Answer {
        let result = match self.result {
            CalibratedMandelbrotResult::Agnostic{period, escape_time_r2, escape_z} => {
                match bias.result {
                    MandelbrotResult::Inside { period: bias_period } => {
                        MandelbrotResult::Inside {
                            period: period.guess_biased(bias_period)
                        }
                    }
                    ,
                    MandelbrotResult::Outside { escape_time_r2:bias_escape_time_r2, escape_z:bias_escape_z } => {
                        MandelbrotResult::Outside {
                            escape_time_r2:
                                escape_time_r2.guess_biased(bias_escape_time_r2)

                            , escape_z: (
                                escape_z.0.guess_biased(bias_escape_z.0)
                                , escape_z.1.guess_biased(bias_escape_z.1)
                            )
                        }
                    }
                }
            }
            , CalibratedMandelbrotResult::Inside{period} => {
                match bias.result {
                    MandelbrotResult::Inside{period: bias_period} => {
                        MandelbrotResult::Inside{
                            period: period.guess_biased(bias_period)
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
                            escape_time_r2:
                                escape_time_r2.guess_biased(bias_escape_time_r2)
                            ,
                            escape_z: (
                                escape_z.0.guess_biased(bias_escape_z.0)
                                , escape_z.1.guess_biased(bias_escape_z.1)
                            )
                        }
                    }
                }
            }
        };
        Answer{
            result
            , min_magnitude_time:
                self.min_magnitude_time.guess_biased(bias.min_magnitude_time)
            , min_magnitude:
                self.min_magnitude.guess_biased(bias.min_magnitude)
            , highlights:
            Highlights{
                in_filament: self.highlights.in_filament.guess_biased(bias.highlights.in_filament)
                , out_filament: self.highlights.out_filament.guess_biased(bias.highlights.out_filament)
                , small_time_edge: self.highlights.small_time_edge.guess_biased(bias.highlights.small_time_edge)
                , node: self.highlights.node.guess_biased(bias.highlights.node)
            }
        }
    }
}