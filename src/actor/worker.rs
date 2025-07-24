use steady_state::*;

use crate::actor::window::*;
use crate::actor::updater::*;
use crate::action::workday::*;
use crate::action::sampling::*;

use rand::Rng;

use std::cmp::*;
use crate::action::utils::*;

pub(crate) struct ZoomerScreenValues {
    pub(crate) values: Vec<(u32)>
    , pub(crate) screen_size: (u32, u32)
    , pub(crate) originating_relative_transforms: SamplingRelativeTransforms
    , pub(crate) complete: bool
    , pub(crate) dummy: bool
}

pub(crate) struct WorkerState {
    work_context: WorkContextF32
    , loc: (f64, f64)
    , zoom_pot: i64
    , workday_token_budget: u32
    , iteration_token_cost: u32
    , point_token_cost: u32
    , bout_token_cost: u32
    , workday_token_cost: u32
}


pub(crate) const WORKER_INIT_RES_POT:u64 = 9;
pub(crate) const WORKER_INIT_RES:(u32, u32) = (1<<WORKER_INIT_RES_POT, 1<<WORKER_INIT_RES_POT);
pub(crate) const WORKER_INIT_LOC:(f64, f64) = (0.0, 0.0);
pub(crate) const WORKER_INIT_ZOOM_POT: i64 = -2;
pub(crate) const WORKER_INIT_ZOOM:f64 = if WORKER_INIT_ZOOM_POT>0 {(1<<WORKER_INIT_ZOOM_POT) as f64} else {1.0 / (1<<-WORKER_INIT_ZOOM_POT) as f64};
pub(crate) const PIXELS_PER_UNIT: u64 = 1<<(WORKER_INIT_RES_POT);

pub async fn run(
    actor: SteadyActorShadow,
    transforms_in: SteadyRx<SamplingRelativeTransforms>,
    values_out: SteadyTx<ZoomerScreenValues>,
    state: SteadyState<WorkerState>,
) -> Result<(), Box<dyn Error>> {
    // The worker is tested by its simulated neighbors, so we always use internal_behavior.
    internal_behavior(
        actor.into_spotlight([&transforms_in], [&values_out]),
        transforms_in,
        values_out,
        state,
    )
        .await
}

async fn internal_behavior<A: SteadyActor>(
    mut actor: A,
    transforms_in: SteadyRx<SamplingRelativeTransforms>,
    values_out: SteadyTx<ZoomerScreenValues>,
    state: SteadyState<WorkerState>,
) -> Result<(), Box<dyn Error>> {
    let mut transforms_in = transforms_in.lock().await;
    let mut values_out = values_out.lock().await;

    let mut state = state.lock(|| WorkerState {
        loc: WORKER_INIT_LOC
        , zoom_pot: WORKER_INIT_ZOOM_POT
        , work_context: WorkContextF32 {
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
            , originating_relative_transforms: SamplingRelativeTransforms{pos: (0, 0), zoom_pot: 0, counter: 0}
        }
        , workday_token_budget: 40000000
        , iteration_token_cost: 2
        , bout_token_cost: 4
        , workday_token_cost: 0
        , point_token_cost: 150
    }).await;

    let max_sleep = Duration::from_millis(50);

    let workday_duration = Duration::from_millis(50);

    while actor.is_running(
        || i!(values_out.mark_closed())
    ) {


        if actor.avail_units(&mut transforms_in) > 0 {
            while actor.avail_units(&mut transforms_in) > 1 {
                drop(actor.try_take(&mut transforms_in).expect("internal error"))
            };

            let transforms = actor.try_take(&mut transforms_in).expect("internal error");
            if transforms.counter > state.work_context.originating_relative_transforms.counter {
                handle_transforms(
                    &mut state
                    , transforms
                );
            }

        }


        let working = state.work_context.percent_completed < 99.9999999;

        // this actor always pins its core if its doing work.
        if working {} else {
            /*await_for_any!(
                actor.wait_periodic(max_sleep),
                //actor.wait_avail(&mut transforms_in, 1),
            );*/
            //std::thread::sleep(Duration::from_millis(40));
        }



        if working {

            let start = Instant::now();

            match workday (
                state.workday_token_budget - state.workday_token_cost - state.bout_token_cost
                , state.iteration_token_cost
                , state.bout_token_cost
                , state.point_token_cost
                ,  &mut state.work_context)
            {
                Some(c) => {
                    if state.work_context.percent_completed == 100.0 {
                        //info!("context is done. Total time: {:.2}s\n total iterations: {}", state.work_context.time_created.elapsed().as_secs_f64(), state.work_context.total_iterations);
                    } else {
                        //info!("workday completed. context is now {:.2}% done.", state.work_context.percent_completed);
                    }

                    actor.try_send(&mut values_out, ZoomerScreenValues{
                        values: strip_destination_f32(c)
                        , screen_size: WORKER_INIT_RES
                        , originating_relative_transforms: state.work_context.originating_relative_transforms.clone()
                        , complete: state.work_context.percent_completed == 100.0
                        , dummy: false
                    });
                }
                None => {
                    //info!("workday completed. context is now {:.2}% done.", state.work_context.percent_completed);
                }
            }

            let workday_length = start.elapsed();
            //info!("workday took {:.2} ms", workday_length.as_secs_f64() * 1000.0);

        } else {
            actor.wait(Duration::from_millis(40)).await;

            actor.try_send(&mut values_out, ZoomerScreenValues{
                values: vec!()
                , screen_size: (0, 0)
                , originating_relative_transforms: state.work_context.originating_relative_transforms.clone()
                , complete: true
                , dummy: true
            });
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

fn handle_transforms(state: &mut WorkerState, transforms: SamplingRelativeTransforms) {


    if (transforms.pos != (0, 0)) || (transforms.zoom_pot != 0) {

        info!("changing zoom from {} to {} based on counter number {}", state.zoom_pot, state.zoom_pot + transforms.zoom_pot, transforms.counter);

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


        state.work_context = WorkContextF32 {
            points: get_points_f32((WORKER_INIT_RES.0, WORKER_INIT_RES.1), state.loc, state.zoom_pot)
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
            , originating_relative_transforms: transforms
        };
    } else {
        state.work_context.originating_relative_transforms = transforms;
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