use std::time::Instant;
use std::collections::*;
use std::cmp::*;
use crate::action::utils::*;
pub(crate) const NUMBER_OF_LOOP_CHECK_POINTS: usize = 5;

pub trait Floaty:
Copy
+ PartialEq
+ PartialOrd
+ std::ops::Add<Output = Self>
+ std::ops::Sub<Output = Self>
+ std::ops::Mul<Output = Self>
+ std::ops::Div<Output = Self>
+ From<f32>
+ Into<f32>
{
    fn abs(self) -> Self;
    fn is_finite(self) -> bool;
    fn zero() -> Self;
    fn one() -> Self;
}

impl Floaty for f32 {
    #[inline]
    fn abs(self) -> Self {
        f32::abs(self)
    }
    #[inline]
    fn is_finite(self) -> bool {
        f32::is_finite(self)
    }
    #[inline]
    fn zero() -> Self {
        0.0
    }
    #[inline]
    fn one() -> Self {
        1.0
    }
}

pub(crate) struct floaty64 {v:f64}

impl Floaty for floaty64 {
    #[inline]
    fn abs(self) -> Self {
        floaty64{v:f64::abs(self.v)}
    }

    #[inline]
    fn is_finite(self) -> bool {
        f64::is_finite(self.v)
    }
    #[inline]
    fn zero() -> Self {
        floaty64{v:0.0}
    }
    #[inline]
    fn one() -> Self {
        floaty64{v:1.0}
    }
}
impl From<f32> for floaty64 {
    fn from<f32>(self) -> f32 {
        self.v as f32
    }
}

#[derive(Clone, Debug)]
pub(crate) enum Step {
    Scredge,
    In,
    Out,
    Edge,
    Random,
}

#[derive(Clone, Debug)]
pub(crate) enum Points<T: Floaty> {
    Floaty { p: Vec<Point<T>> },
}

#[derive(Clone, Debug)]
pub(crate) struct WorkContext<T: Floaty> {
    pub(crate) points: Points<T>,
    pub(crate) completed_points: Vec<(CompletedPoint, usize)>,
    pub(crate) last_update: usize,
    pub(crate) index: usize,
    pub(crate) random_index: usize,
    pub(crate) time_created: Instant,
    pub(crate) time_workshift_started: Instant,
    pub(crate) percent_completed: f64,
    pub(crate) random_map: Vec<usize>,
    pub(crate) workshifts: u32,
    pub(crate) total_iterations: u32,
    pub(crate) total_iterations_today: u32,
    pub(crate) total_bouts_today: u32,
    pub(crate) total_points_today: u32,
    pub(crate) spent_tokens_today: u32,
    pub(crate) res: (u32, u32),
    pub(crate) scredge_poses: VecDeque<(i32, i32)>,
    pub(crate) edge_queue: VecDeque<((i32, i32), u32)>,
    pub(crate) out_queue: VecDeque<((i32, i32), u32)>,
    pub(crate) in_queue: VecDeque<((i32, i32), u32)>,
}

#[derive(Clone, Debug)]
pub(crate) enum CompletedPoint {
    Repeats { period: u32 },
    Escapes {
        escape_time: u32,
        escape_location: (f32, f32),
        start_location: (f32, f32),
    },
    Dummy {},
}

//pub(crate) const SpeedTestPoint
#[derive(Clone, Debug)]
pub(crate) struct Point<T: Floaty> {
    pub(crate) c: (T, T),
    pub(crate) z: (T, T),
    pub(crate) real_squared: T,
    pub(crate) imag_squared: T,
    pub(crate) real_imag: T,
    pub(crate) iterations: u32,
    pub(crate) loop_detection_point: ((T, T), u32),
    pub(crate) done: (bool, bool),
    pub(crate) delivered: bool,
    pub(crate) period: u32,
}

pub(crate) fn workshift<T: Floaty>(
    day_token_allowance: u32,
    iteration_token_cost: u32,
    point_token_cost: u32,
    bout_token_cost: u32,
    context: &mut WorkContext<T>,
) {
    match context.points {
        Points::Floaty { .. } => workshift_floaty(
            day_token_allowance,
            iteration_token_cost,
            point_token_cost,
            bout_token_cost,
            context,
        ),
    }
}

