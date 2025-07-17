use steady_state::*;

use crate::actor::window::*;
use crate::actor::updater::*;
use crate::action::workday::*;
use crate::actor::colorer::ZoomerScreenValues;

use rand::Rng;

use std::cmp::*;

pub(crate) struct WorkerState {
    current_work_context: WorkContextF32
    , worker_token_budget: u32
    , iteration_token_cost: u32
    , point_token_cost: u32
    , bout_token_cost: u32
    , workday_token_cost: u32
}

pub(crate) const WORKER_INIT_RES:(u32, u32) = (500, 500);
pub(crate) const WORKER_INIT_LOC:(&'static str, &'static str) = ("0", "0");
pub(crate) const WORKER_INIT_ZOOM:&'static str = "1";

pub async fn run(
    actor: SteadyActorShadow,
    updates_in: SteadyRx<ZoomerUpdate>,
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
    updates_in: SteadyRx<ZoomerUpdate>,
    values_out: SteadyTx<ZoomerScreenValues>,
    state: SteadyState<WorkerState>,
) -> Result<(), Box<dyn Error>> {
    let mut updates_in = updates_in.lock().await;
    let mut values_out = values_out.lock().await;


    let mut state = state.lock(|| WorkerState {
        current_work_context: WorkContextF32 {
            points: get_points_f32((WORKER_INIT_RES.0, WORKER_INIT_RES.1), WORKER_INIT_LOC, WORKER_INIT_ZOOM)
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
    }).await;

    let max_sleep = Duration::from_millis(100);

    let workday_duration = Duration::from_millis(50);

    while actor.is_running(
        || i!(values_out.mark_closed())
    ) {

        if state.current_work_context.percent_completed == 0.0 {
            //info!("calculating tokens");

            //calculate_tokens(&mut state);

            //info!("calculated tokens");
        }

        let working = state.current_work_context.percent_completed < 99.99;

        if working {} else {
            await_for_any!(
                actor.wait_periodic(max_sleep),
                actor.wait_avail(&mut updates_in, 1),
            );
        }

        // this actor always pins its core if its doing work.

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
                        , location: (WORKER_INIT_LOC.0.to_string(), WORKER_INIT_LOC.1.to_string())
                        , zoom_factor: WORKER_INIT_ZOOM.to_string()
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

fn get_points_f32(res: (u32, u32), loc:(&str, &str), zoom: &str) -> Vec<PointF32> {
    let mut out:Vec<PointF32> = Vec::with_capacity((res.0*res.1) as usize);

        for row in 0..res.1 {
            for seat in 0..res.0 {

                let significant_res = min(res.0, res.1);

                let real_center:f32 = loc.0.parse().unwrap();
                let imag_center:f32 = loc.1.parse().unwrap();

                let zoom:f32 = zoom.parse().unwrap();


                let point:(f32, f32) = (
                    (seat as f32 / significant_res as f32 - 0.5) * 4.0 / zoom
                    , -((row as f32 / significant_res as f32 - 0.5) * 4.0 / zoom)
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

    //run one no-bout workday

    let start = Instant::now();
    workday(100, 1, 100, 100,  &mut state.current_work_context);
    let time = start.elapsed().as_nanos();

    let workday_time = time;

    info!("workday time: {:.2}ms", workday_time as u64 / 1000000);

    //collect two one-bout no-point >=10 iteration workdays

    info!("collecting first two results");

    let mut results = vec!();

    let mut poison = 0;
    let mut key = (0, 0);

   // let mut found = false;
    //while !found {
        let start = Instant::now();
        workday(4000000, 1, 1000, 1000,  &mut state.current_work_context);
        let time = start.elapsed().as_nanos();
        //if state.current_work_context.total_iterations_today != state.current_work_context.total_
        poison = state.current_work_context.total_iterations_today;
        key = (state.current_work_context.total_bouts_today, state.current_work_context.total_points_today);
        results.push( (state.current_work_context.total_iterations_today, time) );
   // }


    info!("collected first result");


    let mut found = false;
    while !found {
        let start = Instant::now();
        workday(4000000, 1, 1000, 1000,  &mut state.current_work_context);
        let time = start.elapsed().as_nanos();
        if state.current_work_context.total_points_today == key.1
            && state.current_work_context.total_bouts_today == key.0
            && state.current_work_context.total_iterations_today != poison {

            found = true;
            results.push( (state.current_work_context.total_iterations_today, time) );
        }
    }

    info!("collected second result");



    info!("collected first two results");

    // calculate from that the bout time and iteration time

    let iteration_time = ((results[0].1 - results[1].1) as i64 / (results[0].0 as i32 - results[1].0 as i32 )as i64) as u128;

    info!("iteration time: {:.2}ms", iteration_time as u64 / 1000000);




    info!("collecting second two results");

    let mut results = vec!();

    let mut poison = 0;
    let mut key = 0;

    let mut found = false;
    while !found {
        let start = Instant::now();
        workday(4000000, 1, 1000, 1000,  &mut state.current_work_context);
        let time = start.elapsed().as_nanos();
        if state.current_work_context.total_points_today != state.current_work_context.total_bouts_today {
            poison = state.current_work_context.total_bouts_today;
            key = state.current_work_context.total_points_today;
            results.push( (
                state.current_work_context.total_bouts_today
                , time - workday_time - state.current_work_context.total_iterations_today as u128 * iteration_time)
            );
            found = true;
        }
    }

    info!("collected first result");


    let mut found = false;
    while !found {
        let start = Instant::now();
        workday(4000000, 1, 1000, 1000,  &mut state.current_work_context);
        let time = start.elapsed().as_nanos();
        if state.current_work_context.total_points_today == key
            && state.current_work_context.total_bouts_today != poison {

            found = true;
            results.push( (state.current_work_context.total_bouts_today, time - workday_time - state.current_work_context.total_iterations_today as u128 * iteration_time) );
        }
    }

    info!("collected second result");



    info!("collected second two results");

    let bout_time = ((results[0].1 - results[1].1) as i64 / (results[0].0 as i32 - results[1].0 as i32 )as i64) as u128;


    // run one workday to get point time

    let start = Instant::now();
    workday(4000000, 1, 1000, 1000,  &mut state.current_work_context);
    let time = start.elapsed().as_nanos();

    // subtract to get point time

    let point_time =
        time
            - workday_time
            - bout_time * state.current_work_context.total_bouts_today as u128
            - iteration_time * state.current_work_context.total_iterations_today as u128;


    state.workday_token_cost = workday_time as u32;
    state.iteration_token_cost = iteration_time as u32;
    state.bout_token_cost = iteration_time as u32;
    state.point_token_cost = iteration_time as u32;
}