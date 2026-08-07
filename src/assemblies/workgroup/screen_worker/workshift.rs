
use rand::Rng;
use rand::prelude::SliceRandom;

use std::time::Instant;
use std::collections::*;
use std::cmp::*;
use crate::assemblies::workgroup::c_generator::{CGenerator, Mandelbrotable};
use crate::utils::*;
pub const NUMBER_OF_LOOP_CHECK_POINTS: usize = 5;

pub const MAX_PIXELS:usize = 1920*1080*4;

#[derive(Clone, Debug)]
pub enum Step {Scredge, In, Out, Edge, Random}


pub trait Floaty: Sub<Output=Self> + Add<Output=Self> + Mul<Output=Self> + Into<f64> + PartialOrd + Finite + Gt + Abs + From<f32> + Into<f64> + Copy {}

#[derive(Clone, Debug)]
pub struct Stec<T: Copy> {
    pub stuff: Vec<T>
    , pub len: usize
}

impl<T: Copy> Stec<T> {
    pub fn with_capacity(cap: usize, fill: T) -> Self {
        let mut stuff = Vec::with_capacity(cap);
        stuff.resize(cap, fill);
        Self { stuff, len: 0 }
    }

    pub fn try_push(&mut self, thing:T) -> bool {
        if self.len < self.stuff.len() {
            self.stuff[self.len] = thing;
            self.len+=1;
            true
        } else {
            false
        }
    }
    pub fn try_pop(&mut self) -> Option<T> {
        if self.len > 0 {
            self.len-=1;
            Some(self.stuff[self.len])
        } else {
            None
        }
    }
}


use std::collections::*;
#[derive(Clone, Debug)]
pub struct WorkContext<T: Mandelbrotable> {
    pub points: Vec<Point<T>>
    , pub completed_points: Stec<(CompletedPoint<T>, usize)>
    , pub last_update: usize
    , pub index: usize
    , pub random_index: usize
    , pub time_created: Instant
    , pub time_workshift_started: Instant
    , pub percent_completed:f64
    , pub random_map: Vec<usize>
    , pub workshifts: u32
    , pub total_iterations: u32
    , pub total_iterations_today: u32
    , pub total_bouts_today: u32
    , pub total_points_today: u32
    , pub spent_tokens_today: u32
    , pub res: (u32, u32)
    , pub scredge_poses: VecDeque<(i32, i32)>
    , pub edge_queue: VecDeque<((i32, i32), u32)>
    , pub out_queue: VecDeque<((i32, i32), u32)>
    , pub in_queue: VecDeque<((i32, i32), u32)>
    , pub zoomed: bool
    , pub attention: (i32, i32)
    , pub c_generator: CGenerator<T>
    , pub pitch_epsilon: T
}


#[derive(Clone, Copy, Debug)]
pub enum CompletedPoint<T> {
    Repeats{
        period: u32,
        smallness: T,
        small_time: u32
    }
    , Escapes{
        escape_time: u32
        , escape_location: (T, T)
        , escape_derivative: (T, T)
        , start_location: (T, T)
        , smallness: T
        , small_time: u32
    }
    , Dummy{}
}


//pub const SpeedTestPoint
#[derive(Clone, Debug)]

pub struct Point<T> {
    pub c: (T, T)
    , pub z: (T, T)
    , pub dc: (T, T)
    , pub real_squared: T
    , pub imag_squared: T
    , pub real_imag: T
    , pub iterations: u32
    , pub loop_detection_point: ((T, T), u32)
    , pub escapes: bool
    , pub repeats: bool
    , pub delivered: bool
    // Seat coordinates materialize at first start from `c_generator`.
    , pub initialized: bool
    , pub period: u32
    , pub smallness_squared: T
    , pub small_time: u32
}




pub trait Abs {
    fn abs(self) -> Self;
}
impl Abs for f32 {
    fn abs(self) -> Self {
        self.abs()
    }
}
impl Abs for f64 {
    fn abs(self) -> Self {
        self.abs()
    }
}
pub trait Gt {
    fn gt(self, a:Self) -> bool;
}

impl Gt for f32 {
    fn gt(self, a:Self) -> bool {
        self > a
    }
}
impl Gt for f64 {
    fn gt(self, a:Self) -> bool {
        self > a
    }
}


