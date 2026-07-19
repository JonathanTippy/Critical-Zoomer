use std::cmp::{max, min};
use steady_state::*;
use crate::utils::{signed_shift, ObjectivePosAndZoom};
use crate::assemblies::workgroup::work_controller::*;
use crate::assemblies::workgroup::screen_worker::workshift::*;
use crate::assemblies::workgroup_new::tile_session::TileSession;
use crate::assemblies::structs::*;

pub mod workshift;

pub struct AnswerTilePublish {
    pub tile: Tile<Answer>
    , pub screen_res: (usize, usize)
    , pub location: ObjectivePosAndZoom
}

pub struct WorkUpdate {
    pub frame_info: Option<(ObjectivePosAndZoom, (u32, u32))>
    , pub completed_points: Vec<(CompletedPoint, usize)>
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
    , unsent_tiles: Vec<AnswerTilePublish>
}

pub async fn run(
    actor: SteadyActorShadow
    , commands_in: SteadyRx<WorkerCommand>
    , tiles_out: SteadyTx<AnswerTilePublish>
    , attention_in: SteadyRx<(i32, i32)>
    , state: SteadyState<WorkerState>
) -> Result<(), Box<dyn Error>> {
    internal_behavior(
        actor.into_spotlight([&commands_in, &attention_in], [&tiles_out])
        , commands_in
        , tiles_out
        , attention_in
        , state
    )
        .await
}

async fn internal_behavior<A: SteadyActor>(
    mut actor: A
    , commands_in: SteadyRx<WorkerCommand>
    , tiles_out: SteadyTx<AnswerTilePublish>
    , attention_in: SteadyRx<(i32, i32)>
    , state: SteadyState<WorkerState>
) -> Result<(), Box<dyn Error>> {
    let mut commands_in = commands_in.lock().await;
    let mut tiles_out = tiles_out.lock().await;
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
        , unsent_tiles: Vec::new()
    }).await;

    let max_sleep = Duration::from_millis(50);

    while actor.is_running(
        || i!(tiles_out.mark_closed())
    ) {
        let working = match &state.tile_session {
            Some(session) => {session.percent_completed() < 100.0}
            , None => {false}
        };

        if working {} else {
            await_for_any!(
                actor.wait_periodic(max_sleep)
                , actor.wait_avail(&mut commands_in, 1)
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
                    let published = if let Some(session) = &mut state.tile_session {
                        session.workshift();
                        Some(drain_publish(session))
                    } else {
                        None
                    };
                    if let Some(tiles) = published {
                        state.unsent_tiles.extend(tiles);
                    }
                    flush_unsent_tiles(&mut actor, &mut tiles_out, &mut state);
                    state.tile_session = Some(TileSession::new(frame_info.0.clone(), frame_info.1));
                    state.work_context = None;
                    state.unsent_tiles.clear();
                }
            }
        }

        let published = if let Some(session) = &mut state.tile_session {
            session.workshift();
            Some(drain_publish(session))
        } else {
            None
        };
        if let Some(tiles) = published {
            state.total_workshifts += 1;
            state.unsent_tiles.extend(tiles);
            flush_unsent_tiles(&mut actor, &mut tiles_out, &mut state);
        }
    }
    info!("Computer shutting down.");
    Ok(())
}

fn drain_publish(session: &mut TileSession) -> Vec<AnswerTilePublish> {
    let screen_res = session.screen_res;
    let location = session.location.clone();
    session.drain_publish_tiles().into_iter().map(|tile| {
        AnswerTilePublish {
            tile
            , screen_res
            , location: location.clone()
        }
    }).collect()
}

fn flush_unsent_tiles<A: SteadyActor>(
    actor: &mut A
    , tiles_out: &mut Tx<AnswerTilePublish>
    , state: &mut WorkerState
) {
    let pending = std::mem::take(&mut state.unsent_tiles);
    let mut blocked = Vec::new();
    let mut iter = pending.into_iter();
    while let Some(msg) = iter.next() {
        match actor.try_send(tiles_out, msg) {
            SendOutcome::Success => {}
            SendOutcome::Blocked(msg)
            | SendOutcome::Timeout(msg)
            | SendOutcome::Closed(msg) => {
                blocked.push(msg);
                blocked.extend(iter);
                break;
            }
        }
    }
    state.unsent_tiles = blocked;
}

fn work_update(ctx: &mut WorkContext) -> Vec<(CompletedPoint, usize)> {
    let update_start = ctx.last_update;
    let mut returned = vec!();
    for _ in 0..ctx.completed_points.len {
        returned.push(ctx.completed_points.try_pop().unwrap())
    }
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
    (
        signed_shift(l.0 - m.0, -zoom)
        , signed_shift(l.1 - m.1, -zoom)
    )
}
