use steady_state::*;

use crate::actor::window::*;
use crate::action::workshift::*;
use crate::action::sampling::*;
use crate::actor::screen_worker::*;



use rand::prelude::SliceRandom;
use crate::action::utils::*;

pub(crate) enum WorkerCommand {
    Update
    , Replace{frame_info: ObjectivePosAndZoom, context: WorkContext}
}

pub(crate) enum ScreenValue {
    Outside{escape_time: u32}
    , Inside{loop_period: u32}
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
        || i!(to_worker.mark_closed())
    ) {
        state.percent_completed = (((state.completed_work.len() as f32) / ((state.worker_res.0*state.worker_res.1) as f32)) * u16::MAX as f32) as u16;

        await_for_any!(
            actor.wait_periodic(max_sleep),
            actor.wait_avail(&mut from_sampler, 1),
        );

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
                actor.try_send(&mut to_worker, WorkerCommand::Replace{frame_info: stuff.0, context:ctx});
            };
        }

        if state.percent_completed<u16::MAX {
            actor.try_send(&mut to_worker, WorkerCommand::Update{});
        }

    }
    // Final shutdown log, reporting all statistics.
    info!("Computer shutting down.");
    Ok(())
}

fn get_points_f32(res: (u32, u32), loc:(f64, f64), zoom: i64) -> Points {
    let mut out:Vec<PointF32> = Vec::with_capacity((res.0*res.1) as usize);

        for row in 0..res.1 {
            for seat in 0..res.0 {

                let significant_res = PIXELS_PER_UNIT;//min(res.0, res.1);

                let real_center:f64 = loc.0;
                let imag_center:f64 = loc.1;


                let zoom_factor:f32;

                if zoom > 0 {
                    zoom_factor = (1<<zoom) as f32;
                } else {
                    zoom_factor =  1.0 / ((1<<-zoom) as f32);
                }

                let point:(f32, f32) = (
                    /*(real_center + ((seat as f32 / significant_res as f32 - 0.5) / zoom_factor) as f64) as f32
                    , (imag_center + (-((row as f32 / significant_res as f32 - 0.5) / zoom_factor)) as f64) as f32*/
                    (real_center + ((seat as f32 / significant_res as f32) / zoom_factor) as f64) as f32
                    , (imag_center + (-((row as f32 / significant_res as f32) / zoom_factor)) as f64) as f32
                );

                out.push(
                    PointF32{
                        c: point
                        , z: point
                        , real_squared: 0.0
                        , imag_squared: 0.0
                        , real_imag: 0.0
                        , iterations: 0
                        , loop_detection_points: [(0.0, 0.0); NUMBER_OF_LOOP_CHECK_POINTS]
                        , done: (false, false)
                        , last_point: (0.0, 0.0)
                    }
                )
            }
        }
    Points::F32{p:out}
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


fn handle_sampler_stuff(state: &mut WorkControllerState, stuff: (ObjectivePosAndZoom, (u32, u32))) -> Option<WorkContext> {

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
        state.loc.0
        , -state.loc.1
        );

    state.zoom_pot = obj.zoom_pot as i64;


    let work_context = WorkContext {
        points: get_points_f32(stuff.1, state.loc, state.zoom_pot)
        , completed_points: vec!()
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
    };
    state.last_sampler_location = Some(obj);
    state.completed_work = vec!();
    Some(work_context)
}