use egui::Color32;
use crate::assemblies::headgroup::window::sampling::*;
use crate::assemblies::shadergroup::colorer::color::*;
use crate::assemblies::shadergroup::escaper::*;
use crate::assemblies::structs::*;
use crate::assemblies::workgroup::screen_worker::workshift::CompletedPoint;
use crate::constants::*;
use crate::intexp::*;
use crate::settings::*;

pub fn answer_to_completed(answer: Answer) -> CompletedPoint {
    match answer.result {
        MandelbrotResult::Outside { escape_time_r2, escape_z } => {
            CompletedPoint::Escapes {
                escape_time: escape_time_r2.min(u32::MAX as u64) as u32
                , escape_location: (escape_z.0 as f64, escape_z.1 as f64)
                , start_location: (0.0, 0.0)
                , smallness: answer.min_magnitude
                , small_time: answer.min_magnitude_time.min(u32::MAX as u64) as u32
            }
        }
        , MandelbrotResult::Inside { period } => {
            CompletedPoint::Repeats {
                period: period.min(u32::MAX as u64) as u32
                , smallness: answer.min_magnitude
                , small_time: answer.min_magnitude_time.min(u32::MAX as u64) as u32
            }
        }
    }
}