pub(crate) fn workshift_floaty<T: Floaty>(
    day_token_allowance: u32,
    iteration_token_cost: u32,
    point_token_cost: u32,
    bout_token_cost: u32,
    context: &mut WorkContext<T>,
) {
    context.time_workshift_started = Instant::now();

    context.total_bouts_today = 0;
    context.total_iterations_today = 0;
    context.total_points_today = 0;
    context.spent_tokens_today = 0;

    let points = match &mut context.points {
        Points::Floaty { p } => p,
    };
    let episilon = (points[0].c.0 - points[1].c.0).abs() / T::from(64.0);

    let total_points = points.len();
    context.random_index = context.random_map[min(context.index, total_points - 1)];

    while context.time_workshift_started.elapsed().as_millis() < 10 {
        let (pos, step) = match context.workshifts % 5 {
            0 => {
                if context.workshifts == 0 {
                    if context.scredge_poses.len() > 0 {
                        (&context.scredge_poses[0], Step::Scredge)
                    } else if context.edge_queue.len() > 0 {
                        (&context.edge_queue[0].0, Step::Edge)
                    } else if context.out_queue.len() > 0 {
                        (&context.out_queue[0].0, Step::Out)
                    } else if context.in_queue.len() > 0 {
                        (&context.in_queue[0].0, Step::In)
                    } else {
                        context.index = total_points - 1;
                        break;
                    }
                } else {
                    if context.edge_queue.len() > 0 {
                        (&context.edge_queue[0].0, Step::Edge)
                    } else if context.out_queue.len() > 0 {
                        (&context.out_queue[0].0, Step::Out)
                    } else if context.scredge_poses.len() > 0 {
                        (&context.scredge_poses[0], Step::Scredge)
                    } else if context.in_queue.len() > 0 {
                        (&context.in_queue[0].0, Step::In)
                    } else {
                        context.index = total_points - 1;
                        break;
                    }
                }
            }
            1 => {
                if context.edge_queue.len() > 0 {
                    (&context.edge_queue[0].0, Step::Edge)
                } else if context.out_queue.len() > 0 {
                    (&context.out_queue[0].0, Step::Out)
                } else if context.scredge_poses.len() > 0 {
                    (&context.scredge_poses[0], Step::Scredge)
                } else if context.in_queue.len() > 0 {
                    (&context.in_queue[0].0, Step::In)
                } else {
                    context.index = total_points - 1;
                    break;
                }
            }
            2 => {
                if context.out_queue.len() > 0 {
                    (&context.out_queue[0].0, Step::Out)
                } else if context.edge_queue.len() > 0 {
                    (&context.edge_queue[0].0, Step::Edge)
                } else if context.scredge_poses.len() > 0 {
                    (&context.scredge_poses[0], Step::Scredge)
                } else if context.in_queue.len() > 0 {
                    (&context.in_queue[0].0, Step::In)
                } else {
                    context.index = total_points - 1;
                    break;
                }
            }
            3 => {
                if context.edge_queue.len() > 0 {
                    (&context.edge_queue[0].0, Step::Edge)
                } else if context.out_queue.len() > 0 {
                    (&context.out_queue[0].0, Step::Out)
                } else if context.scredge_poses.len() > 0 {
                    (&context.scredge_poses[0], Step::Scredge)
                } else if context.in_queue.len() > 0 {
                    (&context.in_queue[0].0, Step::In)
                } else {
                    context.index = total_points - 1;
                    break;
                }
            }
            4 => {
                //(&pos_from_index(context.random_index, context.res.0), Step::Random)
                if context.edge_queue.len() > 0 {
                    (&context.edge_queue[0].0, Step::Edge)
                } else if context.out_queue.len() > 0 {
                    (&context.out_queue[0].0, Step::Out)
                } else if context.scredge_poses.len() > 0 {
                    (&context.scredge_poses[0], Step::Scredge)
                } else if context.in_queue.len() > 0 {
                    (&context.in_queue[0].0, Step::In)
                } else {
                    context.index = total_points - 1;
                    break;
                }
            }
            _ => {
                break;
            }
        };

        let index = index_from_pos(pos, context.res.0);
        let point = &mut points[index];
        let pos = pos.clone();

        if point.delivered {
            match step {
                Step::Out => {
                    let _ = context.out_queue.pop_front();
                }
                Step::Scredge => {
                    let _ = context.scredge_poses.pop_front();
                }
                Step::In => {
                    let _ = context.in_queue.pop_front();
                }
                Step::Edge => {
                    let _ = context.edge_queue.pop_front();
                }
                Step::Random => {
                    context.index += 1;
                    context.random_index =
                        context.random_map[min(context.index, total_points - 1)];
                }
            }
            continue;
        }

        //if context.workshifts > 100 {
        match step {
            Step::In => {
                point.period = context.in_queue[0].1;
                context.completed_points.push((
                    CompletedPoint::Repeats {
                        period: context.in_queue[0].1,
                    },
                    index,
                ));
                point.delivered = true;
                queue_incomplete_neighbors_in(&pos, context.res, points, &mut context.in_queue);
                let _ = context.in_queue.pop_front();
                continue;
            }
            _ => {}
        }
        //}

        let old_iterations = point.iterations;

        match step {
            Step::Scredge => {
                iterate_max_n_times(point, T::from(4.0), episilon, 1000);
            }
            Step::Random => {
                iterate_max_n_times(point, T::from(4.0), episilon, 1000);
            }
            Step::Out => {
                let difficulty = context.out_queue[0].1;
                let eta = difficulty as i32 - point.iterations as i32;
                if eta > 2000 {
                    let warp = min(eta / 2, 1000000);
                    if !timewarp_n_iterations(point, T::from(4.0), warp as u32) {
                        context.out_queue[0].1 = 0;
                    };
                } else {
                    iterate_max_n_times(point, T::from(4.0), episilon, 100);
                }
            }
            Step::In => {
                let difficulty = context.in_queue[0].1;
                let eta = difficulty as i32 - point.iterations as i32;
                if eta > 2000 {
                    let warp = min(eta / 2, 1000000);
                    if !timewarp_n_iterations(point, T::from(4.0), warp as u32) {
                        context.in_queue[0].1 = 0;
                    };
                } else {
                    iterate_max_n_times(point, T::from(4.0), episilon, 100);
                }
            }
            Step::Edge => {
                let difficulty = context.edge_queue[0].1;
                let eta = difficulty as i32 - point.iterations as i32;
                if eta > 2000 {
                    let warp = min(eta / 2, 10000000);
                    if !timewarp_n_iterations(point, T::from(4.0), warp as u32) {
                        context.edge_queue[0].1 = 0;
                    };
                } else {
                    iterate_max_n_times(point, T::from(4.0), episilon, 100);
                }
            }
        }

        /*if let Some(t) = point.escaped_time {
            let warp = (t-point.iterations)/2;
            if !timewarp_n_iterations(point, 4.0, warp) {
                point.escaped_time = Some(point.iterations + warp);
                context.total_iterations_today+=warp;
            }
        } else {
            let warp = min(point.iterations/2, 1000);
            if !timewarp_n_iterations(point, 4.0, warp) {
                point.escaped_time = Some(point.iterations + warp);
                context.total_iterations_today+=warp;
            }
        }*/

        context.total_iterations_today += point.iterations - old_iterations;

        if point.done.0 || point.done.1 {
            //context.already_done.push(context.index);
            //context.already_done_hashset.insert(context.index);
            context.total_iterations += point.iterations;

            match step {
                Step::Out => {
                    let _ = context.out_queue.pop_front();
                }
                Step::Scredge => {
                    let _ = context.scredge_poses.pop_front();
                }
                Step::In => {
                    let _ = context.in_queue.pop_front();
                }
                Step::Edge => {
                    let _ = context.edge_queue.pop_front();
                }
                Step::Random => {
                    context.index += 1;
                    context.random_index =
                        context.random_map[min(context.index, total_points - 1)];
                }
            }

            point.delivered = true;

            let completed_point = if point.done.1 {
                let raw_period = point.iterations - point.loop_detection_point.1;
                let returned = if determine_period(point, episilon) {
                    point.period = point.iterations - point.loop_detection_point.1;
                    CompletedPoint::Repeats {
                        period: point.period,
                    }
                } else {
                    point.period = raw_period;
                    CompletedPoint::Repeats {
                        period: point.period,
                    }
                };
                queue_incomplete_neighbors_in(&pos, context.res, points, &mut context.in_queue);
                returned
            } else {
                let result = CompletedPoint::Escapes {
                    escape_time: point.iterations,
                    escape_location: (point.z.0.into(), point.z.1.into()),
                    start_location: (point.c.0.into(), point.c.1.into()),
                };
                queue_incomplete_neighbors(&pos, context.res, points, &mut context.out_queue);
                result
            };
            if let Some(e) = point_is_edge(&pos, context.res, points) {
                //context.edge_queue.clear();
                queue_incomplete_neighbors_of_edge(
                    &e.0,
                    &e.1,
                    context.res,
                    points,
                    &mut context.edge_queue,
                );
            }

            context.completed_points.push((completed_point, index));

            context.total_points_today += 1
        } else {
            match step {
                Step::Out => {
                    let pos = context.out_queue.pop_front().unwrap();
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
                        CompletedPoint::Repeats {
                            period: point.iterations - point.loop_detection_point.1,
                        }
                    };
                    context.completed_points.push((completed_point, index));
                    continue;
                }
                _ => {}
            }
        }

        context.total_bouts_today += 1;
        context.spent_tokens_today = context.total_bouts_today * bout_token_cost
            + context.total_points_today * point_token_cost
            + context.total_iterations_today * point_token_cost;
    }

    context.workshifts += 1;
    context.percent_completed = context.index as f64 / (total_points) as f64 * 100.0;
}

