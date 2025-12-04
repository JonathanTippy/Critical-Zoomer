use std::cmp::min;
use steady_state::*;
use crate::action::sampling::{index_from_relative_location, relative_location_i32_row_and_seat, transform_relative_location_i32};
use crate::action::utils::ObjectivePosAndZoom;
use crate::action::workshift::*;
use crate::actor::work_collector::ScreenValue;
use crate::actor::work_controller::*;




pub(crate) struct WorkUpdate {
    pub(crate) frame_info: Option<(ObjectivePosAndZoom, (u32, u32))>,
    pub(crate) completed_points: (Vec<(CompletedPoint, usize)>)
}

#[derive(Clone)]
pub(crate) struct WorkerState {
    work_context: Option<(WorkContext, (ObjectivePosAndZoom, (u32, u32)))>
    , workshift_token_budget: u32
    , iteration_token_cost: u32
    , point_token_cost: u32
    , bout_token_cost: u32
    , workshift_token_cost: u32
}

pub async fn run(
    actor: SteadyActorShadow,
    commands_in: SteadyRx<WorkerCommand>,
    updates_out: SteadyTx<WorkUpdate>,
    state: SteadyState<WorkerState>,
) -> Result<(), Box<dyn Error>> {
    // The worker is tested by its simulated neighbors, so we always use internal_behavior.
    internal_behavior(
        actor.into_spotlight([&commands_in], [&updates_out]),
        commands_in,
        updates_out,
        state,
    )
        .await
}

async fn internal_behavior<A: SteadyActor>(
    mut actor: A,
    commands_in: SteadyRx<WorkerCommand>,
    updates_out: SteadyTx<WorkUpdate>,
    state: SteadyState<WorkerState>,
) -> Result<(), Box<dyn Error>> {

    actor.loglevel(LogLevel::Debug);

    let mut commands_in = commands_in.lock().await;
    let mut updates_out = updates_out.lock().await;

    let mut state = state.lock(|| WorkerState {
        work_context: None
        , workshift_token_budget: 1000000
        , iteration_token_cost: 2
        , bout_token_cost: 4
        , workshift_token_cost: 0
        , point_token_cost: 150
    }).await;

    let max_sleep = Duration::from_millis(50);

    while actor.is_running(
        || i!(updates_out.mark_closed())
    ) {

        let working = if let Some(_) = state.work_context {true} else {false};

        if working {} else {
            await_for_any!(
                actor.wait_periodic(max_sleep),
                actor.wait_avail(&mut commands_in, 1),
            );
        }

        while actor.avail_units(&mut commands_in) > 0 {
            match actor.try_take(&mut commands_in).unwrap() {
                WorkerCommand::Update => {
                    if let Some(ctx) = &mut state.work_context {
                        let c = work_update(&mut ctx.0);
                        if c.len() > 0 {
                            actor.try_send(&mut updates_out, WorkUpdate{frame_info:None, completed_points:c});
                        }
                    }
                }
                WorkerCommand::Replace{frame_info: frame_info, context:ctx} => {
                    if let Some((old_ctx, old_frame_info)) = &mut state.work_context {
                        let U = work_update(old_ctx);

                        if U.len() > 0 {
                            actor.try_send(&mut updates_out, WorkUpdate{frame_info:None, completed_points:U});
                        }

                        let old_size = old_frame_info.1.0 * old_frame_info.1.1;

                        let relative_pos = (
                            old_frame_info.0.pos.0.clone()-frame_info.0.pos.0.clone()
                            , old_frame_info.0.pos.1.clone()-frame_info.0.pos.1.clone()
                        );

                        let relative_pos_in_pixels:(i32, i32) = (
                            relative_pos.0.shift(frame_info.0.zoom_pot).shift(crate::actor::work_controller::PIXELS_PER_UNIT_POT).into()
                            , relative_pos.1.shift(frame_info.0.zoom_pot).shift(crate::actor::work_controller::PIXELS_PER_UNIT_POT).into()
                        );

                        let relative_zoom = frame_info.0.zoom_pot - old_frame_info.0.zoom_pot;

                        let mut new_ctx = ctx;

                        for A in old_ctx.already_done.clone() {
                            if let Some(a) = transform_index(
                                A
                                , old_frame_info.1
                                , frame_info.1
                                , old_size as usize
                                , relative_pos_in_pixels
                                , relative_zoom as i64
                            ) {
                                new_ctx.already_done.push(a);
                            }
                        }
                        for A in new_ctx.already_done.clone() {
                            new_ctx.already_done_hashset.insert(A);
                        }

                        state.work_context = Some((new_ctx, frame_info.clone()));
                        actor.try_send(&mut updates_out, WorkUpdate{frame_info:Some(frame_info), completed_points:vec!()});

                    } else {
                        state.work_context = Some((ctx, frame_info.clone()));
                        actor.try_send(&mut updates_out, WorkUpdate{frame_info:Some(frame_info), completed_points:vec!()});
                        //debug!("screen worker got new context: \n{:?}", state.work_context);
                    }

                }
            }
        }

        let token_budget = state.workshift_token_budget.clone();
        let iteration_token_cost = state.iteration_token_cost.clone();
        let bout_token_cost = state.bout_token_cost.clone();
        let point_token_cost = state.point_token_cost.clone();
        

        if let Some(ctx) = &mut state.work_context {
            let start = Instant::now();
            workshift (
                token_budget
                , iteration_token_cost
                , bout_token_cost
                , point_token_cost
                , &mut ctx.0
            );
            //info!("workday completed. took {}ms.", start.elapsed().as_millis());
        }
    }
    // Final shutdown log, reporting all statistics.
    info!("Computer shutting down.");
    Ok(())
}

fn calculate_tokens(state: &mut WorkerState) {

}

fn work_update(ctx: &mut WorkContext) -> Vec<(CompletedPoint, usize)> {
    let update_start = ctx.last_update;
    let mut returned = vec!();
    returned.append(&mut ctx.completed_points);
    ctx.completed_points = vec!();
    ctx.last_update = ctx.index;
    returned
}

#[inline]
fn transform_index(
    i: usize
    , in_data_res: (u32, u32)
    , out_data_res: (u32, u32)
    , out_data_len: usize
    , relative_pos: (i32, i32)
    , relative_zoom_pot: i64
) -> Option<usize> {

    let l = transform_relative_location_i32(
        relative_location_from_index(
            in_data_res, i
        )
        , relative_pos
        , relative_zoom_pot
    );

    if l.0 <= out_data_res.0 as i32 && l.0 > 0 && l.1 > 0 && l.1 <= out_data_res.1 as i32 {
        Some(index_from_relative_location(
            l
            , out_data_res
            , out_data_len
        ))
    } else {
        None
    }
}

#[inline]
pub(crate) fn relative_location_from_index(data_res: (u32, u32), index: usize) -> (i32, i32) {

    (
        index as i32 % data_res.0 as i32
        , index as i32 / data_res.1 as i32
        )
}