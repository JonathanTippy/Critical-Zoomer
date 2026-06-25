use steady_state::*;

use std::collections::*;
use crate::assemblies::headgroup::window::*;
use crate::assemblies::workgroup::screen_worker::workshift::*;
use crate::assemblies::headgroup::window::sampling::*;
use crate::assemblies::workgroup::screen_worker::*;
use crate::assemblies::structs::*;
use rand::prelude::SliceRandom;
use crate::utils::*; use crate::intexp::*;
use crate::constants::*;

pub enum WorkerCommand {
    Replace{frame_info: (ObjectivePosAndZoom, (u32, u32)), context: WorkContext}
}


pub struct WorkControllerState {
    mixmap: Vec<usize>
    , loc: (IntExp, IntExp)
    , zoom_pot: i64
    , worker_res: (u32, u32)
    , percent_completed: u16
    , last_sampler_location: Option<ObjectivePosAndZoom>
}


pub const WORKER_INIT_RES:(u32, u32) = DEFAULT_WINDOW_RES;
pub const WORKER_INIT_ZOOM_POT: i64 = -2;
pub const WORKER_INIT_ZOOM:f64 = if WORKER_INIT_ZOOM_POT>0 {(1<<WORKER_INIT_ZOOM_POT) as f64} else {1.0 / (1<<-WORKER_INIT_ZOOM_POT) as f64};

pub const PIXELS_PER_UNIT: u64 = 1<<(PIXELS_PER_UNIT_POT);

pub async fn run(
    actor: SteadyActorShadow,
    from_sampler: SteadyRx<(PointStencil)>,
    to_worker: SteadyTx<WorkerCommand>,
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

async fn internal_behavior<A: SteadyActor>(
    mut actor: A,
    from_sampler: SteadyRx<(PointStencil)>,
    to_worker: SteadyTx<WorkerCommand>,
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
                ,(
                    ObjectivePosAndZoom {
                        pos: (stuff.location.0.clone(), IntExp::ZERO-stuff.location.1.clone())
                        , zoom_pot: stuff.location.2
                    }
                    , (stuff.resolution.0 as u32
                    , stuff.resolution.1 as u32)
                )
            ) {
                actor.try_send(&mut to_worker, WorkerCommand::Replace{frame_info: (ObjectivePosAndZoom {
                    pos: (stuff.location.0, IntExp::ZERO-stuff.location.1)
                    ,
                    zoom_pot: stuff.location.2
                }, (stuff.resolution.0 as u32
                    , stuff.resolution.1 as u32)), context:ctx});
            };
        }
    }
    // Final shutdown log, reporting all statistics.
    info!("Computer shutting down.");
    Ok(())
}

use std::ops::*;
fn get_points(res: (u32, u32), loc:(IntExp, IntExp), zoom: i64) -> Vec<Point> {
    let mut out:Vec<Point> = Vec::with_capacity((res.0*res.1) as usize);

        let significant_res = PIXELS_PER_UNIT;

        let real_center:f64 = loc.0.to_f64();
        let imag_center:f64 = loc.1.to_f64();


        let zoom_factor:IntExp;

        if zoom > 0 {
            zoom_factor = IntExp::from(1) >> (zoom as u32);
        } else {
            zoom_factor = IntExp::from(1) << ((-zoom) as u32);
        }

        let zoom_factor_f64 = zoom_factor.to_f64();

        for row in 0..res.1 {
            for seat in 0..res.0 {

                let row = row as f32;
                let seat = seat as f32;

                let point:(f64, f64) = (
                    real_center + (seat / significant_res as f32) as f64 * zoom_factor_f64
                    , imag_center + (-(row / significant_res as f32)) as f64 * zoom_factor_f64
                );

                out.push(
                    Point{
                        c: point
                        , z: point
                        , real_squared: 0.0
                        , imag_squared: 0.0
                        , real_imag: 0.0
                        , iterations: 0
                        , loop_detection_point: ((0.0, 0.0), 0)
                        , escapes: false
                        , repeats: false
                        , delivered: false
                        , period: 0
                        , smallness_squared: 100.0
                        , small_time:0
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


fn handle_sampler_stuff(state: &mut WorkControllerState, stuff: (ObjectivePosAndZoom, (u32, u32))) -> Option<WorkContext> {

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
        obj.pos.0.clone()
        , obj.pos.1.clone()
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