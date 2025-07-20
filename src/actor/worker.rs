use steady_state::*;

use crate::actor::window::*;
use crate::actor::updater::*;
use crate::action::workday::*;
use crate::action::sampling::*;

use rand::Rng;

use std::cmp::*;

pub(crate) struct ZoomerScreenValues {
    pub(crate) values: Vec<(u32)>
    , pub(crate) relative_location_of_predecessor: (i32, i32)
    , pub(crate) relative_zoom_of_predecessor: i64
    , pub(crate) screen_size: (u32, u32)
    , pub(crate) zoom_factor_pot: i64
    , pub(crate) state_revision: u64
}

pub(crate) struct WorkerState {
    current_work_context: WorkContextF32
    , current_loc: (f64, f64)
    , current_zoom: i64
    , last_loc: (f64, f64)
    , last_zoom: i64
    , worker_token_budget: u32
    , iteration_token_cost: u32
    , point_token_cost: u32
    , bout_token_cost: u32
    , workday_token_cost: u32
}


pub(crate) const WORKER_INIT_RES_POT:u64 = 10;
pub(crate) const WORKER_INIT_RES:(u32, u32) = (1<<WORKER_INIT_RES_POT, 1<<WORKER_INIT_RES_POT);
pub(crate) const WORKER_INIT_LOC:(f64, f64) = (0.0, 0.0);
pub(crate) const WORKER_INIT_ZOOM_POT: i64 = 0;
pub(crate) const PIXELS_PER_UNIT: u64 = 270 * (1<<(WORKER_INIT_RES_POT-8));

pub async fn run(
    actor: SteadyActorShadow,
    updates_in: SteadyRx<SamplingContext>,
    values_out: SteadyTx<ZoomerScreenValues>,
    state: SteadyState<WorkerState>,
) -> Result<(), Box<dyn Error>> {
    // The worker is tested by its simulated neighbors, so we always use internal_behavior.
    internal_behavior(
        actor.into_spotlight([&updates_in], [&values_out]),
        updates_in,
        values_out,
        state,
    )
        .await
}

/// The core logic for the worker actor.
/// This function implements high-throughput, cache-friendly batch processing.
///
/// Key performance strategies:      //#!#//
/// - **Double-buffering**: The channel is logically split into two halves. While one half is being filled by the producer, the consumer processes the other half.
/// - **Full-channel consumption**: The worker processes both halves (two slices) before yielding, maximizing cache line reuse and minimizing context switches.
/// - **Pre-allocated buffers**: All batch buffers are allocated once and reused, ensuring zero-allocation hot paths.
/// - **Mechanically sympathetic**: The design aligns with CPU cache and memory bus behavior for optimal throughput.
async fn internal_behavior<A: SteadyActor>(
    mut actor: A,
    updates_in: SteadyRx<SamplingContext>,
    values_out: SteadyTx<ZoomerScreenValues>,
    state: SteadyState<WorkerState>,
) -> Result<(), Box<dyn Error>> {
    let mut updates_in = updates_in.lock().await;
    let mut values_out = values_out.lock().await;

    let mut state = state.lock(|| WorkerState {
        current_loc: WORKER_INIT_LOC
        , current_zoom: WORKER_INIT_ZOOM_POT
        , current_work_context: WorkContextF32 {
            points: get_points_f32((WORKER_INIT_RES.0, WORKER_INIT_RES.1), WORKER_INIT_LOC, WORKER_INIT_ZOOM_POT)
            , completed_points: vec!(CompletedPoint::Dummy{};(WORKER_INIT_RES.0 * WORKER_INIT_RES.1) as usize)
            , index: 0
            , random_index: 0
            , time_created: Instant::now()
            , time_workday_started: Instant::now()
            , percent_completed: 0.0
            , random_map: None
            , workdays: 0
            , total_iterations: 0
            , spent_tokens_today: 0
            , total_iterations_today: 0
            , total_points_today: 0
            , total_bouts_today: 0
        }
        , worker_token_budget: 50000000
        , iteration_token_cost: 2
        , bout_token_cost: 4
        , workday_token_cost: 0
        , point_token_cost: 150
        , last_loc: (WORKER_INIT_LOC.0, WORKER_INIT_LOC.1)
        , last_zoom: WORKER_INIT_ZOOM_POT
    }).await;

    let max_sleep = Duration::from_millis(1);

    let workday_duration = Duration::from_millis(50);

    while actor.is_running(
        || i!(values_out.mark_closed())
    ) {

        if state.current_work_context.percent_completed == 0.0 {
            //info!("calculating tokens");

            //calculate_tokens(&mut state);

            //info!("calculated tokens");
        }

        let working = state.current_work_context.percent_completed < 99.9999999;
        // this actor always pins its core if its doing work.
        if working {} else {
            await_for_any!(
                actor.wait_periodic(max_sleep),
                actor.wait_avail(&mut updates_in, 1),
            );
        }



        let mut sampling_contexts = vec!();
        while actor.avail_units(&mut updates_in) > 0 {
            sampling_contexts.push(actor.try_take(&mut updates_in).expect("internal error"));
        }
        if sampling_contexts.len() > 0 {
            let sampling_context = sampling_contexts.pop().unwrap();
            drop(sampling_contexts);
            handle_sampling_context(&mut state, sampling_context);
        }


        if working {

            let start = Instant::now();

            match workday(
                state.worker_token_budget - state.workday_token_cost - state.bout_token_cost
                , state.iteration_token_cost
                , state.bout_token_cost
                , state.point_token_cost
                ,  &mut state.current_work_context)
            {
                Some(c) => {
                    if state.current_work_context.percent_completed == 100.0 {
                        info!("context is done. Total time: {:.2}s\n total iterations: {}", state.current_work_context.time_created.elapsed().as_secs_f64(), state.current_work_context.total_iterations);
                    } else {
                        info!("workday completed. context is now {:.2}% done.", state.current_work_context.percent_completed);
                    }

                    actor.try_send(&mut values_out, ZoomerScreenValues{
                        values: strip_destination_f32(c)
                        , relative_location_of_predecessor: (
                            (((state.last_loc.0 - state.current_loc.0) / PIXELS_PER_UNIT as f64) * (1<<16) as f64) as i32
                            , (((state.last_loc.1 - state.current_loc.1) / PIXELS_PER_UNIT as f64) * (1<<16) as f64) as i32
                        )
                        , relative_zoom_of_predecessor: state.last_zoom - state.current_zoom
                        , zoom_factor_pot: WORKER_INIT_ZOOM_POT
                        , screen_size: WORKER_INIT_RES
                        , state_revision: 0
                    });


                }
                None => {
                    info!("workday completed. context is now {:.2}% done.", state.current_work_context.percent_completed);
                }
            }

            let workday_length = start.elapsed();
            info!("workday took {:.2} ms", workday_length.as_secs_f64() * 1000.0);

            //let iteration_length = workday_length/state.worker_iteration_budget as f64;

            //state.worker_iteration_budget = (0.05 / iteration_length) as u32;
        }
    }
    // Final shutdown log, reporting all statistics.
    info!("Computer shutting down.");
    Ok(())
}

