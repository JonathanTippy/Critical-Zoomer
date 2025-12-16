
use rand::Rng;

use std::time::Instant;
use std::collections::*;
use std::cmp::*;
use crate::action::utils::*;
pub(crate) const NUMBER_OF_LOOP_CHECK_POINTS: usize = 5;

#[derive(Clone, Debug)]
pub(crate) enum Step {Scredge, In, Out, Edge, Random}

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
    , pub(crate) scredge_poses: VecDeque<(i32, i32)>
    , pub(crate) edge_queue: VecDeque<((i32, i32), u32)>
    , pub(crate) out_queue: VecDeque<((i32, i32), u32)>
    , pub(crate) in_queue: VecDeque<((i32, i32), u32)>
    , pub(crate) zoomed: bool
    , pub(crate) attention: (i32, i32)
    , pub(crate) attention_radius: u32
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
#[derive(Clone, Debug, Copy)]

pub(crate) struct PointF32 {
    pub(crate) c: (f32, f32)
    , pub(crate) z: (f32, f32)
    , pub(crate) real_squared: f32
    , pub(crate) imag_squared: f32
    , pub(crate) real_imag: f32
    , pub(crate) iterations: u32
    , pub(crate) loop_detection_point: ((f32, f32), u32)
    , pub(crate) done: (bool, bool)
    , pub(crate) delivered: bool
    , pub(crate) period: u32
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


    let (total_points, episilon) = {
        let points = match &mut context.points {
            Points::F32 { p} => {p}
        };
        (points.len(), (points[0].c.0 - points[1].c.0).abs() / 64.0)
    };

    context.random_index = context.random_map[min(context.index, total_points-1)];


    while context.time_workshift_started.elapsed().as_millis()<10{//while context.index < total_points && context.spent_tokens_today + bout_token_cost + 1000 * iteration_token_cost * point_token_cost < day_token_allowance { // workbout loop



        let poses = get_poses(context, total_points);

        for queue_index in 0..poses.len() {
            let (pos, step) = &poses[queue_index];
            let points = match &mut context.points {
                Points::F32 { p} => {p}
            };

            let index = index_from_pos(&pos, context.res.0);

            let point = &mut points[index];

            //if context.workshifts > 100 {
            match step {
                Step::In => {
                    point.period = context.in_queue[0].1;
                    context.completed_points.push((CompletedPoint::Repeats{period: context.in_queue[queue_index].1}, index));
                    point.delivered = true;
                    queue_incomplete_neighbors_in(&pos, context.res, points, &mut context.in_queue);
                    let _ =  context.in_queue.remove(queue_index);
                    continue;
                }
                _ => {}
            }
            //}

        }

        multi_iterate_max_n_times(&poses, context,4.0, episilon, 1000);

        for queue_index in 0..poses.len() {
            let (pos, step) = &poses[queue_index];
            let points = match &mut context.points {
                Points::F32 { p} => {p}
            };

            let index = index_from_pos(&pos, context.res.0);

            let point = &mut points[index];


            if point.done.0 || point.done.1 {

                //context.already_done.push(context.index);
                //context.already_done_hashset.insert(context.index);
                context.total_iterations += point.iterations;

                point.delivered = true;



                let completed_point = if point.done.1 {
                    let raw_period = point.iterations-point.loop_detection_point.1;
                    let returned = if determine_period(point, episilon) {
                        point.period = point.iterations-point.loop_detection_point.1;
                        CompletedPoint::Repeats{period: point.period}
                    } else {
                        point.period = raw_period;
                        CompletedPoint::Repeats{period: point.period}
                    };
                    returned
                } else {
                    let result = CompletedPoint::Escapes {
                        escape_time: point.iterations
                        , escape_location: point.z
                        , start_location: point.c
                    };
                    result
                };
                if let Some(e) = point_is_edge(&pos, context.res, points) {
                    //context.edge_queue.clear();
                }

                context.completed_points.push((completed_point, index));


                context.total_points_today += 1
            }

        }

        for queue_index in (0..poses.len()).rev() {
            let (pos, step) = &poses[queue_index];
            let points = match &mut context.points {
                Points::F32 { p} => {p}
            };

            let index = index_from_pos(&pos, context.res.0);

            let point = &mut points[index];


            if point.done.0 || point.done.1 {
                match step {
                    Step::Out => {
                        let _ =  context.out_queue.remove(queue_index);
                    }
                    Step::Scredge => {
                        let _ = context.scredge_poses.remove(queue_index);
                    }
                    Step::In => {
                        let _ =  context.in_queue.remove(queue_index);
                    }
                    Step::Edge => {
                        let _ =  context.edge_queue.remove(queue_index);
                    }
                    Step::Random => {
                        context.index += 1;
                        context.random_index = context.random_map[min(context.index, total_points-1)];
                    }
                }
            } else {
                match step {
                    Step::Out => {
                        let pos = context.out_queue.remove(queue_index).unwrap();
                        context.out_queue.push_back(pos);
                        continue;
                    }
                    /*Step::In => {
                        let pos = context.in_queue.pop_front().unwrap();
                        context.in_queue.push_back(pos);
                        continue;
                    }*/
                    Step::Scredge => {
                        //let pos = context.scredge_poses.pop_front().unwrap();
                        //context.scredge_poses.push_back(pos);
                        let completed_point = {
                            CompletedPoint::Repeats{period: point.iterations-point.loop_detection_point.1}
                        };
                        context.completed_points.push((completed_point, index));
                        continue;
                    }
                    _ => {}
                }
            }
        }


        for queue_index in 0..poses.len() {
            let (pos, step) = &poses[queue_index];
            let points = match &mut context.points {
                Points::F32 { p} => {p}
            };

            let index = index_from_pos(&pos, context.res.0);

            let point = &mut points[index];

            if point.done.1 {
                queue_incomplete_neighbors_in(&pos, context.res, points, &mut context.in_queue);
            } else {
                queue_incomplete_neighbors(&pos, context.res, points, &mut context.out_queue);
            };
            if let Some(e) = point_is_edge(&pos, context.res, points) {
                queue_incomplete_neighbors_of_edge(&e.0, &e.1, context.res, points, &mut context.edge_queue);
            }
        }

        context.total_bouts_today += 1;
        context.spent_tokens_today = context.total_bouts_today * bout_token_cost + context.total_points_today * point_token_cost + context.total_iterations_today * point_token_cost;
    }