#[inline]
pub(crate) fn iterate_max_n_times<T: Floaty>(
    point: &mut Point<T>,
    r_squared: T,
    epsilon: T,
    n: u32,
) {
    for _ in 0..n {
        update_point_results(point);
        point.done.0 = bailout_point(point, r_squared)
            || (!point.real_squared.is_finite())
            || (!point.imag_squared.is_finite());
        if !(point.done.0 || point.done.1) {
            iterate(point);
        } else {
            break;
        }
        point.done.1 = loop_check_point(point, epsilon);
        update_loop_check_points(point);
    }
}

#[inline]
pub(crate) fn timewarp_n_iterations<T: Floaty>(point: &mut Point<T>, r_squared: T, n: u32) -> bool {
    let c = point.c.clone();
    let mut z = point.z.clone();

    let blocks = n / 4096;
    let change = n % 4096;

    for _ in 0..blocks {
        timewarp_4096(&mut z, c);
    }
    for _ in 0..change {
        z = (z.0 * z.0 - z.1 * z.1 + c.0, T::from(2.0) * z.0 * z.1 + c.1);
    }

    let backup = point.clone();
    point.z = z;
    update_point_results(point);

    if bailout_point(point, r_squared)
        || (!point.real_squared.is_finite())
        || (!point.imag_squared.is_finite())
    {
        *point = backup;
        false
    } else {
        point.iterations += n;
        true
    }
}

