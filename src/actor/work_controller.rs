use steady_state::*;

use std::collections::*;
use crate::actor::window::*;
use crate::action::workshift::*;
use crate::action::sampling::*;
use crate::actor::screen_worker::*;



use rand::prelude::SliceRandom;
use crate::action::utils::*;

pub(crate) enum WorkerCommand {
    Replace{frame_info: (ObjectivePosAndZoom, (u32, u32))}
}


pub(crate) struct WorkControllerState {
    mixmap: Vec<usize>
    , loc: (f64, f64)
    , zoom_pot: i64
    , worker_res: (u32, u32)
    , percent_completed: u16
    , last_sampler_location: Option<ObjectivePosAndZoom>
}


pub(crate) const WORKER_INIT_RES:(u32, u32) = DEFAULT_WINDOW_RES;
pub(crate) const WORKER_INIT_LOC:(f64, f64) = (0.0, 0.0);
pub(crate) const WORKER_INIT_ZOOM_POT: i64 = -2;
pub(crate) const WORKER_INIT_ZOOM:f64 = if WORKER_INIT_ZOOM_POT>0 {(1<<WORKER_INIT_ZOOM_POT) as f64} else {1.0 / (1<<-WORKER_INIT_ZOOM_POT) as f64};

pub(crate) const PIXELS_PER_UNIT_POT:i32 = 9;
pub(crate) const PIXELS_PER_UNIT: u64 = 1<<(PIXELS_PER_UNIT_POT);

pub async fn run(
    actor: SteadyActorShadow,
    from_sampler: SteadyRx<(ObjectivePosAndZoom, (u32, u32))>,
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
    from_sampler: SteadyRx<(ObjectivePosAndZoom, (u32, u32))>,
    to_worker: SteadyTx<WorkerCommand>,
    state: SteadyState<WorkControllerState>,
) -> Result<(), Box<dyn Error>> {

    let mut from_sampler = from_sampler.lock().await;
    let mut to_worker = to_worker.lock().await;

    let mut state = state.lock(|| WorkControllerState {
        mixmap: get_random_mixmap((WORKER_INIT_RES.0*WORKER_INIT_RES.1) as usize)
        , loc: WORKER_INIT_LOC
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

            if handle_sampler_stuff(
                &mut state
                , stuff.clone()
            ) {
                actor.try_send(&mut to_worker, WorkerCommand::Replace{frame_info: (stuff.0, stuff.1)});
            };
        }
    }
    // Final shutdown log, reporting all statistics.
    info!("Computer shutting down.");
    Ok(())
}







fn handle_sampler_stuff(state: &mut WorkControllerState, stuff: (ObjectivePosAndZoom, (u32, u32))) -> bool {

    let obj = stuff.0;

    if let Some(loc) = state.last_sampler_location.clone() {
        if !((obj != loc) || stuff.1 != state.worker_res) {
            return false
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
        state.loc.0
        , -state.loc.1
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

    
    state.last_sampler_location = Some(obj);
    true
}