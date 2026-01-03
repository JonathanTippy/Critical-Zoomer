use steady_state::*;

use std::collections::*;
use crate::actor::window::*;
use crate::action::workshift::*;
use crate::action::sampling::*;
use crate::actor::screen_worker::*;
use crate::action::constants::*;

use rand::prelude::SliceRandom;
use crate::action::utils::*;

pub(crate) enum WorkerCommand<T:Copy> {
    Replace{frame_info: (ObjectivePosAndZoom, (u32, u32)), context: WorkContext<T>}
}


pub(crate) struct WorkControllerState {
    mixmap: Vec<usize>
    , loc: (IntExp, IntExp)
    , zoom_pot: i64
    , worker_res: (u32, u32)
    , percent_completed: u16
    , last_sampler_location: Option<ObjectivePosAndZoom>
}


pub(crate) const WORKER_INIT_RES:(u32, u32) = DEFAULT_WINDOW_RES;
pub(crate) const WORKER_INIT_ZOOM_POT: i64 = -2;
pub(crate) const WORKER_INIT_ZOOM:f64 = if WORKER_INIT_ZOOM_POT>0 {(1<<WORKER_INIT_ZOOM_POT) as f64} else {1.0 / (1<<-WORKER_INIT_ZOOM_POT) as f64};

pub(crate) const PIXELS_PER_UNIT_POT:i32 = 9;
pub(crate) const PIXELS_PER_UNIT: u64 = 1<<(PIXELS_PER_UNIT_POT);

pub async fn run(
    actor: SteadyActorShadow,
    from_sampler: SteadyRx<(ObjectivePosAndZoom, (u32, u32))>,
    to_worker: SteadyTx<WorkerCommand<f64>>,
    state: SteadyState<WorkControllerState>,
) -> Result<(), Box<dyn Error>> {
    // The worker is tested by its simulated neighbors, so we always use internal_behavior.
    internal_behavior(
        actor.into_spotlight([&from_sampler], [&to_worker]),
        from_sampler,
        to_worker,
        state,
    )
        .await
}

async fn internal_behavior<A: SteadyActor, T:Clone + From<f32> + From<f32> + Clone + From<IntExp> + Sub<Output=T> + Add<Output=T> + Mul<Output=T> + PartialOrd + crate::action::workshift::Finite + crate::action::workshift::Gt + crate::action::workshift::Abs + From<f32> + Into<f64> + Copy>(
    mut actor: A,
    from_sampler: SteadyRx<(ObjectivePosAndZoom, (u32, u32))>,
    to_worker: SteadyTx<WorkerCommand<T>>,
    state: SteadyState<WorkControllerState>,
) -> Result<(), Box<dyn Error>> {

    let mut from_sampler = from_sampler.lock().await;
    let mut to_worker = to_worker.lock().await;

    let mut state = state.lock(|| WorkControllerState {
        mixmap: get_random_mixmap((WORKER_INIT_RES.0*WORKER_INIT_RES.1) as usize)
        , loc: (IntExp::from(0), IntExp::from(0))
        , zoom_pot: WORKER_INIT_ZOOM_POT
        , worker_res: WORKER_INIT_RES
        , percent_completed: 0
        , last_sampler_location: None
    }).await;


    let max_sleep = Duration::from_millis(50);

    let res = state.worker_res.clone();
    //let ctx = handle_sampler_stuff(&mut state, (
    //    (IntExp::from(0), IntExp::from(0)), res));
    //actor.try_send(&mut to_worker, WorkerCommand::Replace{context:ctx});

    while actor.is_running(
        || i!(to_worker.mark_closed())
    ) {

        await_for_any!(
            actor.wait_periodic(max_sleep),
            actor.wait_avail(&mut from_sampler, 1),
        );

        //info!("work controller alive");
        if actor.avail_units(&mut from_sampler) > 0 {
            while actor.avail_units(&mut from_sampler) > 1 {
                let stuff = actor.try_take(&mut from_sampler).expect("internal error");
                drop(stuff);
            };

            let stuff = actor.try_take(&mut from_sampler).expect("internal error");

            if let Some(ctx) = handle_sampler_stuff(
                &mut state
                , stuff.clone()
            ) {
                actor.try_send(&mut to_worker, WorkerCommand::Replace{frame_info: (stuff.0, stuff.1), context:ctx});
            };
        }
    }
    // Final shutdown log, reporting all statistics.
    info!("Computer shutting down.");
    Ok(())
}