// r[impl cz.craft.epsilon-pixel-pitch+1]
pub fn pitch_epsilon<T:Sub<Output=T> + Abs + From<f32> + Mul<Output=T> + Copy>(points: &Vec<Point<T>>) -> T {
    (points[0].c.0 - points[1].c.0).abs() * (T::from(1.0 * (1.0/256.0)))
}

pub fn placeholder_point<T: From<f32> + Copy>() -> Point<T> {
    Point {
        c: (0.0.into(), 0.0.into()),
        z: (0.0.into(), 0.0.into()),
        dc: (1.0.into(), 0.0.into()),
        real_squared: 0.0.into(),
        imag_squared: 0.0.into(),
        real_imag: 0.0.into(),
        iterations: 0,
        loop_detection_point: ((0.0.into(), 0.0.into()), 0),
        escapes: false,
        repeats: false,
        delivered: false,
        initialized: false,
        period: 0,
        smallness_squared: 100.0.into(),
        small_time: 0,
    }
}

// r[impl cz.craft.mixmap-shuffle+1]
pub(crate) fn get_random_mixmap(size: usize) -> Vec<usize> {
    let mut rng = rand::rng();
    let mut indices: Vec<usize> = (0..size).collect();
    indices.shuffle(&mut rng);
    indices
}

/// Build an O(1)-coordinate shell from a stencil. Reuses the previous context's
/// point/mixmap buffers when present so steady-zoom pivots avoid large reallocs.
///
/// `previous` is `(old_context, old_zoom_pot)`.
// r[impl cz.craft.stencil-only-replace+2]
pub fn from_stencil<T: Mandelbrotable + From<f32>>(
    frame_info: (ObjectivePosAndZoom, (u32, u32)),
    previous: Option<(WorkContext<T>, i32)>,
) -> Option<WorkContext<T>> {
    let (obj, res) = frame_info;
    let compute_loc = (obj.pos.0.clone(), IntExp::ZERO - obj.pos.1.clone());
    let c_generator = CGenerator::<T>::new(&compute_loc, obj.zoom_pot as i64, res)?;
    let (_, space) = c_generator.origin_and_space();
    let pitch_epsilon = space.abs() * T::from(1.0 / 256.0);

    let zoomed = previous
        .as_ref()
        .map(|(_, old_zoom)| obj.zoom_pot > *old_zoom)
        .unwrap_or(false);

    let new_len = (res.0 * res.1) as usize;
    let (mut points, mut random_map, old_res, mut completed_points) = match previous {
        Some((old, _)) => {
            let WorkContext {
                points,
                random_map,
                res: old_res,
                mut completed_points,
                ..
            } = old;
            completed_points.len = 0;
            (points, random_map, old_res, completed_points)
        }
        None => (
            Vec::new(),
            Vec::new(),
            (0, 0),
            Stec::with_capacity(100000, (CompletedPoint::Dummy {}, 0)),
        ),
    };

    points.clear();
    points.resize_with(new_len, placeholder_point);

    if old_res != res || random_map.len() != new_len {
        random_map = get_random_mixmap(new_len);
    }

    let mut edges = Vec::new();
    for i in 0..(res.0 - 1) as i32 {
        edges.push((i, 0));
    }
    for i in 0..(res.1 - 1) as i32 {
        edges.push(((res.0 - 1) as i32, i));
    }
    for i in 0..(res.0) as i32 {
        edges.push((i, (res.1 - 1) as i32));
    }
    for i in 1..(res.1 - 1) as i32 {
        edges.push((0, i));
    }
    {
        let mut rng = rand::rng();
        edges.shuffle(&mut rng);
    }

    Some(WorkContext {
        points,
        completed_points,
        index: 0,
        random_index: 0,
        time_created: Instant::now(),
        time_workshift_started: Instant::now(),
        percent_completed: 0.0,
        random_map,
        workshifts: 0,
        total_iterations: 0,
        spent_tokens_today: 0,
        total_iterations_today: 0,
        total_points_today: 0,
        total_bouts_today: 0,
        last_update: 0,
        res,
        scredge_poses: VecDeque::from(edges),
        edge_queue: VecDeque::new(),
        out_queue: VecDeque::new(),
        in_queue: VecDeque::new(),
        zoomed,
        attention: (0, 0),
        c_generator,
        pitch_epsilon,
    })
}