#[inline(always)]
fn timewarp_4096<T: Floaty>(z: &mut (T, T), c: (T, T)) {
    for _ in 0..4096 {
        *z = (z.0 * z.0 - z.1 * z.1 + c.0, T::from(2.0) * z.0 * z.1 + c.1);
    }
}

#[inline(always)]
pub(crate) fn iterate<T: Floaty>(point: &mut Point<T>) {
    // move z
    point.z = (point.real_squared - point.imag_squared + point.c.0, T::from(2.0) * point.real_imag + point.c.1);
    point.iterations += 1;
}

#[inline(always)]
pub(crate) fn bailout_point<T: Floaty>(point: &Point<T>, r_squared: T) -> bool {
    // checks
    point.real_squared + point.imag_squared > r_squared
}

#[inline(always)]
fn points_near<T: Floaty>(z1: (T, T), z2: (T, T), e: T) -> bool {
    z1.0 >= (z2.0 - e)
        && z1.0 <= (z2.0 + e)
        && z1.1 >= (z2.1 - e)
        && z1.1 <= (z2.1 + e)
}

#[inline(always)]
fn loop_check_point<T: Floaty>(point: &Point<T>, epsilon: T) -> bool {
    points_near(point.z, point.loop_detection_point.0, epsilon)
}

