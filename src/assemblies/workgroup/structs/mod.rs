pub mod mandelbrotable;

use std::cmp::Ordering;
use crate::assemblies::structs::*;
use std::collections::*;
use crate::constants::PIXELS_PER_UNIT_POT;
use crate::intexp::*;
use crate::assemblies::workgroup::structs::mandelbrotable::*;
use crate::range::*;


pub struct SchedulingAnswer {
    result: SchedulingMandelbrotResult
    , min_magnitude_angle: u8
    , min_magnitude_time_hash: u16
}

pub enum SchedulingMandelbrotResult {
    Outside {
        escape_time_angle: u32
    }
    , Inside{
        period_hash: u32
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
    // Derivative-magnitude slope angle (D-SCH-3); 0..=255, 0 when unknown.
    , pub escape_time_angle: u8
    , pub min_magnitude_angle: u8
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
    // r[impl cz.range.guess-biased-nearest+1]
    pub fn guess_biased(&self, bias: Answer) -> Answer {
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
            , escape_time_angle: bias.escape_time_angle
            , min_magnitude_angle: bias.min_magnitude_angle
        }

    }
}

#[cfg(test)]
mod calibrated_bias_tests {
    use super::*;
    use crate::assemblies::structs::*;
    use crate::constants::NORES_ANSWER;

    fn exact_u64(v: u64) -> Range<u64> {
        Range {
            lower_bound: v,
            upper_bound: v,
        }
    }
    fn exact_f64(v: f64) -> Range<f64> {
        Range {
            lower_bound: v,
            upper_bound: v,
        }
    }
    fn exact_f32(v: f32) -> Range<f32> {
        Range {
            lower_bound: v,
            upper_bound: v,
        }
    }
    fn wide_escape(lo: u64, hi: u64) -> CalibratedAnswer {
        CalibratedAnswer {
            result: CalibratedMandelbrotResult::Outside {
                escape_time_r2: Range {
                    lower_bound: lo,
                    upper_bound: hi,
                },
                escape_z: (exact_f32(0.0), exact_f32(0.0)),
            },
            min_magnitude_time: exact_u64(0),
            min_magnitude: exact_f64(1.0),
            highlights: CalibratedHighlights {
                in_filament: Range {
                    lower_bound: false,
                    upper_bound: false,
                },
                out_filament: Range {
                    lower_bound: false,
                    upper_bound: false,
                },
                small_time_edge: Range {
                    lower_bound: false,
                    upper_bound: false,
                },
                node: Range {
                    lower_bound: false,
                    upper_bound: false,
                },
            },
            escape_time_angle: 0,
            min_magnitude_angle: 0,
        }
    }

    // r[verify cz.range.guess-biased-nearest+1]
    #[test]
    fn biased_escape_keeps_proximate_when_in_bounds() {
        let cal = wide_escape(10, 50);
        let bias = Answer {
            result: MandelbrotResult::Outside {
                escape_time_r2: 20,
                escape_z: (0.0, 0.0),
            },
            min_magnitude_time: 0,
            min_magnitude: 1.0,
            escape_time_angle: 0,
            min_magnitude_angle: 0
};
        let out = cal.guess_biased(bias);
        match out.result {
            MandelbrotResult::Outside { escape_time_r2, .. } => assert_eq!(escape_time_r2, 20),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn biased_escape_clamps_high_proximate() {
        let cal = wide_escape(10, 50);
        let bias = Answer {
            result: MandelbrotResult::Outside {
                escape_time_r2: 99,
                escape_z: (0.0, 0.0),
            },
            min_magnitude_time: 0,
            min_magnitude: 1.0,
            escape_time_angle: 0,
            min_magnitude_angle: 0
};
        let out = cal.guess_biased(bias);
        match out.result {
            MandelbrotResult::Outside { escape_time_r2, .. } => assert_eq!(escape_time_r2, 50),
            other => panic!("{other:?}"),
        }
    }

    // r[verify cz.display.nores-when-no-proximate+1]
    #[test]
    fn nores_bias_survives_as_outside() {
        let cal = wide_escape(1, 1);
        let out = cal.guess_biased(NORES_ANSWER);
        match out.result {
            MandelbrotResult::Outside { escape_time_r2, .. } => assert_eq!(escape_time_r2, 1),
            MandelbrotResult::Inside { .. } => panic!("must not invent Inside from NORES bias"),
        }
    }
}