/// Materialize seat coordinates from the generator on first start.
// r[impl cz.craft.stencil-only-replace+2]
#[inline]
pub fn ensure_started<T: Mandelbrotable>(ctx: &mut WorkContext<T>, pos: (i32, i32)) {
    let index = index_from_pos(&pos, ctx.res.0);
    let point = &mut ctx.points[index];
    if !point.initialized {
        let c = ctx.c_generator.get_c((pos.0 as u32, pos.1 as u32));
        point.c = c;
        point.z = c;
        point.dc = (T::ONE, T::ZERO);
        point.initialized = true;
    }
}

pub fn workshift<T: Mandelbrotable + Sub<Output=T> + std::fmt::Debug + Add<Output=T> + Mul<Output=T> + Into<f64> + PartialOrd + Finite + Gt + Abs + From<f32> + Into<f64> + Copy>(
    day_token_allowance: u32
    , iteration_token_cost: u32
    , point_token_cost: u32
    , bout_token_cost: u32
    , context: &mut WorkContext<T>
) {

    context.time_workshift_started = Instant::now();


    context.total_bouts_today = 0;
    context.total_iterations_today = 0;
    context.total_points_today = 0;
    context.spent_tokens_today = 0;



    // r[impl cz.craft.epsilon-pixel-pitch+1]
    let episilon = context.pitch_epsilon;


    let total_points = context.points.len();
    context.random_index = context.random_map[min(context.index, total_points-1)];


    // r[impl cz.craft.wall-clock-law+1]
    while context.time_workshift_started.elapsed().as_millis()<10{//while context.index < total_points && context.spent_tokens_today + bout_token_cost + 1000 * iteration_token_cost * point_token_cost < day_token_allowance { // workbout loop


        // r[impl cz.craft.scredge-first-shift0+1]
        let (pos, step) = match context.workshifts%5 {
            0 => {
                if context.workshifts == 0 {
                    if context.scredge_poses.len()>0 {
                        (&context.scredge_poses[0], Step::Scredge)
                    } else if context.edge_queue.len()>0 {
                        (&context.edge_queue[0].0, Step::Edge)
                    } else if context.out_queue.len()>0{
                        (&context.out_queue[0].0, Step::Out)
                    } else if context.in_queue.len()>0 {
                        (&context.in_queue[0].0, Step::In)
                    } else {context.index = total_points-1; break;
                    }
                } else {
                    if context.edge_queue.len()>0 {
                        (&context.edge_queue[0].0, Step::Edge)
                    } else if context.out_queue.len()>0{
                        (&context.out_queue[0].0, Step::Out)
                    } else if context.scredge_poses.len()>0 {
                        (&context.scredge_poses[0], Step::Scredge)
                    } else if context.in_queue.len()>0 {
                        (&context.in_queue[0].0, Step::In)
                    } else {context.index = total_points-1; break;
                    }
                }
            }
            1 => {
                if context.edge_queue.len()>0 {
                    (&context.edge_queue[0].0, Step::Edge)
                } else if context.out_queue.len()>0{
                    (&context.out_queue[0].0, Step::Out)
                } else if context.scredge_poses.len()>0 {
                    (&context.scredge_poses[0], Step::Scredge)
                } else if context.in_queue.len()>0 {
                    (&context.in_queue[0].0, Step::In)
                } else {context.index = total_points-1; break;}
            }
            2 =>{
                if context.out_queue.len()>0{
                    (&context.out_queue[0].0, Step::Out)
                } else if context.edge_queue.len()>0 {
                    (&context.edge_queue[0].0, Step::Edge)
                } else if context.scredge_poses.len()>0 {
                    (&context.scredge_poses[0], Step::Scredge)
                } else if context.in_queue.len()>0 {
                    (&context.in_queue[0].0, Step::In)
                } else {context.index = total_points-1; break;
                }
            }
            3 =>{
                if context.edge_queue.len()>0 {
                    (&context.edge_queue[0].0, Step::Edge)
                } else if context.out_queue.len()>0{
                    (&context.out_queue[0].0, Step::Out)
                } else if context.scredge_poses.len()>0 {
                    (&context.scredge_poses[0], Step::Scredge)
                } else if context.in_queue.len()>0 {
                    (&context.in_queue[0].0, Step::In)
                } else {context.index = total_points-1; break;}
            }
            4 => {
                //(&pos_from_index(context.random_index, context.res.0), Step::Random)
                /*if context.edge_queue.len()>0 {
                    (&context.edge_queue[0].0, Step::Edge)
                } else if context.out_queue.len()>0{
                    (&context.out_queue[0].0, Step::Out)
                } else   if context.scredge_poses.len()>0 {
                    (&context.scredge_poses[0], Step::Scredge)
                } else if context.in_queue.len()>0 {
                    (&context.in_queue[0].0, Step::In)
                } else {context.index = total_points-1; break;}*/
                let mut rng = rand::rng();
                let mut x:i32 = rng.random_range(-50..50);
                let mut y:i32 = rng.random_range(-50..50);

                if x + context.attention.0 < 0 || x + context.attention.0 > context.res.0 as i32-1
                || y + context.attention.1 < 0 || y + context.attention.1 > context.res.1 as i32-1{
                    x = 0;y=0;
                }

                let p = &context.points[index_from_pos(&context.attention, context.res.0)];
                //println!("selected point: {:?}", p);

                (&(
                    context.attention.0 + x, context.attention.1 + y
                ), Step::Random)
            }
            _ => {break}
        };

        let index = index_from_pos(pos, context.res.0);
        let pos = pos.clone();

        if context.points[index].delivered {
            match step {
                Step::Out => {
                    let _ =  context.out_queue.pop_front();
                }
                Step::Scredge => {
                    let _ = context.scredge_poses.pop_front();
                }
                Step::In => {
                    let _ =  context.in_queue.pop_front();
                }
                Step::Edge => {
                    let _ =  context.edge_queue.pop_front();
                }
                Step::Random => {
                    context.index += 1;
                    context.random_index = context.random_map[min(context.index, total_points-1)];
                }
            }
            continue;
        }

        // r[impl cz.craft.stencil-only-replace+2]
        ensure_started(context, pos);
        let point = &mut context.points[index];

        let old_iterations = point.iterations;


        iterate_max_n_times(point, 4.0f32.into(), episilon, 1000);



        context.total_iterations_today += point.iterations - old_iterations;


        if point.repeats || point.escapes {

            //context.already_done.push(context.index);
            //context.already_done_hashset.insert(context.index);
            context.total_iterations += point.iterations;



            match step {
                Step::Out => {
                    let _ =  context.out_queue.pop_front();
                }
                Step::Scredge => {
                    let _ = context.scredge_poses.pop_front();
                }
                Step::In => {
                    let _ =  context.in_queue.pop_front();
                }
                Step::Edge => {
                    let _ =  context.edge_queue.pop_front();
                }
                Step::Random => {
                    context.index += 1;
                    context.random_index = context.random_map[min(context.index, total_points-1)];
                }
            }

            point.delivered = true;



            let completed_point = if point.repeats {
                // Candidate periods are the atom-domain partials, tried ascending;
                // the first that survives Newton + the multiplier test is the true
                // period. 0 means "repeats, period unknown" — never a guessed number.
                let c64 = (point.c.0.into(), point.c.1.into());
                let (partials, tail) = period_partials(c64, point.iterations);
                point.period = partials
                    .into_iter()
                    .find_map(|p| verified_period_from(c64, p, tail))
                    .unwrap_or(0);
                let returned = CompletedPoint::Repeats{period: point.period, smallness: point.smallness_squared, small_time:point.small_time};
                queue_incomplete_neighbors_in(&pos, context.res, &context.points, &mut context.in_queue);
                returned

            } else {
                let result = CompletedPoint::Escapes {
                    escape_time: point.iterations
                    , escape_location: (point.z.0, point.z.1)
                    , escape_derivative: point.dc
                    , start_location: (point.c.0, point.c.1)
                    , smallness: point.smallness_squared
                    , small_time:point.small_time
                };
                queue_incomplete_neighbors(&pos, context.res, &context.points, &mut context.out_queue);
                result
            };
            if let Some(e) = point_is_edge(&pos, context.res, &context.points) {
                //context.edge_queue.clear();
                queue_incomplete_neighbors_of_edge(&e.0, &e.1, context.res, &context.points, &mut context.edge_queue);
            }

            if context.completed_points.try_push((completed_point, index)) {} else {
                // r[impl cz.craft.undeliver-on-full+1]
                let point = &mut context.points[index];
                point.delivered=false;
                break;
            };


            context.total_points_today += 1
        } else {
            match step {
                // r[impl cz.craft.out-rotates-in-stays+1]
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
                    // r[impl cz.craft.provisional-not-delivered+1]
                    let completed_point = {
                        // This point has not completed and its checkpoint gap is
                        // not a verified period. Publish 0 ("unknown") so the
                        // provisional frame remains current without inventing
                        // period edges. Completion replaces it with a period
                        // verified by Newton and the cycle multiplier.
                        CompletedPoint::Repeats{period: 0, smallness: point.smallness_squared, small_time:point.small_time}
                    };
                    if context.completed_points.try_push((completed_point, index)) {} else {
                        break;
                    };
                    continue;
                }
                _ => {}
            }
        }

        context.total_bouts_today += 1;
        context.spent_tokens_today = context.total_bouts_today * bout_token_cost + context.total_points_today * point_token_cost + context.total_iterations_today * point_token_cost;
    }

    context.workshifts += 1;
    context.percent_completed = context.index as f64 / (total_points) as f64 * 100.0;
}