    context.workshifts += 1;
    context.percent_completed = context.index as f64 / (total_points) as f64 * 100.0;
}

#[inline]
pub(crate) fn iterate_max_n_times_f32 (point: &mut PointF32, r_squared:f32, epsilon:f32, n: u32) {
    for i in 0..n {
        update_point_results_f32(point);
        point.done.0 = bailout_point_f32(point, r_squared) || (!point.real_squared.is_finite()) || (!point.imag_squared.is_finite());
        if !(point.done.0 || point.done.1) {
            iterate_f32(point);
        } else {
            break;
        }
        point.done.1 = loop_check_point_f32(point, epsilon);
        update_loop_check_points(point);
    }
}


#[inline]
pub(crate) fn timewarp_n_iterations (point: &mut PointF32, r_squared:f32, n: u32) -> bool {


    let c = point.c.clone();
    let mut z = point.z.clone();

    let blocks = n / 4096;
    let change = n % 4096;

    for _ in 0..blocks {
        timewarp_4096(&mut z, c);
    }
    for _ in 0..change {
        z = (
            z.0 * z.0 - z.1 * z.1 + c.0
            , 2.0 * z.0 * z.1 + c.1
        );
    }

    let backup = point.clone();
    point.z = z; update_point_results_f32(point);

    if bailout_point_f32(point, r_squared) || (!point.real_squared.is_finite()) || (!point.imag_squared.is_finite()) {
        *point = backup; false
    } else {
        point.iterations+=n;
        true
    }
}

#[inline(always)]
fn timewarp_4096 ( z:&mut (f32, f32), c:(f32,f32)) {
    for _ in 0..4096 {
        *z = (
            z.0 * z.0 - z.1 * z.1 + c.0
            , 2.0 * z.0 * z.1 + c.1
        );
    }
}

#[inline(always)]
pub(crate) fn iterate_f32 (point: &mut PointF32) {
    // move z
    point.z = (
        point.real_squared - point.imag_squared + point.c.0
        , 2.0 * point.real_imag + point.c.1
    );
    point.iterations+=1;
}

