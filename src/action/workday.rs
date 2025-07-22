use steady_state::*;


use std::time::{Duration, Instant};


use std::collections::*;

use rand::seq::SliceRandom;
use rand::rng;



pub(crate) const NUMBER_OF_LOOP_CHECK_POINTS: usize = 5;




pub(crate) struct WorkContextF32 {
    pub(crate) points: Vec<PointF32>
    , pub(crate) completed_points: Vec<CompletedPoint>
    , pub(crate) index: usize
    , pub(crate) random_index: usize
    , pub(crate) time_created: Instant
    , pub(crate) time_workday_started: Instant
    , pub(crate) percent_completed:f64
    , pub(crate) random_map: Option<Vec<usize>>
    , pub(crate) workdays: u32
    , pub(crate) total_iterations: u32
    , pub(crate) total_iterations_today: u32
    , pub(crate) total_bouts_today: u32
    , pub(crate) total_points_today: u32
    , pub(crate) spent_tokens_today: u32
}


#[derive(Clone)]
pub(crate) enum CompletedPoint {
    Repeats{}
    , Escapes{
        escape_time: u32,
        escape_location: (f32, f32)
    },
    Dummy
}


//pub(crate) const SpeedTestPoint

pub(crate) struct PointF32 {
    pub(crate) c: (f32, f32)
    , pub(crate) z: (f32, f32)
    , pub(crate) real_squared: f32
    , pub(crate) imag_squared: f32
    , pub(crate) real_imag: f32
    , pub(crate) iterations: u32
    // if this isn't updated enough, you will take longer to realize loops.
    // If its updated too often, you will not be able to realize long loops.
    , pub(crate) loop_detection_points: [(f32, f32);NUMBER_OF_LOOP_CHECK_POINTS]
    , pub(crate) last_point: (f32, f32)
    , pub(crate) done: (bool, bool)
}


pub(crate) fn workday(
    day_token_allowance: u32
    , iteration_token_cost: u32
    , point_token_cost: u32
    , bout_token_cost: u32
    , context: &mut WorkContextF32
) -> Option<Vec<CompletedPoint>> {

    context.time_workday_started = Instant::now();

    context.total_bouts_today = 0;
    context.total_iterations_today = 0;
    context.total_points_today = 0;
    context.spent_tokens_today = 0;




    //info!("workday start");

    match &context.random_map {
        Some(r) => {}
        None => {
            let mut rng = rand::rng();

            let mut indices: Vec<usize> = (0..context.points.len()).collect();

            // Shuffle indices randomly
            indices.shuffle(&mut rng);
            context.random_map = Some(indices);
            context.random_index = context.random_map.as_ref().unwrap()[context.index];
        }
    }

    let total_points = context.points.len();

    while context.index < total_points-1 && context.spent_tokens_today + bout_token_cost + 1000 * iteration_token_cost * point_token_cost < day_token_allowance { // workbout loop

        let point = &mut context.points[context.random_index];

        let old_iterations = point.iterations;

        iterate_max_n_times_f32(point, 1000);

        context.total_iterations_today += point.iterations - old_iterations;

        if point.done.0 || point.done.1 {

            let completed_point = if point.done.1 {
                CompletedPoint::Repeats{}
            } else {
                CompletedPoint::Escapes {
                    escape_time: point.iterations
                    , escape_location: point.z
                }
            };

            context.completed_points[context.random_index] = completed_point;

            context.total_iterations += point.iterations;

            context.index += 1;
            context.random_index = context.random_map.as_ref().unwrap()[context.index];
            context.total_points_today += 1
        }

        context.total_bouts_today += 1;
        context.spent_tokens_today = context.total_bouts_today * bout_token_cost + context.total_points_today * point_token_cost + context.total_iterations_today * point_token_cost;
    }

    if context.index == total_points-1 {

        let mut returned = vec!();
        for i in 0..total_points {
            returned.push(
                match context.points[i].done {
                    (true, false) => {
                        CompletedPoint::Escapes{
                            escape_time: context.points[i].iterations
                            , escape_location: context.points[i].z
                        }
                    }
                    , (false, true) => {
                        CompletedPoint::Repeats{}
                    }
                    , _ => {CompletedPoint::Repeats{}}//CompletedPoint::Dummy}
                }
            );
        }
        context.workdays += 1;
        context.percent_completed = context.index as f64 / (total_points-1) as f64 * 100.0;
        Some(returned)
    } else {


        /*let mut returned = vec!();
        for i in 0..total_points {
            returned.push(
                match context.points[i].done {
                    (true, false) => {
                        CompletedPoint::Escapes{
                            escape_time: context.points[i].iterations
                            , escape_location: context.points[i].z
                        }
                    }
                    , (false, true) => {
                        CompletedPoint::Repeats{}
                    }
                    , _ => {CompletedPoint::Repeats{}}//CompletedPoint::Dummy}
                }
            );
        }
        context.workdays += 1;
        context.percent_completed = context.index as f64 / (total_points-1) as f64 * 100.0;
        Some(returned)*/

        if context.workdays % 20 == 0 {
            let mut returned = vec!();
            for i in 0..total_points {
                returned.push(
                    match context.points[i].done {
                        (true, false) => {
                            CompletedPoint::Escapes{
                                escape_time: context.points[i].iterations
                                , escape_location: context.points[i].z
                            }
                        }
                        , (false, true) => {
                            CompletedPoint::Repeats{}
                        }
                        , _ => {CompletedPoint::Repeats{}}//CompletedPoint::Dummy}
                    }
                );
            }
            context.workdays += 1;
            context.percent_completed = context.index as f64 / (total_points-1) as f64 * 100.0;
            Some(returned)
        } else {
            context.workdays += 1;

            context.percent_completed = context.index as f64 / (total_points-1) as f64 * 100.0;
            None
        }
    }
}
#[inline]
fn iterate_max_n_times_f32 (point: &mut PointF32, n: u32) {
    for i in 0..n {
        update_point_results_f32(point);
        point.done.0 = bailout_point_f32(point);
        if !(point.done.0 || point.done.1) {
            iterate_f32(point);
            point.iterations+=1;
        } else {
            break;
        }
        point.done.1 = loop_check_point_f32(point);
        update_loop_check_points(point);
    }
}