#[inline]
pub fn iterate_max_n_times<T:Sub<Output=T> + Add<Output=T> + Mul<Output=T> + Into<f64>+ PartialOrd + Gt +From<f32>+ Copy> (point: &mut Point<T>, r_squared:T, epsilon:T, n: u32) {
    for i in 0..n {
        update_point_results(point);
        point.escapes = bailout_point(point, r_squared);// || (!point.real_squared.is_finite()) || (!point.imag_squared.is_finite());
        if !(point.escapes || point.repeats) {
            iterate(point);
        } else {
            break;
        }
        point.repeats = loop_check_point(point, epsilon);
        update_loop_check_points(point);
    }
}


pub trait Finite {
    fn is_finite(self) -> bool;
}
impl Finite for f32 {
    fn is_finite(self) -> bool {
        self.is_finite()
    }
}
impl Finite for f64 {
    fn is_finite(self) -> bool {
        self.is_finite()
    }
}


use std::ops::*;

#[inline(always)]
pub fn iterate<T:Sub<Output=T> + Add<Output=T> + Mul<Output=T> + From<f32> + Copy> (point: &mut Point<T>) {
    // r[impl cz.craft.screen-space-derivative-edges+1]
    // dc_n = dz_n/dc follows dc_{n+1} = 2 z_n dc_n + 1.
    let dc = (
        T::from(2.0) * (point.z.0 * point.dc.0 - point.z.1 * point.dc.1) + T::from(1.0),
        T::from(2.0) * (point.z.0 * point.dc.1 + point.z.1 * point.dc.0),
    );
    // move z
    point.z = (
        point.real_squared - point.imag_squared + point.c.0
        , T::from(2.0f32.into()) * point.real_imag + point.c.1
    );
    point.dc = dc;
    point.iterations+=1;
}

