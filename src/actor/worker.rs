use steady_state::*;

use crate::actor::window::*;
use crate::actor::updater::*;
use crate::action::workday::*;
use crate::action::sampling::*;

use rand::Rng;

use std::cmp::*;
use crate::action::utils::relative_zoom_from_pot;

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
    , last_relative_loc: (i32, i32)
    , current_relative_loc: (i32, i32)
    , state_revision_counter: u64
    , last_relative_zoom_pot: i8
   // , state_revised: bool
}


pub(crate) const WORKER_INIT_RES_POT:u64 = 9;
pub(crate) const WORKER_INIT_RES:(u32, u32) = (1<<WORKER_INIT_RES_POT, 1<<WORKER_INIT_RES_POT);
pub(crate) const WORKER_INIT_LOC:(f64, f64) = (0.0, 0.0);
pub(crate) const WORKER_INIT_ZOOM_POT: i64 = -2;
pub(crate) const WORKER_INIT_ZOOM:f64 = if WORKER_INIT_ZOOM_POT>0 {(1<<WORKER_INIT_ZOOM_POT) as f64} else {1.0 / (1<<-WORKER_INIT_ZOOM_POT) as f64};
pub(crate) const PIXELS_PER_UNIT: u64 = 1<<(WORKER_INIT_RES_POT);

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
        , worker_token_budget: 40000000
        , iteration_token_cost: 2
        , bout_token_cost: 4
        , workday_token_cost: 0
        , point_token_cost: 150
        , last_loc: (WORKER_INIT_LOC.0, WORKER_INIT_LOC.1)
        , last_zoom: WORKER_INIT_ZOOM_POT
        , last_relative_loc: (0, 0)
        , current_relative_loc: (0, 0)
        , state_revision_counter: 0
        , last_relative_zoom_pot: 0
        //, state_revised: false
    }).await;

    let max_sleep = Duration::from_millis(1);

    let workday_duration = Duration::from_millis(50);

    while actor.is_running(
        || i!(values_out.mark_closed())
    ) {

        let working = state.current_work_context.percent_completed < 99.9999999;
        // this actor always pins its core if its doing work.
        if working {} else {
            await_for_any!(
                actor.wait_periodic(max_sleep),
                actor.wait_avail(&mut updates_in, 1),
            );
        }

        for _ in 1..actor.avail_units(&mut updates_in) {
            drop(actor.try_take(&mut updates_in).expect("internal error"));
        }

        if actor.avail_units(&mut updates_in) > 0 {

            let context = actor.try_take(&mut updates_in).expect("internal error");
            handle_sampling_context(
                &mut state
                , context
            );
        }

        if working {

            let start = Instant::now();

            match workday (
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
                        , relative_location_of_predecessor: state.last_relative_loc
                        , relative_zoom_of_predecessor: state.last_relative_zoom_pot as i64
                        , zoom_factor_pot: WORKER_INIT_ZOOM_POT
                        , screen_size: WORKER_INIT_RES
                        , state_revision: state.state_revision_counter
                    });

                    //state.state_revised = false;
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

    if sampling_context.screens[0].state_revision == state.state_revision_counter {

        if sampling_context.relative_pos != (0, 0) || sampling_context.relative_zoom_pot != 0 {

            info!("relative pos: {}, {}", sampling_context.relative_pos.0, sampling_context.relative_pos.1);

            let objective_zoom_pot = state.current_zoom + sampling_context.relative_zoom_pot as i64;

            let zoom = relative_zoom_from_pot(state.current_zoom as i8);

            let objective_zoom = relative_zoom_from_pot(objective_zoom_pot as i8);

            let relative_zoom = relative_zoom_from_pot(sampling_context.relative_zoom_pot);

            let objective_translation = (
                -(sampling_context.relative_pos.0 as f64 / PIXELS_PER_UNIT as f64 / objective_zoom)
                , sampling_context.relative_pos.1 as f64 / PIXELS_PER_UNIT as f64 / objective_zoom
            );

            let zoomed = sampling_context.relative_zoom_pot != 0;

            let new_loc = if !zoomed {(
                state.current_loc.0 + objective_translation.0
                , state.current_loc.1 + objective_translation.1
            )} else if sampling_context.relative_zoom_pot > 0 {(
                state.current_loc.0 + objective_translation.0 - (2.0/(objective_zoom/WORKER_INIT_ZOOM))
                , state.current_loc.1 + objective_translation.1 + (2.0/(objective_zoom/WORKER_INIT_ZOOM))
            )} else {(
                state.current_loc.0 + objective_translation.0 + (1.0/(objective_zoom/WORKER_INIT_ZOOM))
                , state.current_loc.1 + objective_translation.1 - (1.0/(objective_zoom/WORKER_INIT_ZOOM))
            )};

            state.last_relative_loc = (
                sampling_context.relative_pos.0
                , sampling_context.relative_pos.1
            );

            state.last_relative_zoom_pot = sampling_context.relative_zoom_pot;




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


            state.current_loc = new_loc;
            state.current_zoom = objective_zoom_pot;
            state.state_revision_counter = state.state_revision_counter + 1;

        } else {
            state.last_relative_loc = (0, 0);
            state.last_relative_zoom_pot = 0;
            state.state_revision_counter = state.state_revision_counter + 1;
        }
    }
}


#[cfg(test)]
pub(crate) mod worker_tests {

    use steady_state::*;
    use super::*;

    #[test]
    fn test_worker() -> Result<(), Box<dyn Error>> {
        let mut graph = GraphBuilder::for_testing().build(());
        let (values_tx, values_rx) = graph.channel_builder().build();
        let (state_tx, state_rx) = graph.channel_builder().build();
        let state = new_state();

        graph.actor_builder().with_name("UnitTest")
            .build(move |context| internal_behavior(
                context
                , state_rx.clone()
                , values_tx.clone()
                , state.clone()
            ), SoloAct);

        state_tx.testing_send_all(vec![], true);
        graph.start();
        // because shutdown waits for closed and empty, it does not happen until our test data is digested.
        graph.request_shutdown();
        graph.block_until_stopped(Duration::from_secs(1))?;
        assert_steady_rx_eq_take!(&values_rx, []);
        Ok(())
    }
}