#[inline(always)]
fn update_loop_check_points<T: Floaty>(point: &mut Point<T>) {
    if point.iterations >= point.loop_detection_point.1 << 1 {
        point.loop_detection_point = (point.z, point.iterations);
    }
}

fn determine_period<T: Floaty>(point: &mut Point<T>, epsilon: T) -> bool {
    let max_period = 1000;

    timewarp_n_iterations(point, T::from(4.0), 1000);

    point.loop_detection_point = (point.z, point.iterations);
    for _ in 0..max_period {
        update_point_results(point);
        iterate(point);
        if loop_check_point(point, T::from(0.0001)) {
            return true;
        }
    }
    return false;
}

#[inline]
pub(crate) fn update_point_results<T: Floaty>(point: &mut Point<T>) {
    // update values
    point.real_squared = point.z.0 * point.z.0;
    point.imag_squared = point.z.1 * point.z.1;
    point.real_imag = point.z.0 * point.z.1;
}

#[inline]
pub(crate) fn index_from_pos(pos: &(i32, i32), wid: u32) -> usize {
    (pos.0 + pos.1 * wid as i32) as usize
}

pub(crate) fn pos_from_index(i: usize, wid: u32) -> (i32, i32) {
    (i as i32 % wid as i32, i as i32 / wid as i32)
}

pub(crate) fn queue_incomplete_neighbors<T: Floaty>(
    pos: &(i32, i32),
    res: (u32, u32),
    points: &Vec<Point<T>>,
    queue: &mut VecDeque<((i32, i32), u32)>,
) {
    let difficulty = points[index_from_pos(pos, res.0)].iterations;
    let wid = res.0;

    let neighbors: [(i32, i32); 4] = [
        (pos.0 + 1, pos.1),
        (pos.0 - 1, pos.1),
        (pos.0, pos.1 + 1),
        (pos.0, pos.1 - 1),
    ];
    for n in neighbors {
        if n.0 >= 0
            && n.0 <= res.0 as i32 - 1
            && n.1 >= 0
            && n.1 <= res.1 as i32 - 1
        {
            let index = index_from_pos(&n, wid);
            if !(points[index].done.0 || points[index].done.1) {
                queue.push_back((n, difficulty));
            }
        }
    }
}

pub(crate) fn queue_incomplete_neighbors_in<T: Floaty>(
    pos: &(i32, i32),
    res: (u32, u32),
    points: &Vec<Point<T>>,
    queue: &mut VecDeque<((i32, i32), u32)>,
) {
    let period = points[index_from_pos(&pos, res.0)].period;
    let wid = res.0;

    let neighbors: [(i32, i32); 4] = [
        (pos.0 + 1, pos.1),
        (pos.0 - 1, pos.1),
        (pos.0, pos.1 + 1),
        (pos.0, pos.1 - 1),
    ];
    for n in neighbors {
        if n.0 >= 0
            && n.0 <= res.0 as i32 - 1
            && n.1 >= 0
            && n.1 <= res.1 as i32 - 1
        {
            let index = index_from_pos(&n, wid);
            if !(points[index].done.0 || points[index].done.1) {
                queue.push_back((n, period));
            }
        }
    }
}