use std::cmp::*;
#[inline(always)]
pub fn bailout_point<T:Sub<Output=T> + Add<Output=T> + Mul<Output=T> + Gt + PartialOrd + Copy> (point: & Point<T>, r_squared:T) -> bool {
    // checks

    point.real_squared + point.imag_squared > r_squared
}

#[inline(always)]
fn points_near<T:Sub<Output=T> + Add<Output=T> + Mul<Output=T> + PartialOrd + Copy> (z1: (T, T), z2: (T, T), e: T) -> bool {
    z1.0 >= (z2.0 - e) && z1.0 <= (z2.0 + e)
    && z1.1 >= (z2.1 - e) && z1.1 <= (z2.1 + e)
}

#[inline(always)]
fn loop_check_point<T:Sub<Output=T> + Add<Output=T> + PartialOrd + Mul<Output=T> + Copy> (point: &mut  Point<T>, epsilon:T) -> bool {
    let near = points_near(point.z, point.loop_detection_point.0, epsilon);

    if near {point.period = point.iterations-point.loop_detection_point.1}
    near
}

#[inline(always)]
fn update_loop_check_points<T:Sub<Output=T> + Add<Output=T> + Mul<Output=T> + Copy> (point: &mut Point<T>) {

    if point.iterations >= point.loop_detection_point.1 << 1 {
        point.loop_detection_point = (point.z, point.iterations);
    }

}