#[inline]
fn iterate_f32 (point: &mut PointF32) {
    // move z
    point.z = (
        point.real_squared - point.imag_squared + point.c.0
        , 2.0 * point.real_imag + point.c.1
    );
}

#[inline]
fn bailout_point_f32 (point: & PointF32) -> bool {
    // checks

    point.real_squared + point.imag_squared > 4.0

}

#[inline]
fn loop_check_point_f32 (point: & PointF32) -> bool {
    // checks
    let mut looped = false;
    for loop_check_point in &point.loop_detection_points {
        looped = looped || point.z == *loop_check_point;
    }
    looped// || point.z == point.last_point
}

#[inline]
fn update_loop_check_points (point: &mut PointF32) {
    /*point.last_point = point.z;
    if point.iterations%(1000) == 0 {
        point.loop_detection_points[0] = point.z;
    }
    if point.iterations%(5000) == 0 {
        point.loop_detection_points[1] = point.z;
    }
    if point.iterations%(10000) == 0 {
        point.loop_detection_points[2] = point.z;
    }
    if point.iterations%(50000) == 0 {
        point.loop_detection_points[3] = point.z;
    }
    if point.iterations%(100000) == 0 {
        point.loop_detection_points[4] =point.z;
    }
    if point.iterations%(500000) == 0 {
        point.loop_detection_points[5] = point.z;
    }
    if point.iterations%(1000000) == 0 {
        point.loop_detection_points[6] = point.z;
    }
    if point.iterations%(5000000) == 0 {
        point.loop_detection_points[7] = point.z;
    }
    if point.iterations%(10000000) == 0 {
        point.loop_detection_points[8] =  point.z;
    }
    if point.iterations%(50000000) == 0 {
        point.loop_detection_points[9] = point.z;
    }*/

   /* point.last_point = point.z;
    if point.iterations%(1000) == 0 {
        point.loop_detection_points[0] = point.z;
    }
    if point.iterations%(10000) == 0 {
        point.loop_detection_points[1] = point.z;
    }

    if point.iterations%(100000) == 0 {
        point.loop_detection_points[2] =point.z;
    }

    if point.iterations%(1000000) == 0 {
        point.loop_detection_points[3] = point.z;
    }

    if point.iterations%(10000000) == 0 {
        point.loop_detection_points[4] =  point.z;
    }*/

    //point.last_point = point.z;

    if point.iterations%(1<<1) == 0 {
        point.loop_detection_points[0] = point.z;
    }

    if point.iterations%(1<<8) == 0 {
        point.loop_detection_points[1] = point.z;
    }

    if point.iterations%(1<<14) == 0 {
        point.loop_detection_points[2] = point.z;
    }

    if point.iterations%(1<<23) == 0 {
        point.loop_detection_points[3] =point.z;
    }

    if point.iterations%(1<<25) == 0 {
        point.loop_detection_points[4] =point.z;
    }

}


#[inline]
fn update_point_results_f32(point: &mut PointF32) {
    // update values
    point.real_squared = point.z.0 * point.z.0;
    point.imag_squared = point.z.1 * point.z.1;
    point.real_imag = point.z.0 * point.z.1;
}