fn get_points_f32(res: (u32, u32), loc:(f64, f64), zoom: i64) -> Vec<PointF32> {
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
                    (real_center + ((seat as f32 / significant_res as f32 - 0.5) * 4.0 / zoom_factor) as f64) as f32
                    , (imag_center + (-((row as f32 / significant_res as f32 - 0.5) * 4.0 / zoom_factor)) as f64) as f32
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

    out
}

fn strip_destination_f32(pts: Vec<CompletedPoint>) -> Vec<u32> {
    let mut out:Vec<u32> = Vec::with_capacity(pts.len());
    for i in 0..pts.len() {
        match &pts[i] {
            CompletedPoint::Repeats{} => {
                out.push(u32::MAX);
            },
            CompletedPoint::Escapes{escape_time, escape_location} => {
                out.push(*escape_time);
            }
            CompletedPoint::Dummy{} => {
                panic!("completed point was a dummy");
            }
        }
    }
    out
}


fn calculate_tokens(state: &mut WorkerState) {

}

fn handle_sampling_context(state: &mut WorkerState, sampling_context: SamplingContext) {
    let mut relative_translation: (f64, f64) = (
        sampling_context.relative_pos.0 as f64 / PIXELS_PER_UNIT as f64
        , sampling_context.relative_pos.1 as f64 / PIXELS_PER_UNIT as f64
    );
    let mut relative_zoom_pot: i8 = sampling_context.relative_zoom_pot;

    if !(relative_translation==(0.0,0.0) && relative_zoom_pot==0) {

        let objective_zoom_pot = state.last_zoom + relative_zoom_pot as i64;

        let objective_translation;
        if state.last_zoom > 0 {
            objective_translation = (
                relative_translation.0 * (1 >> state.last_zoom) as f64
                , relative_translation.1 * (1 >> state.last_zoom) as f64
            );
        } else {
            objective_translation = (
                relative_translation.0 * (1 << -state.last_zoom) as f64
                , relative_translation.1 * (1 << -state.last_zoom) as f64
            );
        }


        let new_loc = (
            state.last_loc.0 - objective_translation.0
            , state.last_loc.1 + objective_translation.1
            );


        state.current_work_context = WorkContextF32 {
            points: get_points_f32((WORKER_INIT_RES.0, WORKER_INIT_RES.1), new_loc, objective_zoom_pot)
            , completed_points: vec!(CompletedPoint::Dummy{};(WORKER_INIT_RES.0 * WORKER_INIT_RES.1) as usize)
            , index: 0
            , random_index: 0
            , time_created: Instant::now()
            , time_workday_started: Instant::now()
            , percent_completed: 0.0
            , random_map: None
            , workdays: 0
            , total_iterations: 0
            , spent_tokens_today: 0
            , total_iterations_today: 0
            , total_points_today: 0
            , total_bouts_today: 0
        };

        state.last_loc = new_loc;
        state.last_zoom = objective_zoom_pot;
    }
}