fn iterate_complex(z: (f64, f64), c: (f64, f64)) -> (f64, f64) {
    (z.0 * z.0 - z.1 * z.1 + c.0, 2.0 * z.0 * z.1 + c.1)
}

// Record-minimum steps of the critical orbit (atom-domain partials), ascending,
// plus the tail iterate after `max` steps. Candidates must be tried in this
// order: the first that verifies is the true period. Using only the LAST record
// (e.g. small_time) is wrong — an interior orbit keeps setting minima as it
// converges, so the last record is the convergence time, and Newton then
// verifies a multiple of the true period. The tail iterate rides the attracting
// cycle and is the best Newton start; F^p(0,c) (the published guess) is far
// from the attractor exactly where necks make convergence slow.
pub fn period_partials(c: (f64, f64), max: u32) -> (Vec<u32>, (f64, f64)) {
    let mut z = c; // z_1: this codebase's orbit convention starts at c
    let mut best = f64::MAX;
    let mut out = Vec::new();
    for n in 1..=max {
        let r = z.0 * z.0 + z.1 * z.1;
        if r < best {
            best = r;
            out.push(n);
        }
        z = iterate_complex(z, c);
    }
    (out, z)
}

// r[impl cz.craft.period-derivative-test+1]
pub fn verified_period(c: (f64, f64), period: u32) -> Option<u32> {
    // F^p(0,c) is the published Newton starting point.
    let mut w = (0.0, 0.0);
    for _ in 0..period {
        w = iterate_complex(w, c);
    }
    verified_period_from(c, period, w)
}

// r[impl cz.craft.period-derivative-test+1]
pub fn verified_period_from(
    c: (f64, f64),
    period: u32,
    start: (f64, f64),
) -> Option<u32> {
    if period == 0 {
        return None;
    }

    let mut w = start;

    // Solve F^p(w,c)=w.  Stop only when another Newton step is exactly
    // unrepresentable in f64; the mathematical acceptance test below has no epsilon.
    // Budget is generous because necks between hyperbolic components are
    // parabolic (multiplier on the unit circle): convergence is linear there,
    // with quadratic behavior only resuming within ~δ of the attractor.
    let mut converged = false;
    let mut previous = None;
    for _ in 0..128 {
        let mut z = w;
        let mut dz = (1.0, 0.0);
        for _ in 0..period {
            dz = (
                2.0 * (z.0 * dz.0 - z.1 * dz.1),
                2.0 * (z.0 * dz.1 + z.1 * dz.0),
            );
            z = iterate_complex(z, c);
        }

        let numerator = (z.0 - w.0, z.1 - w.1);
        let denominator = (dz.0 - 1.0, dz.1);
        let denominator_norm = denominator.0 * denominator.0
            + denominator.1 * denominator.1;
        if denominator_norm == 0.0 || !denominator_norm.is_finite() {
            return None;
        }
        let quotient = (
            (numerator.0 * denominator.0 + numerator.1 * denominator.1)
                / denominator_norm,
            (numerator.1 * denominator.0 - numerator.0 * denominator.1)
                / denominator_norm,
        );
        let next = (w.0 - quotient.0, w.1 - quotient.1);
        if !next.0.is_finite() || !next.1.is_finite() {
            return None;
        }
        let correction_norm = quotient.0 * quotient.0 + quotient.1 * quotient.1;
        let scale = (w.0 * w.0 + w.1 * w.1).max(1.0);
        if next == w
            || previous == Some(next)
            || correction_norm <= f64::EPSILON * f64::EPSILON * scale
        {
            converged = true;
            w = next;
            break;
        }
        previous = Some(w);
        w = next;
    }
    if !converged {
        return None;
    }

    // Newton can land on a divisor of the candidate period (fixed points also
    // satisfy F^p(w)=w). Reduce to the true minimal period of the converged
    // attractor before the multiplier test; the cycle points are O(1) apart,
    // so the closeness bound here is only absorbing f64 roundoff, not deciding
    // membership (that is the exact unit-circle multiplier test below).
    let mut z = w;
    let mut minimal_period = period;
    for d in 1..period {
        z = iterate_complex(z, c);
        if period % d == 0 {
            let diff = (z.0 - w.0, z.1 - w.1);
            let norm = diff.0 * diff.0 + diff.1 * diff.1;
            let scale = (w.0 * w.0 + w.1 * w.1).max(1.0);
            if norm <= 1e-24 * scale {
                minimal_period = d;
                break;
            }
        }
    }

    // The cycle multiplier is exact in the mathematical model: attracting iff |b| <= 1.
    let mut z = w;
    let mut multiplier = (1.0, 0.0);
    for _ in 0..minimal_period {
        multiplier = (
            2.0 * (z.0 * multiplier.0 - z.1 * multiplier.1),
            2.0 * (z.0 * multiplier.1 + z.1 * multiplier.0),
        );
        z = iterate_complex(z, c);
    }
    let multiplier_norm =
        multiplier.0 * multiplier.0 + multiplier.1 * multiplier.1;
    (multiplier_norm <= 1.0).then_some(minimal_period)
}

