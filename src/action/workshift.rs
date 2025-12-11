

use std::time::Instant;
use std::collections::*;
use std::cmp::*;
use crate::action::utils::*;
pub(crate) const NUMBER_OF_LOOP_CHECK_POINTS: usize = 5;

#[derive(Clone, Debug)]

pub(crate) enum Points {
    F32{p:Vec<PointF32>}
}
#[derive(Clone, Debug)]

pub(crate) struct WorkContext {
    pub(crate) points: Points
    , pub(crate) completed_points: Vec<(CompletedPoint, usize)>
    , pub(crate) last_update: usize
    , pub(crate) index: usize
    , pub(crate) random_index: usize
    , pub(crate) time_created: Instant
    , pub(crate) time_workshift_started: Instant
    , pub(crate) percent_completed:f64
    , pub(crate) random_map: Vec<usize>
    , pub(crate) workshifts: u32
    , pub(crate) total_iterations: u32
    , pub(crate) total_iterations_today: u32
    , pub(crate) total_bouts_today: u32
    , pub(crate) total_points_today: u32
    , pub(crate) spent_tokens_today: u32
    , pub(crate) res: (u32, u32)
    , pub(crate) edge_poses: Vec<(i32, i32)>
    , pub(crate) out_queue: VecDeque<(i32, i32)>
    , pub(crate) in_queue: VecDeque<(i32, i32)>
}


#[derive(Clone, Debug)]
pub(crate) enum CompletedPoint {
    Repeats{
        period: u32
    }
    , Escapes{
        escape_time: u32
        , escape_location: (f32, f32)
        , start_location: (f32, f32)
    }
    , Dummy{}
}


//pub(crate) const SpeedTestPoint
#[derive(Clone, Debug)]

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
    , pub(crate) done: (bool, bool)
}


pub(crate) fn workshift(
    day_token_allowance: u32
    , iteration_token_cost: u32
    , point_token_cost: u32
    , bout_token_cost: u32
    , context: &mut WorkContext
) {
    match context.points {
        Points::F32{p: _} => {
            workshift_f32(
                day_token_allowance
                , iteration_token_cost
                , point_token_cost
                , bout_token_cost
                , context
            )
        }
    }
}


pub(crate) fn workshift_f32(
    day_token_allowance: u32
    , iteration_token_cost: u32
    , point_token_cost: u32
    , bout_token_cost: u32
    , context: &mut WorkContext
) {

    context.time_workshift_started = Instant::now();


    context.total_bouts_today = 0;
    context.total_iterations_today = 0;
    context.total_points_today = 0;
    context.spent_tokens_today = 0;

    let points = match &mut context.points {
        Points::F32 { p} => {p}
    };
    let total_points = points.len();
    context.random_index = context.random_map[min(context.index, total_points-1)];

    while context.index < total_points && context.spent_tokens_today + bout_token_cost + 1000 * iteration_token_cost * point_token_cost < day_token_allowance { // workbout loop

        //while context.already_done_hashset.contains(&context.index) {
        //    context.index += 1;
        //}


        let pos = if context.out_queue.len()>0 {
            &context.out_queue[0]
        } else if context.edge_poses.len()>0{
            &context.edge_poses[0]
        } else if context.in_queue.len()>0 {
            &context.in_queue[0]
        } else {context.index = total_points-1; break;};

        let index = index_from_pos(pos, context.res.0);

        let point = &mut points[index];



        let old_iterations = point.iterations;

        iterate_max_n_times_f32(point, 4.0, 1000);

        context.total_iterations_today += point.iterations - old_iterations;

        if point.done.0 || point.done.1 {

            //context.already_done.push(context.index);
            //context.already_done_hashset.insert(context.index);

            let completed_point = if point.done.1 {
                CompletedPoint::Repeats{period: 0}
            } else {
                CompletedPoint::Escapes {
                    escape_time: point.iterations
                    , escape_location: point.z
                    , start_location: point.c
                }
            };

            context.completed_points.push((completed_point, context.index));

            context.total_iterations += point.iterations;

            context.index += 1;



            context.random_index = context.random_map[min(context.index, total_points-1)];
            context.total_points_today += 1
        }

        context.total_bouts_today += 1;
        context.spent_tokens_today = context.total_bouts_today * bout_token_cost + context.total_points_today * point_token_cost + context.total_iterations_today * point_token_cost;
    }

    context.workshifts += 1;
    context.percent_completed = context.index as f64 / (total_points) as f64 * 100.0;
}

#[inline]
pub(crate) fn iterate_max_n_times_f32 (point: &mut PointF32, r_squared:f32, n: u32) {
    for i in 0..n {
        update_point_results_f32(point);
        point.done.0 = bailout_point_f32(point, r_squared);
        if !(point.done.0 || point.done.1) {
            iterate_f32(point);
        } else {
            break;
        }
        point.done.1 = loop_check_point_f32(point);
        update_loop_check_points(point);
    }
}

#[inline]
pub(crate) fn iterate_f32 (point: &mut PointF32) {
    // move z
    point.z = (
        point.real_squared - point.imag_squared + point.c.0
        , 2.0 * point.real_imag + point.c.1
    );
    point.iterations+=1;
}

#[inline]
pub(crate) fn bailout_point_f32 (point: & PointF32, r_squared:f32) -> bool {
    // checks

    point.real_squared + point.imag_squared > r_squared

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
pub(crate) fn update_point_results_f32(point: &mut PointF32) {
    // update values
    point.real_squared = point.z.0 * point.z.0;
    point.imag_squared = point.z.1 * point.z.1;
    point.real_imag = point.z.0 * point.z.1;
}

#[inline]
pub(crate) fn index_from_pos(pos:&(i32, i32), wid:u32) -> usize {
    (pos.0 + pos.1*wid as i32) as usize
}

pub(crate) fn queue_incomplete_neighbors(pos:&(i32, i32), points: &Vec<PointF32>, queue: &mut VecDeque<(i32, i32)>) {
    let neighbors: [(i32, i32);4] = [
        (pos.0+1, pos.1)
        , (pos.0-1, pos.1)
        , (pos.0, pos.1+1)
        , (pos.0, pos.1-1)
    ];
    for neighbor in neighbors {
        let index = index_from_pos(neighbor,);
        if !(points[neighbor].done.0 || points[neighbor].done.1) {

        }
    }
}