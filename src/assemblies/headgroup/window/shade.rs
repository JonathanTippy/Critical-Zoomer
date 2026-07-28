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

// r[impl cz.hoarding.one-answer-per-point+1]
pub fn recolor_hoard(
    sampling_context: &mut SamplingContext
    , settings: &mut Settings
) {
    let res = (
        sampling_context.screen_size.0
        , sampling_context.screen_size.1
    );
    if res.0 == 0 || res.1 == 0 {
        return;
    }
    let width = res.0 as usize;
    let height = res.1 as usize;
    let mut points = Vec::with_capacity(width * height);
    for y in 0..height {
        for x in 0..width {
            points.push(answer_to_completed(
                sampling_context.lookup_answer_viewport((x, y))
            ));
        }
    }
    let radius = settings.bailout_radius.determine().max(2.0) as f32;
    let mut values = Vec::with_capacity(points.len());
    for i in 0..points.len() {
        let pos = (
            (i % width) as i32
            , (i / width) as i32
        );
        values.push(escape_finished_answer(
            &points[i]
            , radius
            , pos
            , &points
            , res
            , settings.clone()
        ));
    }
    let location = sampling_context.location.clone();
    let screen = ZoomerValuesScreen {
        values
        , res
        , objective_location: location.clone()
    };
    let colors = color(&screen, settings);
    let stencil = PointStencil {
        homothety: (
            location.pos.0.clone()
            , IntExp::ZERO - location.pos.1.clone()
            , location.zoom_pot
        )
        , resolution: (width, height)
        , serial_number: 0
        , focus: None
        , hover: None
    }.correct_precision();
    let mut view = View::new(stencil, Color32::BLACK);
    view.data = colors;
    sampling_context.color_screen = Some(view);
    sampling_context.proximate_answers = false;
    sampling_context.unsent_answers = true;
}