#[inline]
// r[impl cz.craft.cached-products+1]
pub fn update_point_results<T:Sub<Output=T> + Add<Output=T> + Into<f64> + Gt + Mul<Output=T> + Copy>(point: &mut Point<T>) {
    // update values
    point.real_squared = point.z.0 * point.z.0;
    point.imag_squared = point.z.1 * point.z.1;
    point.real_imag = point.z.0 * point.z.1;
    let rad = point.real_squared + point.imag_squared;
    if rad.into() < point.smallness_squared.into() {point.smallness_squared =rad;point.small_time=point.iterations}

}



// r[impl cz.craft.cost-metadata+1]
pub fn queue_incomplete_neighbors<T:Sub<Output=T> + Add<Output=T> + Mul<Output=T> + Copy>(pos:&(i32, i32), res: (u32, u32), points: &Vec<Point<T>>, queue: &mut VecDeque<((i32, i32), u32)>) {

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
            if !points[index].delivered {
                queue.push_back((n, difficulty));
            }
        }
    }
}

pub fn queue_incomplete_neighbors_in<T:Sub<Output=T> + Add<Output=T> + Mul<Output=T> + Copy>(pos:&(i32, i32), res: (u32, u32), points: &Vec<Point<T>>, queue: &mut VecDeque<((i32, i32), u32)>) {

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
            if !points[index].delivered {
                queue.push_back((n, period));
            }
        }
    }
}

pub fn point_is_edge<T:Sub<Output=T> + Add<Output=T> + Mul<Output=T> + Copy> (pos:&(i32, i32), res: (u32, u32), points: &Vec<Point<T>>) -> Option<((i32, i32), (i32, i32))> {
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
            if (points[index].escapes || points[index].repeats)
                && (points[nindex].escapes || points[nindex].repeats)
            {
                if points[index].escapes != points[nindex].escapes || points[index].repeats != points[nindex].repeats {
                    return Some((*pos, n));
                } else if points[index].repeats == true {
                    // period 0 = verified-unknown; unknown must not light a filament
                    if points[index].period != 0 && points[nindex].period != 0
                        && points[index].period != points[nindex].period {
                        return Some((*pos, n));
                    }
                }
            }
        }
    }
    None
}

pub fn queue_incomplete_neighbors_of_edge<T:Sub<Output=T> + Add<Output=T> + Mul<Output=T> + Copy>(pos1:&(i32, i32), pos2:&(i32, i32), res: (u32, u32), points: &Vec<Point<T>>, queue: &mut VecDeque<((i32, i32), u32)>) {

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
            if !points[index].delivered {
                // r[impl cz.craft.edge-push-front+1]
                queue.push_front((n, difficulty));
            }
        }
    }
}