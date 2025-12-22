use std::cmp::min;
use std::ops::{Add, Mul, Sub};
use steady_state::*;
use crate::action::sampling::{index_from_relative_location, relative_location_i32_row_and_seat, transform_relative_location_i32};
use crate::action::utils::ObjectivePosAndZoom;
use crate::action::workshift::*;
//use crate::actor::work_collector::*;
use crate::actor::work_controller::*;




pub(crate) struct WorkUpdate {
    pub(crate) frame_info: Option<(ObjectivePosAndZoom, (u32, u32))>,
    pub(crate) completed_points: (Vec<(CompletedPoint, usize)>)
}

#[derive(Clone)]
pub(crate) struct WorkerState<T> {
    work_context: Option<(WorkContext<T>, (ObjectivePosAndZoom, (u32, u32)))>
    , workshift_token_budget: u32
    , iteration_token_cost: u32
    , point_token_cost: u32
    , bout_token_cost: u32
    , workshift_token_cost: u32
    , total_workshifts: u32
}

pub async fn run(
    actor: SteadyActorShadow,
    commands_in: SteadyRx<WorkerCommand<f64>>,
    updates_out: SteadyTx<WorkUpdate>,
    attention_in: SteadyRx<(i32, i32)>,
    state: SteadyState<WorkerState<f64>>,
) -> Result<(), Box<dyn Error>> {
    // The worker is tested by its simulated neighbors, so we always use internal_behavior.
    internal_behavior(
        actor.into_spotlight([&commands_in, &attention_in], [&updates_out]),
        commands_in,
        updates_out,
        attention_in,
        state,
    )
        .await
}

async fn internal_behavior<A: SteadyActor, T: Send + Sub<Output=T> + Add<Output=T> + Mul<Output=T> + PartialOrd + crate::action::workshift::Finite + crate::action::workshift::Gt + crate::action::workshift::Abs + From<f32> + Into<f64> + Copy>(
    mut actor: A,
    commands_in: SteadyRx<WorkerCommand<T>>,
    updates_out: SteadyTx<WorkUpdate>,
    attention_in: SteadyRx<(i32, i32)>,
    state: SteadyState<WorkerState<T>>,
) -> Result<(), Box<dyn Error>> {

    actor.loglevel(LogLevel::Debug);

    let mut commands_in = commands_in.lock().await;
    let mut updates_out = updates_out.lock().await;
    let mut attention_in = attention_in.lock().await;

    let mut state = state.lock(|| WorkerState {
        work_context: None
        , workshift_token_budget: 16000000
        , iteration_token_cost: 2
        , bout_token_cost: 4
        , workshift_token_cost: 0
        , point_token_cost: 150
        , total_workshifts: 0
    }).await;

    let max_sleep = Duration::from_millis(50);

    while actor.is_running(
        || i!(updates_out.mark_closed())
    ) {

        let working = match &state.work_context {
            Some(ctx) => {ctx.0.percent_completed < 100.0}
            , None => {false}
        };

        if working {} else {
            await_for_any!(
                actor.wait_periodic(max_sleep),
                actor.wait_avail(&mut commands_in, 1),
            );
        }

        if actor.avail_units(&mut attention_in) > 0 {
            while actor.avail_units(&mut attention_in) > 1 {
                let stuff = actor.try_take(&mut attention_in).expect("internal error");
                drop(stuff);
            };
            let attention = actor.try_take(&mut attention_in).expect("internal error");
            if let Some((ctx, _)) = &mut state.work_context {
                ctx.attention = attention;
            }
        }

        if actor.avail_units(&mut commands_in) > 0 {

            while actor.avail_units(&mut commands_in) > 1 {
                let stuff = actor.try_take(&mut commands_in).expect("internal error");
                drop(stuff);
            };

            match actor.try_take(&mut commands_in).unwrap() {

                WorkerCommand::Replace{frame_info: frame_info, context:ctx} => {
                    if let Some((old_ctx, old_frame_info)) = &mut state.work_context {
                        let U = work_update(old_ctx);

                        if U.len() > 0 {
                            actor.try_send(&mut updates_out, WorkUpdate{frame_info:None, completed_points:U});
                        }

                        state.work_context = Some((ctx, frame_info.clone()));
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
            //let start = Instant::now();
            workshift (
                token_budget
                , iteration_token_cost
                , bout_token_cost
                , point_token_cost
                , &mut ctx.0
            );
            state.total_workshifts+=1;
            //info!("workday completed. took {}ms.", start.elapsed().as_millis());
            //info!("workshift {}", state.total_workshifts);
        }


        if state.total_workshifts % 1 == 0 {
            if let Some(ctx) = &mut state.work_context {
                let c = work_update(&mut ctx.0);
                if c.len() > 0 {
                    actor.try_send(&mut updates_out, WorkUpdate{frame_info:None, completed_points:c});
                }
            }
        }
    }
    // Final shutdown log, reporting all statistics.
    info!("Computer shutting down.");
    Ok(())
}

fn work_update<T>(ctx: &mut WorkContext<T>) -> Vec<(CompletedPoint, usize)> {
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

    if l.0 <= (out_data_res.0-1) as i32
        && l.0 > 0
        && l.1 > 0
        && l.1 <= (out_data_res.1-1) as i32
    {
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
        index as i32 % (data_res.0) as i32
        , index as i32 / (data_res.1) as i32
        )
}