use steady_state::*;

use crate::actor::window::*;
use crate::action::workshift::*;
use crate::action::sampling::*;
use crate::actor::screen_worker::*;



use rand::prelude::SliceRandom;
use crate::action::utils::*;

pub(crate) enum WorkerCommand {
    Update
    , Replace{context: WorkContext}
}

pub(crate) enum ScreenValue {
    Outside{escape_time: u32}
    , Inside{loop_period: u32}
}

pub(crate) struct ResultsPackage {
    pub(crate) results: Vec<ScreenValue>
    , pub(crate) screen_res: (u32, u32)
    , pub(crate) location: ObjectivePosAndZoom
    , pub(crate) complete: bool
}

pub(crate) struct WorkControllerState {
    completed_work_layers: Vec<Vec<Option<CompletedPoint>>>
    , completed_work: Vec<CompletedPoint>
    // this vecvec contains the completed work layer by layer, or resolution by resolution.
    // for example, vec 0 contains the 4 points for res 1x1, 1 contains the additional 5 points to make res 2x2
    // vec 2 contains the additional 33 points to make res 4x4 (assuming a square POT screen)
    // this achieves dynamic res at arbitrary positions; when producing ARVs, the smallest possible square is
    // used for each ARV.
    , mixmaps: Vec<Vec<usize>>
    , mixmap: Vec<usize>
    // each res has its own custom sized mixmap
    // this mixmap is used by the workers to determine point order
    // work can also be done in whatever order; for example, for attention.
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
    from_worker: SteadyRx<WorkUpdate>,
    values_out: SteadyTx<ResultsPackage>,
    state: SteadyState<WorkControllerState>,
) -> Result<(), Box<dyn Error>> {
    // The worker is tested by its simulated neighbors, so we always use internal_behavior.
    internal_behavior(
        actor.into_spotlight([&from_worker], [&values_out]),
        from_worker,
        values_out,
        state,
    )
        .await
}

async fn internal_behavior<A: SteadyActor>(
    mut actor: A,
    from_worker: SteadyRx<WorkUpdate>,
    values_out: SteadyTx<ResultsPackage>,
    state: SteadyState<WorkControllerState>,
) -> Result<(), Box<dyn Error>> {

    let mut values_out = values_out.lock().await;
    let mut from_worker = from_worker.lock().await;

    let mut state = state.lock(|| WorkControllerState {
        completed_work_layers: vec!()
        , completed_work: vec!()
        , mixmaps: vec!()//get_mixmaps(WORKER_INIT_RES)
        , mixmap: get_random_mixmap((WORKER_INIT_RES.0*WORKER_INIT_RES.1) as usize)
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
        || i!(values_out.mark_closed())
    ) {
        state.percent_completed = (((state.completed_work.len() as f32) / ((state.worker_res.0*state.worker_res.1) as f32)) * u16::MAX as f32) as u16;

        if actor.avail_units(&mut from_worker) > 0 {
            let mut u = actor.try_take(&mut from_worker).unwrap();

            if let Some(f) = u.frame_info {
                state.completed_work = vec!();
                state.last_sampler_location = Some(f);
            }

            state.completed_work.append(&mut u.completed_points);
            let res = state.worker_res;
            let c = state.percent_completed==u16::MAX;
            let r = determine_arvs_dummy(&state.completed_work, res);
            info!("got work update. results length is now {}", r.len());
            if r.len() == (res.0*res.1) as usize {
                actor.try_send(&mut values_out, ResultsPackage{
                    results:r
                    ,screen_res:res
                    ,location: state.last_sampler_location.clone().expect("WC recieved work from worker but somehow don't have a sampler location")
                    ,complete:c
                });
            }
        }

    }
    // Final shutdown log, reporting all statistics.
    info!("Computer shutting down.");
    Ok(())
}



fn determine_arvs_dummy(points: &Vec<CompletedPoint>, res: (u32, u32)) -> Vec<ScreenValue> {
    let mut returned = vec!();
    for p in points {
        returned.push(
            match p {
                CompletedPoint::Escapes{escape_time: t, escape_location: _} => {
                    ScreenValue::Outside{escape_time:*t}
                }
                CompletedPoint::Repeats{period: p} => {
                    ScreenValue::Inside{loop_period:*p}
                }
                CompletedPoint::Dummy{} => {
                    ScreenValue::Outside{escape_time:2}
                }
            }
        )
    }

    returned
}

fn get_random_mixmap(size: usize) -> Vec<usize> {
    let mut rng = rand::rng();

    let mut indices: Vec<usize> = (0..size).collect();

    // Shuffle indices randomly
    indices.shuffle(&mut rng);
    indices
}