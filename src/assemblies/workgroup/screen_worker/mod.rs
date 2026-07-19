use std::cmp::{max, min};
use std::ops::{Add, Mul, Sub};
use eframe::epaint::Color32;
use steady_state::*;
use crate::utils::{signed_shift, ObjectivePosAndZoom};
//use crate::actor::work_collector::*;
use crate::assemblies::workgroup::work_controller::*;
use crate::assemblies::workgroup::screen_worker::workshift::*;
use crate::assemblies::workgroup_new::tile_session::TileSession;

pub mod workshift;

pub struct WorkUpdate {
    pub frame_info: Option<(ObjectivePosAndZoom, (u32, u32))>,
    pub completed_points: (Vec<(CompletedPoint, usize)>)
}

pub struct WorkerState {
    work_context: Option<(WorkContext, (ObjectivePosAndZoom, (u32, u32)))>
    , tile_session: Option<TileSession>
    , workshift_token_budget: u32
    , iteration_token_cost: u32
    , point_token_cost: u32
    , bout_token_cost: u32
    , workshift_token_cost: u32
    , total_workshifts: u32
    , unsent_completed_points: Vec<(CompletedPoint, usize)>
}

pub async fn run(
    actor: SteadyActorShadow,
    commands_in: SteadyRx<WorkerCommand>,
    updates_out: SteadyTx<WorkUpdate>,
    attention_in: SteadyRx<(i32, i32)>,
    state: SteadyState<WorkerState>,
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

async fn internal_behavior<A: SteadyActor>(
    mut actor: A,
    commands_in: SteadyRx<WorkerCommand>,
    updates_out: SteadyTx<WorkUpdate>,
    attention_in: SteadyRx<(i32, i32)>,
    state: SteadyState<WorkerState>,
) -> Result<(), Box<dyn Error>> {

    //actor.loglevel(LogLevel::Debug);

    let mut commands_in = commands_in.lock().await;
    let mut updates_out = updates_out.lock().await;
    let mut attention_in = attention_in.lock().await;

    let mut state = state.lock(|| WorkerState {
        work_context: None
        , tile_session: None
        , workshift_token_budget: 16000000
        , iteration_token_cost: 2
        , bout_token_cost: 4
        , workshift_token_cost: 0
        , point_token_cost: 150
        , total_workshifts: 0
        , unsent_completed_points: Vec::new()
    }).await;

    let max_sleep = Duration::from_millis(50);

    while actor.is_running(
        || i!(updates_out.mark_closed())
    ) {

        let working = match &state.tile_session {
            Some(session) => {session.percent_completed() < 100.0}
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
            if let Some(attention) = actor.try_take(&mut attention_in) {
                if let Some(session) = &mut state.tile_session {
                    session.set_attention(attention);
                }
            }
        }

        if actor.avail_units(&mut commands_in) > 0 {

            while actor.avail_units(&mut commands_in) > 1 {
                let stuff = actor.try_take(&mut commands_in).expect("internal error");
                drop(stuff);
            };

            match actor.try_take(&mut commands_in).unwrap() {

                WorkerCommand::Replace{frame_info: frame_info, context:ctx} => {
                    let _ = ctx;
                    if let Some(session) = &mut state.tile_session {
                        let U = session.workshift();
                        if !U.is_empty() {
                            state.unsent_completed_points.extend(U);
                        }
                    }
                    flush_unsent_completed(&mut actor, &mut updates_out, &mut state);
                    state.tile_session = Some(TileSession::new(frame_info.0.clone(), frame_info.1));
                    state.work_context = None;
                    state.unsent_completed_points.clear();
                    match actor.try_send(&mut updates_out, WorkUpdate{frame_info:Some(frame_info), completed_points:vec!()}) {
                        SendOutcome::Success => {}
                        SendOutcome::Blocked(_)
                        | SendOutcome::Timeout(_)
                        | SendOutcome::Closed(_) => {}
                    }
                }
            }
        }

        if let Some(session) = &mut state.tile_session {
            let c = session.workshift();
            state.total_workshifts += 1;
            if !c.is_empty() {
                state.unsent_completed_points.extend(c);
            }
            flush_unsent_completed(&mut actor, &mut updates_out, &mut state);
        }
    }
    // Final shutdown log, reporting all statistics.
    info!("Computer shutting down.");
    Ok(())
}

fn flush_unsent_completed<A: SteadyActor>(
    actor: &mut A
    , updates_out: &mut Tx<WorkUpdate>
    , state: &mut WorkerState
) {
    if state.unsent_completed_points.is_empty() {
        return;
    }
    let batch = std::mem::take(&mut state.unsent_completed_points);
    match actor.try_send(
        updates_out
        , WorkUpdate {
            frame_info: None
            , completed_points: batch
        }
    ) {
        SendOutcome::Success => {}
        SendOutcome::Blocked(msg)
        | SendOutcome::Timeout(msg)
        | SendOutcome::Closed(msg) => {
            state.unsent_completed_points = msg.completed_points;
        }
    }
}

fn work_update(ctx: &mut WorkContext) -> Vec<(CompletedPoint, usize)> {


    //ctx.completed_points
    let update_start = ctx.last_update;
    let mut returned = vec!();
    for _ in 0..ctx.completed_points.len {
        returned.push(ctx.completed_points.try_pop().unwrap())
    }
    /*returned.append(&mut ctx.completed_points);
    ctx.completed_points = vec!();
    ctx.last_update = ctx.index;*/
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
pub fn relative_location_from_index(data_res: (u32, u32), index: usize) -> (i32, i32) {

    (
        index as i32 % (data_res.0) as i32
        , index as i32 / (data_res.1) as i32
        )
}


//screen space uses fixed point i32, 1<<16 is 1.
//multiplication results in an extra 1<<16 which means we have to >> 16
//addition is fine as long as all values invloved are already fixed points
//division cancels the 1<<16 so we have to add it back with << 16

#[inline]
fn sample_color(
    pixels: &Vec<Color32>
    , min_side: u32
    , data_res: (u32, u32)
    , data_len: usize
    , row: usize
    , seat: usize
    //, res_recip: (u32, u32)
    , min_side_recip: i64
    , relative_pos: (i32, i32)
    , relative_zoom_pot: i64
) -> Color32 {
    let color =
        pixels[
            index_from_relative_location(
                transform_relative_location_i32(
                    relative_location_i32_row_and_seat(seat, row)
                    , (relative_pos.0, relative_pos.1)
                    , relative_zoom_pot
                )
                , data_res
                , data_len
            )
            ];
    color
}


#[inline]
pub fn relative_location_i32_row_and_seat(seat: usize, row: usize) -> (i32, i32) {
    let seat = seat as u32;
    let row = row as u32;

    (
        seat as i32
        , row as i32
    )
}

#[inline]
pub fn index_from_relative_location(l: (i32, i32), data_res: (u32, u32), data_length: usize) -> usize {
    let normalized_l = (
        max(min(l.0, (data_res.0 - 1) as i32), 0)
        , max(min(l.1, (data_res.1 - 1) as i32), 0)
    );

    let i =
        (
            (normalized_l.1 as u32 * data_res.0)
                + normalized_l.0 as u32
        ) as usize;

    i
}

#[inline]
pub fn optional_index_from_relative_location(l: (i32, i32), data_res: (u32, u32), data_length: usize) -> Option<usize> {
    if l.0 >= 0 && l.0 <= (data_res.0 - 1) as i32 && l.1 >= 0 && l.1 <= (data_res.1 - 1) as i32 {
        let i =
            (
                (l.1 as u32 * data_res.0)
                    + l.0 as u32
            ) as usize;

        Some(i)
    } else { None }
}

#[inline]
pub fn transform_relative_location_i32(l: (i32, i32), m: (i32, i32), zoom: i64) -> (i32, i32) {
    // move + zoom

    (
        signed_shift(l.0 - m.0, -zoom)
        , signed_shift(l.1 - m.1, -zoom)
    )
}
