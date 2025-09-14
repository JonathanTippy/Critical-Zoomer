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

pub(crate) enum AreaRepresentativeValue {
    Outside{escape_time: u32}
    , Inside{loop_period: u32}
    , Edge{}
}

pub(crate) struct ResultsPackage {
    pub(crate) results: Vec<AreaRepresentativeValue>
    , pub(crate) screen_res: (u32, u32)
    , pub(crate) originating_relative_transforms: SamplingRelativeTransforms
    , pub(crate) dummy: bool
    , pub(crate) complete: bool
}

pub(crate) struct WorkControllerState {
    completed_work_layers: Vec<Vec<Option<CompletedPoint>>>
    // this vecvec contains the completed work layer by layer, or resolution by resolution.
    // for example, vec 0 contains the 4 points for res 1x1, 1 contains the additional 5 points to make res 2x2
    // vec 2 contains the additional 33 points to make res 4x4 (assuming a square POT screen)
    // this achieves dynamic res at arbitrary positions; when producing ARVs, the smallest possible square is
    // used for each ARV.
    // each res has its own custom sized mixmap
    // this mixmap is used by the workers to determine point order
    // work can also be done in whatever order; for example, for attention.
    , loc: (f64, f64)
    , zoom_pot: i64
    , worker_res: (u32, u32)
    , percent_completed: u16
    , last_relative_transforms: SamplingRelativeTransforms
}


pub(crate) const WORKER_INIT_RES:(u32, u32) = DEFAULT_WINDOW_RES;
pub(crate) const WORKER_INIT_LOC:(f64, f64) = (0.0, 0.0);
pub(crate) const WORKER_INIT_ZOOM_POT: i64 = -2;
pub(crate) const WORKER_INIT_ZOOM:f64 = if WORKER_INIT_ZOOM_POT>0 {(1<<WORKER_INIT_ZOOM_POT) as f64} else {1.0 / (1<<-WORKER_INIT_ZOOM_POT) as f64};
pub(crate) const PIXELS_PER_UNIT: u64 = 1<<(9);

pub async fn run(
    actor: SteadyActorShadow,
    from_sampler: SteadyRx<(SamplingRelativeTransforms, (u32, u32))>,
    from_worker: SteadyRx<WorkUpdate>,
    values_out: SteadyTx<ResultsPackage>,
    to_worker: SteadyTx<WorkerCommand>,
    state: SteadyState<WorkControllerState>,
) -> Result<(), Box<dyn Error>> {
    // The worker is tested by its simulated neighbors, so we always use internal_behavior.
    internal_behavior(
        actor.into_spotlight([&from_sampler, &from_worker], [&values_out, &to_worker]),
        from_sampler,
        from_worker,
        values_out,
        to_worker,
        state,
    )
        .await
}