pub(crate) fn point_is_edge<T: Floaty>(
    pos: &(i32, i32),
    res: (u32, u32),
    points: &Vec<Point<T>>,
) -> Option<((i32, i32), (i32, i32))> {
    let neighbors: [(i32, i32); 4] = [
        (pos.0 + 1, pos.1),
        (pos.0 - 1, pos.1),
        (pos.0, pos.1 + 1),
        (pos.0, pos.1 - 1),
    ];

    let index = index_from_pos(&pos, res.0);
    for n in neighbors {
        if n.0 >= 0
            && n.0 <= res.0 as i32 - 1
            && n.1 >= 0
            && n.1 <= res.1 as i32 - 1
        {
            let nindex = index_from_pos(&n, res.0);
            if (points[index].done.0 || points[index].done.1)
                && (points[nindex].done.0 || points[nindex].done.1)
            {
                if points[index].done != points[nindex].done {
                    return Some((*pos, n));
                } else if points[index].done.1 == true {
                    if points[index].period != points[nindex].period {
                        return Some((*pos, n));
                    }
                }
            }
        }
    }
    None
}

pub(crate) fn queue_incomplete_neighbors_of_edge<T: Floaty>(
    pos1: &(i32, i32),
    pos2: &(i32, i32),
    res: (u32, u32),
    points: &Vec<Point<T>>,
    queue: &mut VecDeque<((i32, i32), u32)>,
) {
    let difficulty = points[index_from_pos(pos1, res.0)].iterations;
    let wid = res.0;

    let neighbors: [(i32, i32); 8] = if (pos1.0 - pos2.0).abs() == 1 {
        if pos1.0 > pos2.0 {
            [
                (pos1.0, pos1.1 + 1),
                (pos2.0, pos2.1 + 1),
                (pos2.0, pos2.1 - 1),
                (pos1.0, pos1.1 - 1),
                (pos1.0 + 1, pos1.1 + 1),
                (pos2.0 - 1, pos2.1 + 1),
                (pos2.0 - 1, pos2.1 - 1),
                (pos1.0 + 1, pos1.1 - 1),
            ]
        } else {
            [
                (pos2.0, pos2.1 + 1),
                (pos1.0, pos1.1 + 1),
                (pos1.0, pos1.1 - 1),
                (pos2.0, pos2.1 - 1),
                (pos2.0 + 1, pos2.1 + 1),
                (pos1.0 - 1, pos1.1 + 1),
                (pos1.0 - 1, pos1.1 - 1),
                (pos2.0 + 1, pos2.1 - 1),
            ]
        }
    } else {
        if pos1.0 > pos2.0 {
            [
                (pos1.0 + 1, pos1.1),
                (pos2.0 + 1, pos2.1),
                (pos1.0 - 1, pos1.1),
                (pos2.0 - 1, pos2.1),
                (pos1.0 + 1, pos1.1 + 1),
                (pos2.0 + 1, pos2.1 - 1),
                (pos2.0 - 1, pos2.1 - 1),
                (pos1.0 - 1, pos1.1 + 1),
            ]
        } else {
            [
                (pos1.0 + 1, pos1.1),
                (pos2.0 + 1, pos2.1),
                (pos2.0 - 1, pos2.1),
                (pos1.0 - 1, pos1.1),
                (pos2.0 + 1, pos2.1 + 1),
                (pos1.0 + 1, pos1.1 - 1),
                (pos1.0 - 1, pos1.1 - 1),
                (pos2.0 - 1, pos2.1 + 1),
            ]
        }
    };

    for n in neighbors {
        if n.0 >= 0
            && n.0 <= res.0 as i32 - 1
            && n.1 >= 0
            && n.1 <= res.1 as i32 - 1
        {
            let index = index_from_pos(&n, wid);
            if !(points[index].done.0 || points[index].done.1) {
                queue.push_front((n, difficulty));
            }
        }
    }
}

// compatibility type aliases
pub type PointF32 = Point<f32>;
pub type PointF64 = Point<f64>;
pub type WorkContextF32 = WorkContext<f32>;
pub type WorkContextF64 = WorkContext<f64>;