#[inline(always)]
pub(crate) fn bailout_point_f32 (point: & PointF32, r_squared:f32) -> bool {
    // checks

    point.real_squared + point.imag_squared > r_squared
}

#[inline(always)]
fn points_near (z1: (f32, f32), z2: (f32, f32), e: f32) -> bool {
    z1.0 >= (z2.0 - e) && z1.0 <= (z2.0 + e)
    && z1.1 >= (z2.1 - e) && z1.1 <= (z2.1 + e)
}

#[inline(always)]
fn loop_check_point_f32 (point: & PointF32, epsilon:f32) -> bool {
    points_near(point.z, point.loop_detection_point.0, epsilon)
}

#[inline(always)]
fn update_loop_check_points (point: &mut PointF32) {

    if point.iterations >= point.loop_detection_point.1 << 1 {
        point.loop_detection_point = (point.z, point.iterations);
    }
}

fn determine_period (point: &mut PointF32, epsilon:f32) -> bool {
    let max_period = 1000;

    timewarp_n_iterations(point, 4.0, 1000);

    point.loop_detection_point = (point.z, point.iterations);
    for _ in 0..max_period {
        update_point_results_f32(point);
        iterate_f32(point);
        if loop_check_point_f32(point, 0.0001) {
            return true
        }
    }
    return false
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

pub(crate) fn pos_from_index(i: usize, wid:u32) -> (i32, i32) {
    (i as i32 % wid as i32, i as i32/wid as i32)
}

pub(crate) fn queue_incomplete_neighbors(pos:&(i32, i32), res: (u32, u32), points: &Vec<PointF32>, queue: &mut VecDeque<((i32, i32), u32)>) {

    let difficulty = points[index_from_pos(pos, res.0)].iterations;

    let wid = res.0;

    let neighbors: [(i32, i32);4] = [
        (pos.0+1, pos.1)
        , (pos.0-1, pos.1)
        , (pos.0, pos.1+1)
        , (pos.0, pos.1-1)
    ];
    for n in neighbors {

        if (
            n.0 >= 0 && n.0 <= res.0 as i32 - 1
            && n.1 >= 0 && n.1 <= res.1 as i32 - 1
            ) {
            let index = index_from_pos(&n, wid);
            if !(points[index].done.0 || points[index].done.1) {
                queue.push_back((n, difficulty));
            }
        }
    }
}

pub(crate) fn queue_incomplete_neighbors_in(pos:&(i32, i32), res: (u32, u32), points: &Vec<PointF32>, queue: &mut VecDeque<((i32, i32), u32)>) {

    let period = points[index_from_pos(&pos, res.0)].period;

    let wid = res.0;

    let neighbors: [(i32, i32);4] = [
        (pos.0+1, pos.1)
        , (pos.0-1, pos.1)
        , (pos.0, pos.1+1)
        , (pos.0, pos.1-1)
    ];
    for n in neighbors {

        if (
            n.0 >= 0 && n.0 <= res.0 as i32 - 1
                && n.1 >= 0 && n.1 <= res.1 as i32 - 1
        ) {
            let index = index_from_pos(&n, wid);
            if !(points[index].done.0 || points[index].done.1) {
                queue.push_back((n, period));
            }
        }
    }
}

pub(crate) fn point_is_edge(pos:&(i32, i32), res: (u32, u32), points: &Vec<PointF32>) -> Option<((i32, i32), (i32, i32))> {
    let neighbors: [(i32, i32);4] = [
        (pos.0+1, pos.1)
        , (pos.0-1, pos.1)
        , (pos.0, pos.1+1)
        , (pos.0, pos.1-1)
    ];

    let index = index_from_pos(&pos, res.0);
    for n in neighbors {

        if (
            n.0 >= 0 && n.0 <= res.0 as i32 - 1
                && n.1 >= 0 && n.1 <= res.1 as i32 - 1
        ) {
            let nindex = index_from_pos(&n, res.0);
            if (points[index].done.0 || points[index].done.1)
                && (points[nindex].done.0 || points[nindex].done.1)
            {
                if points[index].done != points[nindex].done {
                    return Some((*pos, n));
                } else if points[index].done.1 == true {
                    if points[index].period!=points[nindex].period {
                        return Some((*pos, n));
                    }
                }
            }
        }
    }
    None
}

pub(crate) fn queue_incomplete_neighbors_of_edge(pos1:&(i32, i32), pos2:&(i32, i32), res: (u32, u32), points: &Vec<PointF32>, queue: &mut VecDeque<((i32, i32), u32)>) {

    let difficulty = points[index_from_pos(pos1, res.0)].iterations;

    let wid = res.0;

    let neighbors: [(i32, i32);8] = if (pos1.0 - pos2.0).abs()==1 { // horizontal
        if pos1.0>pos2.0 { // pos1 more right
            [
                (pos1.0, pos1.1+1)
                , (pos2.0, pos2.1+1)
                , (pos2.0, pos2.1-1)
                , (pos1.0, pos1.1-1)
                , (pos1.0+1, pos1.1+1)
                , (pos2.0-1, pos2.1+1)
                , (pos2.0-1, pos2.1-1)
                , (pos1.0+1, pos1.1-1)
                //, (pos1.0+1, pos1.1)
                //, (pos2.0-1, pos2.1)
            ]
        } else { // pos2 more right
            [
                (pos2.0, pos2.1+1)
                , (pos1.0, pos1.1+1)
                , (pos1.0, pos1.1-1)
                , (pos2.0, pos2.1-1)
                , (pos2.0+1, pos2.1+1)
                , (pos1.0-1, pos1.1+1)
                , (pos1.0-1, pos1.1-1)
                , (pos2.0+1, pos2.1-1)
                //, (pos2.0+1, pos2.1)
                //, (pos1.0-1, pos1.1)
            ]
        }
    } else { // vertical
        if pos1.0>pos2.0 { // pos1 higher
            [
                (pos1.0+1, pos1.1)
                , (pos2.0+1, pos2.1)
                , (pos1.0-1, pos1.1)
                , (pos2.0-1, pos2.1)
                , (pos1.0+1, pos1.1+1)
                , (pos2.0+1, pos2.1-1)
                , (pos2.0-1, pos2.1-1)
                , (pos1.0-1, pos1.1+1)
                //, (pos1.0, pos1.1+1)
                //, (pos2.0, pos2.1-1)
            ]
        } else { // pos2 higher
            [
                (pos1.0+1, pos1.1)
                , (pos2.0+1, pos2.1)
                , (pos2.0-1, pos2.1)
                , (pos1.0-1, pos1.1)
                , (pos2.0+1, pos2.1+1)
                , (pos1.0+1, pos1.1-1)
                , (pos1.0-1, pos1.1-1)
                , (pos2.0-1, pos2.1+1)
                //, (pos2.0, pos2.1+1)
                //, (pos1.0, pos1.1-1)
            ]
        }
    };

    /*let neighbors: [(i32, i32);4] = [
        (pos.0+1, pos.1)
        , (pos.0-1, pos.1)
        , (pos.0, pos.1+1)
        , (pos.0, pos.1-1)
    ];*/
    for n in neighbors {

        if (
            n.0 >= 0 && n.0 <= res.0 as i32 - 1
                && n.1 >= 0 && n.1 <= res.1 as i32 - 1
        ) {
            let index = index_from_pos(&n, wid);
            if !(points[index].done.0 || points[index].done.1) {
                queue.push_front((n, difficulty));
            }
        }
    }
}


fn get_poses(mut context: &mut WorkContext, total_points:usize) -> Vec<((i32, i32), Step)> {

    let points = match &mut context.points {
        Points::F32 { p} => {p}
    };

    let mut returned = Vec::new();

    let mut queue_index = 0;

    while returned.len()<8 {

        let (pos, step) = match context.workshifts%5 {
            0 => {
                if context.workshifts == 0 {
                    if context.scredge_poses.len()>queue_index {
                        (&context.scredge_poses[queue_index], Step::Scredge)
                    } else if context.edge_queue.len()>queue_index {
                        (&context.edge_queue[queue_index].0, Step::Edge)
                    } else if context.out_queue.len()>queue_index{
                        (&context.out_queue[queue_index].0, Step::Out)
                    } else if context.in_queue.len()>queue_index {
                        (&context.in_queue[queue_index].0, Step::In)
                    } else {context.index = total_points-1; break;
                    }
                } else {
                    if context.edge_queue.len()>queue_index {
                        (&context.edge_queue[queue_index].0, Step::Edge)
                    } else if context.out_queue.len()>queue_index{
                        (&context.out_queue[queue_index].0, Step::Out)
                    } else if context.scredge_poses.len()>queue_index {
                        (&context.scredge_poses[queue_index], Step::Scredge)
                    } else if context.in_queue.len()>queue_index {
                        (&context.in_queue[queue_index].0, Step::In)
                    } else {context.index = total_points-1; break;
                    }
                }
            }
            1 => {
                if context.edge_queue.len()>queue_index {
                    (&context.edge_queue[queue_index].0, Step::Edge)
                } else if context.out_queue.len()>queue_index{
                    (&context.out_queue[queue_index].0, Step::Out)
                } else if context.scredge_poses.len()>queue_index {
                    (&context.scredge_poses[queue_index], Step::Scredge)
                } else if context.in_queue.len()>queue_index {
                    (&context.in_queue[queue_index].0, Step::In)
                } else {context.index = total_points-1; break;}
            }
            2 =>{
                if context.out_queue.len()>queue_index{
                    (&context.out_queue[queue_index].0, Step::Out)
                } else if context.edge_queue.len()>queue_index {
                    (&context.edge_queue[queue_index].0, Step::Edge)
                } else if context.scredge_poses.len()>queue_index {
                    (&context.scredge_poses[queue_index], Step::Scredge)
                } else if context.in_queue.len()>queue_index {
                    (&context.in_queue[queue_index].0, Step::In)
                } else {context.index = total_points-1; break;
                }
            }
            3 =>{
                if context.edge_queue.len()>queue_index {
                    (&context.edge_queue[queue_index].0, Step::Edge)
                } else if context.out_queue.len()>queue_index{
                    (&context.out_queue[queue_index].0, Step::Out)
                } else if context.scredge_poses.len()>queue_index {
                    (&context.scredge_poses[queue_index], Step::Scredge)
                } else if context.in_queue.len()>queue_index {
                    (&context.in_queue[queue_index].0, Step::In)
                } else {context.index = total_points-1; break;}
            }
            4 => {
                //(&pos_from_index(context.random_index, context.res.0), Step::Random)
                if context.edge_queue.len()>0 {
                    (&context.edge_queue[0].0, Step::Edge)
                } else if context.out_queue.len()>0{
                    (&context.out_queue[0].0, Step::Out)
                } else   if context.scredge_poses.len()>0 {
                    (&context.scredge_poses[0], Step::Scredge)
                } else if context.in_queue.len()>0 {
                    (&context.in_queue[0].0, Step::In)
                } else {context.index = total_points-1; break;}
                /*let mut rng = rand::rng();

                context.attention_radius+=1;

                let mut x:i32 = rng.random_range(-50..=50);
                let mut y:i32 = rng.random_range(-50..=50);

                if x + context.attention.0 < 0 || x + context.attention.0 > context.res.0 as i32-1
                    || y + context.attention.1 < 0 || y + context.attention.1 > context.res.1 as i32-1{
                    x = 0;y=0;
                }

                (&(
                    context.attention.0 + x, context.attention.1 + y
                ), Step::Random)*/
            }
            _ => {break}
        };

        let index = index_from_pos(pos, context.res.0);

        let point = & points[index];

        let pos = pos.clone();

        if point.delivered {
            match step {
                Step::Out => {
                    let _ =  context.out_queue.remove(queue_index);
                }
                Step::Scredge => {
                    let _ = context.scredge_poses.remove(queue_index);
                }
                Step::In => {
                    let _ =  context.in_queue.remove(queue_index);
                }
                Step::Edge => {
                    let _ =  context.edge_queue.remove(queue_index);
                }
                Step::Random => {
                    context.index += 1;
                    context.random_index = context.random_map[min(context.index, total_points-1)];
                }
            }
            continue;
        } else {
            queue_index+=1;
            returned.push((pos, step));
        }
    }

    returned
}

fn multi_iterate_max_n_times_2(
    poses: &Vec<((i32, i32), Step)>
    , context: &mut WorkContext
    , r_squared:f32
    , episilon:f32
    , n:usize) {

    let points = match &mut context.points {
        Points::F32 { p} => {p}
    };
    if poses.len()==8 {
        let mut mini_workspace:[PointF32;8] = [PointF32{
            c: (0.0, 0.0)
            , z: (0.0, 0.0)
            , real_squared: 0.0
            , imag_squared: 0.0
            , real_imag: 0.0
            , iterations: 0
            , loop_detection_point: ((0.0, 0.0), 1)
            , done: (false, false)
            , delivered: false
            , period: 0
        }; 8];
        for i in 0..8 {
            let (pos, step) = &poses[i];
            let index = index_from_pos(&pos, context.res.0);
            let point = &mut points[index];
            mini_workspace[i] = point.clone()
        }

        for _ in 0..n {
            for mut point in &mut mini_workspace {
                update_point_results_f32(&mut point);
                point.done.0 = bailout_point_f32(&mut point, r_squared) || (!point.real_squared.is_finite()) || (!point.imag_squared.is_finite());
                if !(point.done.0 || point.done.1) {
                    iterate_f32(&mut point);
                } else {
                    continue;
                }
                point.done.1 = loop_check_point_f32(&mut point, episilon);
                update_loop_check_points(&mut point);
            }
        }

        for i in 0..8 {
            let (pos, step) = &poses[i];
            let index = index_from_pos(&pos, context.res.0);
            (*points)[index] = mini_workspace[i]
        }
    } else {
        for (pos, step) in poses {
            let index = index_from_pos(&pos, context.res.0);
            let mut point = &mut points[index];
            update_point_results_f32(&mut point);
            point.done.0 = bailout_point_f32(&mut point, r_squared) || (!point.real_squared.is_finite()) || (!point.imag_squared.is_finite());
            if !(point.done.0 || point.done.1) {
                iterate_f32(&mut point);
            } else {
                break;
            }
            point.done.1 = loop_check_point_f32(&mut point, episilon);
            update_loop_check_points(&mut point);
        }
    }
}


fn multi_iterate_max_n_times(
    poses: &Vec<((i32, i32), Step)>,
    context: &mut WorkContext,
    r_squared: f32,
    epsilon: f32,
    n: usize,
) {
    use wide::{f32x8, u32x8, CmpGe, CmpGt, CmpLe, CmpNe};

    let points = match &mut context.points {
        Points::F32 { p } => p,
    };

    if poses.len() == 8 {
        // Gather lanes into scalar arrays
        let mut c_r_arr = [0.0f32; 8];
        let mut c_i_arr = [0.0f32; 8];
        let mut z_r_arr = [0.0f32; 8];
        let mut z_i_arr = [0.0f32; 8];
        let mut it_arr = [0u32; 8];
        let mut lz_r_arr = [0.0f32; 8];
        let mut lz_i_arr = [0.0f32; 8];
        let mut lit_arr = [1u32; 8];
        let mut d0b = [false; 8];
        let mut d1b = [false; 8];

        for i in 0..8 {
            let (pos, _) = &poses[i];
            let idx = index_from_pos(pos, context.res.0);
            let p = points[idx];
            c_r_arr[i] = p.c.0;
            c_i_arr[i] = p.c.1;
            z_r_arr[i] = p.z.0;
            z_i_arr[i] = p.z.1;
            it_arr[i] = p.iterations;
            lz_r_arr[i] = p.loop_detection_point.0.0;
            lz_i_arr[i] = p.loop_detection_point.0.1;
            lit_arr[i] = p.loop_detection_point.1;
            d0b[i] = p.done.0;
            d1b[i] = p.done.1;
        }

        // Pack into vectors
        let c_r = f32x8::new(c_r_arr);
        let c_i = f32x8::new(c_i_arr);
        let mut zr = f32x8::new(z_r_arr);
        let mut zi = f32x8::new(z_i_arr);
        let mut iter = u32x8::new(it_arr);
        let mut loop_zr = f32x8::new(lz_r_arr);
        let mut loop_zi = f32x8::new(lz_i_arr);
        let mut loop_it = u32x8::new(lit_arr);

        // Build initial masks from bool arrays via u32x8 and CmpNe
        let d0_init_bits = u32x8::new([
            d0b[0] as u32, d0b[1] as u32, d0b[2] as u32, d0b[3] as u32,
            d0b[4] as u32, d0b[5] as u32, d0b[6] as u32, d0b[7] as u32,
        ]);
        let d1_init_bits = u32x8::new([
            d1b[0] as u32, d1b[1] as u32, d1b[2] as u32, d1b[3] as u32,
            d1b[4] as u32, d1b[5] as u32, d1b[6] as u32, d1b[7] as u32,
        ]);
        let zero_u = u32x8::splat(0u32);
        let mut done0: m32x8 = d0_init_bits.cmp_ne(zero_u);
        let mut done1: m32x8 = d1_init_bits.cmp_ne(zero_u);

        let two = f32x8::splat(2.0);
        let r2 = f32x8::splat(r_squared);
        let eps = f32x8::splat(epsilon);
        let one_u = u32x8::splat(1u32);

        for _ in 0..n {
            let active = !(done0 | done1);
            if !active.any() {
                break;
            }

            let rr = zr * zr;
            let ii = zi * zi;
            let ri = zr * zi;
            let sum = rr + ii;

            // Bailout if |z|^2 > r2 or NaN appears
            let bailout = sum.cmp_gt(r2) | sum.is_nan();

            // Update z where active and not bailing out
            let upd = active & !bailout;
            let next_zr = rr - ii + c_r;
            let next_zi = two * ri + c_i;
            zr = upd.select(next_zr, zr);
            zi = upd.select(next_zi, zi);
            iter = upd.select(iter + one_u, iter);

            // Loop detection: axis-aligned epsilon box
            let near_r = (zr - loop_zr).abs().cmp_le(eps);
            let near_i = (zi - loop_zi).abs().cmp_le(eps);
            let loop_hit = upd & (near_r & near_i);
            done1 = done1 | loop_hit;

            // Loop-check-point refresh when iter >= loop_it << 1
            let thresh = loop_it << 1u32;
            let refresh = upd & iter.cmp_ge(thresh);
            loop_zr = refresh.select(zr, loop_zr);
            loop_zi = refresh.select(zi, loop_zi);
            loop_it = refresh.select(iter, loop_it);

            // Mark bailout-done
            done0 = done0 | (active & bailout);
        }

        // Spill back to scalar arrays, recompute derived fields, and commit
        let zr_o: [f32; 8] = zr.into();
        let zi_o: [f32; 8] = zi.into();
        let it_o: [u32; 8] = iter.into();
        let lzr_o: [f32; 8] = loop_zr.into();
        let lzi_o: [f32; 8] = loop_zi.into();
        let lit_o: [u32; 8] = loop_it.into();

        let d0_bits: [u32; 8] = done0.to_int().into();
        let d1_bits: [u32; 8] = done1.to_int().into();
        let mut d0_o = [false; 8];
        let mut d1_o = [false; 8];
        for i in 0..8 {
            d0_o[i] = d0_bits[i] != 0;
            d1_o[i] = d1_bits[i] != 0;
        }

        for i in 0..8 {
            let (pos, _) = &poses[i];
            let idx = index_from_pos(pos, context.res.0);
            let p = &mut points[idx];
            p.z = (zr_o[i], zi_o[i]);
            p.iterations = it_o[i];
            p.loop_detection_point = ((lzr_o[i], lzi_o[i]), lit_o[i]);
            p.done = (d0_o[i], d1_o[i]);
            update_point_results_f32(p);
        }
    } else {
        // Fallback scalar path: correct n-iteration loop with local workspace (semantics match SIMD)
        let mut local_points: Vec<PointF32> = Vec::with_capacity(poses.len());
        for (pos, _) in poses {
            let idx = index_from_pos(pos, context.res.0);
            local_points.push(points[idx]);
        }

        for _ in 0..n {
            let mut all_done = true;
            for point in &mut local_points {
                if point.done.0 || point.done.1 {
                    continue;
                }
                all_done = false;
                update_point_results_f32(point);
                point.done.0 = bailout_point_f32(point, r_squared)
                    || (!point.real_squared.is_finite())
                    || (!point.imag_squared.is_finite());
                if !(point.done.0 || point.done.1) {
                    iterate_f32(point);
                }
                point.done.1 = loop_check_point_f32(point, epsilon);
                update_loop_check_points(point);
            }
            if all_done {
                break;
            }
        }

        // Commit back
        for (i, (pos, _)) in poses.iter().enumerate() {
            let idx = index_from_pos(pos, context.res.0);
            points[idx] = local_points[i];
        }
    }
}