async fn internal_behavior<A: SteadyActor>(
    mut actor: A,
    from_sampler: SteadyRx<(SamplingRelativeTransforms, (u32, u32))>,
    from_worker: SteadyRx<WorkUpdate>,
    values_out: SteadyTx<ResultsPackage>,
    to_worker: SteadyTx<WorkerCommand>,
    state: SteadyState<WorkControllerState>,
) -> Result<(), Box<dyn Error>> {

    let mut from_sampler = from_sampler.lock().await;
    let mut values_out = values_out.lock().await;
    let mut from_worker = from_worker.lock().await;
    let mut to_worker = to_worker.lock().await;

    let mut state = state.lock(|| WorkControllerState {
        completed_work_layers: vec!()
        , completed_work: vec!()
        , loc: WORKER_INIT_LOC
        , zoom_pot: WORKER_INIT_ZOOM_POT
        , worker_res: WORKER_INIT_RES
        , percent_completed: 0
        , last_relative_transforms: SamplingRelativeTransforms{pos: (0, 0), zoom_pot: 0, counter: 0}
    }).await;


    let max_sleep = Duration::from_millis(50);

    let res = state.worker_res.clone();
    let ctx = handle_home(&mut state, res);
    actor.try_send(&mut to_worker, WorkerCommand::Replace{context:ctx});

    while actor.is_running(
        || i!(values_out.mark_closed())
    ) {
        state.percent_completed = (((state.completed_work.len() as f32) / ((state.worker_res.0*state.worker_res.1) as f32)) * u16::MAX as f32) as u16;

        await_for_any!(
            actor.wait_periodic(max_sleep),
            actor.wait_avail(&mut from_sampler, 1),
        );
        while actor.avail_units(&mut from_worker) > 0 {
            let mut u = actor.try_take(&mut from_worker).unwrap();
            state.completed_work.append(&mut u.completed_points);
            let res = state.worker_res;
            let t = state.last_relative_transforms.clone();
            let c = state.percent_completed==u16::MAX;
            let r = determine_arvs_dummy(&state.completed_work, res);
            info!("got work update. results length is now {}", r.len());
            if r.len() == (res.0*res.1) as usize {
                actor.try_send(&mut values_out, ResultsPackage{results:r,screen_res:res,originating_relative_transforms:t,dummy:false,complete:c});
            }
        }

        
        if actor.avail_units(&mut from_sampler) > 0 {
            while actor.avail_units(&mut from_sampler) > 1 {
                let stuff = actor.try_take(&mut from_sampler).expect("internal error");
                if stuff.0 == (SamplingRelativeTransforms{ pos:(0,0), zoom_pot:i64::MIN, counter:u64::MAX }) {
                    let ctx = handle_home(&mut state, stuff.1);
                    actor.try_send(&mut to_worker, WorkerCommand::Replace{context:ctx});
                }
                drop(stuff);
            };

            let stuff = actor.try_take(&mut from_sampler).expect("internal error");

            if stuff.0 == (SamplingRelativeTransforms{ pos:(0,0), zoom_pot:i64::MIN, counter:u64::MAX }) {
                handle_home(&mut state, stuff.1);
            } else if stuff.0.counter > state.last_relative_transforms.counter {
                if let Some(ctx) = handle_sampler_stuff(
                    &mut state
                    , stuff
                ) {
                    actor.try_send(&mut to_worker, WorkerCommand::Replace{context:ctx});
                };
            }
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
                    (real_center + ((seat as f32 / significant_res as f32 - 0.5) / zoom_factor) as f64) as f32
                    , (imag_center + (-((row as f32 / significant_res as f32 - 0.5) / zoom_factor)) as f64) as f32
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

fn determine_arvs_dummy(points: &Vec<CompletedPoint>, res: (u32, u32)) -> Vec<AreaRepresentativeValue> {
    let mut returned = vec!();
    for p in points {
        returned.push(
            match p {
                CompletedPoint::Escapes{escape_time: t, escape_location: _} => {
                    AreaRepresentativeValue::Outside{escape_time:*t}
                }
                CompletedPoint::Repeats{period: p} => {
                    AreaRepresentativeValue::Inside{loop_period:*p}
                }
                CompletedPoint::Dummy{} => {
                    AreaRepresentativeValue::Inside{loop_period:0}
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


fn handle_home(state: &mut WorkControllerState, size: (u32, u32)) -> WorkContext {
    *state = WorkControllerState {
        completed_work_layers: vec!()
        , completed_work: vec!()
        , percent_completed: 0
        , last_relative_transforms: SamplingRelativeTransforms{pos: (0, 0), zoom_pot: 0, counter: 1}
        , loc: WORKER_INIT_LOC
        , zoom_pot: WORKER_INIT_ZOOM_POT
        , worker_res: WORKER_INIT_RES
    };

    let work_context = WorkContext {
        points: get_points_f32(size, WORKER_INIT_LOC, WORKER_INIT_ZOOM_POT)
        , completed_points: vec!(CompletedPoint::Dummy{};(size.0 * size.1) as usize)
        , index: 0
        , random_index: 0
        , time_created: Instant::now()
        , time_workshift_started: Instant::now()
        , percent_completed: 0.0
        , workshifts: 0
        , total_iterations: 0
        , spent_tokens_today: 0
        , total_iterations_today: 0
        , total_points_today: 0
        , total_bouts_today: 0
        , last_update: 0
    };
    return work_context;
}

fn handle_sampler_stuff(state: &mut WorkControllerState, stuff: (SamplingRelativeTransforms, (u32, u32))) -> Option<WorkContext> {

    let transforms = stuff.0;


    if (transforms.pos != (0, 0)) || (transforms.zoom_pot != 0) || stuff.1 != state.worker_res {

        state.worker_res = stuff.1;
        //info!("changing zoom from {} to {} based on counter number {}", state.zoom_pot, state.zoom_pot + transforms.zoom_pot, transforms.counter);

        let objective_zoom = zoom_from_pot(state.zoom_pot + transforms.zoom_pot);

        let objective_translation = (
            -(transforms.pos.0 as f64 / PIXELS_PER_UNIT as f64 / objective_zoom)
            , transforms.pos.1 as f64 / PIXELS_PER_UNIT as f64 / objective_zoom
        );

        let zoomed = transforms.zoom_pot != 0;

        let zoom = zoom_from_pot(transforms.zoom_pot);

        state.loc = (
            state.loc.0 + objective_translation.0
            , state.loc.1 + objective_translation.1
        );

        if transforms.zoom_pot > 0 {
            for z in 0..transforms.zoom_pot {
                state.loc = (
                    state.loc.0 - (2.0/(zoom_from_pot(state.zoom_pot + z+1)/WORKER_INIT_ZOOM))
                    , state.loc.1 + (2.0/(zoom_from_pot(state.zoom_pot + z+1)/WORKER_INIT_ZOOM))
                )
            }

        } else if transforms.zoom_pot < 0 {
            for z in 0..-transforms.zoom_pot {
                state.loc = (
                    state.loc.0 + (1.0/(zoom_from_pot(state.zoom_pot - (z+1))/WORKER_INIT_ZOOM))
                    , state.loc.1 - (1.0/(zoom_from_pot(state.zoom_pot - (z+1))/WORKER_INIT_ZOOM))
                )
            }
        }

        state.zoom_pot = state.zoom_pot + transforms.zoom_pot;


        let work_context = WorkContext {
            points: get_points_f32(stuff.1, state.loc, state.zoom_pot)
            , completed_points: vec!(CompletedPoint::Dummy{};(stuff.1.0 * stuff.1.1) as usize)
            , index: 0
            , random_index: 0
            , time_created: Instant::now()
            , time_workshift_started: Instant::now()
            , percent_completed: 0.0
            , workshifts: 0
            , total_iterations: 0
            , spent_tokens_today: 0
            , total_iterations_today: 0
            , total_points_today: 0
            , total_bouts_today: 0
            , last_update: 0
        };
        state.last_relative_transforms = transforms;
        state.completed_work = vec!();
        return Some(work_context);
    } else {
        state.last_relative_transforms = transforms;
        return None;
    }
}