use std::ops::*;
fn get_points<T: From<f32> + Clone + From<IntExp> + Sub<Output=T> + Add<Output=T> + Mul<Output=T> + PartialOrd + crate::action::workshift::Finite + crate::action::workshift::Gt + crate::action::workshift::Abs + From<f32> + Into<f64> + Copy>
    (res: (u32, u32), loc:(IntExp, IntExp), zoom: i64) -> Vec<Point<T>> {
    let mut out:Vec<Point<T>> = Vec::with_capacity((res.0*res.1) as usize);

        let significant_res = PIXELS_PER_UNIT;//min(res.0, res.1);

        let real_center:T = loc.0.into();
        let imag_center:T = loc.1.into();


        let zoom_factor:IntExp;

        if zoom > 0 {
            zoom_factor = IntExp::from(1) >> (zoom as u32);
        } else {
            zoom_factor = IntExp::from(1) << ((-zoom) as u32);
        }

        for row in 0..res.1 {
            for seat in 0..res.0 {

                let row = row as f32;
                let seat = seat as f32;

                let point:(T, T) = (
                    /*(real_center + ((seat as f32 / significant_res as f32 - 0.5) / zoom_factor) as f64) as f32
                    , (imag_center + (-((row as f32 / significant_res as f32 - 0.5) / zoom_factor)) as f64) as f32*/
                    real_center + (T::from((seat / significant_res as f32)) * zoom_factor.clone().into())
                    , imag_center + (T::from(-((row / significant_res as f32))) * zoom_factor.clone().into())
                );

                out.push(
                    Point{
                        c: point.clone()
                        , z: point.clone()
                        , real_squared: 0.0.into()
                        , imag_squared: 0.0.into()
                        , real_imag: 0.0.into()
                        , iterations: 0
                        , loop_detection_point: ((0.0.into(), 0.0.into()), 0)
                        , escapes: false
                        , repeats: false
                        , delivered: false
                        , period: 0
                        , smallness_squared: 100.0.into()
                        , small_time:0
                        , largeness_squared: 0.0.into()
                        , big_time: 0
                    }
                )
            }
        }
    out
}


fn get_random_mixmap(size: usize) -> Vec<usize> {
    let mut rng = rand::rng();

    let mut indices: Vec<usize> = (0..size).collect();

    // Shuffle indices randomly
    indices.shuffle(&mut rng);
    indices
}

fn get_interlaced_mixmap(res:(u32, u32), size:usize) -> Vec<usize> {
    let mut rng = rand::rng();

    let mut row_indices:Vec<usize> = (0..res.1 as usize).collect();
    row_indices.shuffle(&mut rng);

    let mut indices: Vec<usize> = (0..size).collect();
    for mut index in &mut indices {
        *index = *index % res.0 as usize
        +
        row_indices[(*index / res.0 as usize)]
        * res.0 as usize

    }
    indices
}


fn handle_sampler_stuff<T: Clone + From<f32> + From<f32> + Clone + From<IntExp> + Sub<Output=T> + Add<Output=T> + Mul<Output=T> + PartialOrd + crate::action::workshift::Finite + crate::action::workshift::Gt + crate::action::workshift::Abs + From<f32> + Into<f64> + Copy>(state: &mut WorkControllerState, stuff: (ObjectivePosAndZoom, (u32, u32))) -> Option<WorkContext<T>> {

    let zoomed = stuff.0.zoom_pot > state.zoom_pot as i32;

    let obj = stuff.0;

    if let Some(loc) = state.last_sampler_location.clone() {
        if !((obj != loc) || stuff.1 != state.worker_res) {
            return None
        }
    }

    if state.worker_res != stuff.1 {
        state.mixmap = get_random_mixmap((stuff.1.0*stuff.1.1) as usize)
    }

    state.worker_res = stuff.1;

    state.loc = (
        obj.pos.0.clone().into()
        , obj.pos.1.clone().into()
    );

    state.loc = (
        state.loc.0.clone()
        , IntExp::from(0)-state.loc.1.clone()
        );

    state.zoom_pot = obj.zoom_pot as i64;

    let mut edges = Vec::new();
    let res = state.worker_res;
    for i in 0..(res.0-1) as i32 {
        edges.push((i, 0))
    }
    for i in 0..(res.1-1) as i32 {
        edges.push(((res.0-1) as i32, i))
    }
    for i in 0..(res.0) as i32 {
        edges.push((i , (res.1-1) as i32))
    }
    for i in 1..(res.1-1) as i32 {
        edges.push((0, i))
    }

    let mut rng = rand::rng();
    // Shuffle edges randomly
    edges.shuffle(&mut rng);





    let work_context = WorkContext {
        points: get_points(stuff.1, state.loc.clone(), state.zoom_pot)
        , completed_points: Stec{stuff:[(CompletedPoint::Dummy{}, 0);100000], len:0}
        , index: 0
        , random_index: 0
        , time_created: Instant::now()
        , time_workshift_started: Instant::now()
        , percent_completed: 0.0
        , random_map: state.mixmap.clone()
        , workshifts: 0
        , total_iterations: 0
        , spent_tokens_today: 0
        , total_iterations_today: 0
        , total_points_today: 0
        , total_bouts_today: 0
        , last_update: 0
        , res: state.worker_res
        , scredge_poses: VecDeque::from(edges)
        , edge_queue: VecDeque::new()
        , out_queue: VecDeque::new()
        , in_queue: VecDeque::new()
        , zoomed
        , attention: (0, 0)
    };
    state.last_sampler_location = Some(obj);
    Some(work_context)
}