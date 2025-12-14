use std::cmp::min;
use std::collections::VecDeque;
use rand::prelude::SliceRandom;
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
pub(crate) struct WorkerState {
    work_context: Option<(WorkContext, (ObjectivePosAndZoom, (u32, u32)))>
    , workshift_token_budget: u32
    , iteration_token_cost: u32
    , point_token_cost: u32
    , bout_token_cost: u32
    , workshift_token_cost: u32
    , total_workshifts: u32
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

        if actor.avail_units(&mut commands_in) > 0 {

            while actor.avail_units(&mut commands_in) > 1 {
                let stuff = actor.try_take(&mut commands_in).expect("internal error");
                drop(stuff);
            };

            match actor.try_take(&mut commands_in).unwrap() {

                WorkerCommand::Replace{frame_info: frame_info} => {

                    let loc:(f64, f64) = (
                        frame_info.0.pos.0.clone().into()
                        , frame_info.0.pos.1.clone().into()
                    );

                    let loc = (
                        loc.0
                        , -loc.1
                    );

                    let mut edges = Vec::new();
                    let res = frame_info.1;
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

                    let ctx = WorkContext {
                        points: get_points_f32(frame_info.1, loc, frame_info.0.zoom_pot as i64)
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
                        , res: frame_info.1
                        , scredge_poses: VecDeque::from(edges)
                        , edge_queue: VecDeque::new()
                        , out_queue: VecDeque::new()
                        , in_queue: VecDeque::new()
                    };
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
                    , loop_detection_point: (point, 1)
                    , done: (false, false)
                    , delivered: false
                    , period: 0
                }
            )
        }
    }
    Points::F32{p:out}
}

fn get_random_mixmap(size: usize) -> Vec<usize> {
    let mut rng = rand::rng();

    let mut indices: Vec<usize> = (0..size).collect();

    // Shuffle indices randomly
    indices.shuffle(&mut rng);
    indices
}

fn get_interlaced_mixmap(res:(u32, u32), size:usize) -> Vec<usize> {
    let mut rng = rand::rng();

    let mut row_indices:Vec<usize> = (0..res.1 as usize).collect();
    row_indices.shuffle(&mut rng);

    let mut indices: Vec<usize> = (0..size).collect();
    for mut index in &mut indices {
        *index = *index % res.0 as usize
            +
            row_indices[(*index / res.0 as usize)]
                * res.0 as usize

    }
    indices
}