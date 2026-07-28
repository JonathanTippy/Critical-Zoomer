use std::time::Instant;
use steady_state::*;
use crate::utils::ObjectivePosAndZoom;
use crate::assemblies::workgroup_new::work_controller::*;
use crate::assemblies::workgroup_new::tile_session::TileSession;
use crate::assemblies::workgroup_new::tile_publisher::{LivePublisher, MemoryBump};
use crate::assemblies::structs::*;
use crate::settings::Settings;


pub struct AnswerTilePublish {
    pub tile: Tile<Answer>
    , pub screen_res: (usize, usize)
    , pub location: ObjectivePosAndZoom
}

pub struct WorkerState {
    tile_session: Option<TileSession>
    , workshift_token_budget: u32
    , iteration_token_cost: u32
    , point_token_cost: u32
    , bout_token_cost: u32
    , workshift_token_cost: u32
    , total_workshifts: u32
    , unsent_tiles: Vec<AnswerTilePublish>
    , full_republish_done: bool
    // Interim host for publisher cadence + memory policy until full actor extract.
    , live_publisher: LivePublisher
}

pub async fn run(
    actor: SteadyActorShadow
    , commands_in: SteadyRx<WorkerCommand>
    , tiles_out: SteadyTx<AnswerTilePublish>
    , attention_in: SteadyRx<(i32, i32)>
    , settings_in: SteadyRx<Settings>
    , memory_bump_out: SteadyTx<MemoryBump>
    , state: SteadyState<WorkerState>
) -> Result<(), Box<dyn Error>> {
    internal_behavior(
        actor.into_spotlight(
            [&commands_in, &attention_in, &settings_in]
            , [&tiles_out, &memory_bump_out]
        )
        , commands_in
        , tiles_out
        , attention_in
        , settings_in
        , memory_bump_out
        , state
    )
        .await
}

async fn internal_behavior<A: SteadyActor>(
    mut actor: A
    , commands_in: SteadyRx<WorkerCommand>
    , tiles_out: SteadyTx<AnswerTilePublish>
    , attention_in: SteadyRx<(i32, i32)>
    , settings_in: SteadyRx<Settings>
    , memory_bump_out: SteadyTx<MemoryBump>
    , state: SteadyState<WorkerState>
) -> Result<(), Box<dyn Error>> {
    let mut commands_in = commands_in.lock().await;
    let mut tiles_out = tiles_out.lock().await;
    let mut attention_in = attention_in.lock().await;
    let mut settings_in = settings_in.lock().await;
    let mut memory_bump_out = memory_bump_out.lock().await;

    let mut state = state.lock(|| WorkerState {
        tile_session: None
        , workshift_token_budget: 16000000
        , iteration_token_cost: 2
        , bout_token_cost: 4
        , workshift_token_cost: 0
        , point_token_cost: 150
        , total_workshifts: 0
        , unsent_tiles: Vec::new()
        , full_republish_done: false
        , live_publisher: LivePublisher::new(true)
    }).await;

        let max_sleep = Duration::from_millis(50);
        // Wake often enough to honor the 30 Hz publish floor while incomplete.
        let publish_floor = Duration::from_millis(34);

    while actor.is_running(
        || i!(tiles_out.mark_closed())
    ) {
        let working = match &state.tile_session {
            Some(session) => {session.percent_completed() < 100.0}
            , None => {false}
        };
        // Yield when we have a publish backlog so uploader/window can drain;
        // otherwise keep computing without a fixed per-loop sleep.
        let backlog = !state.unsent_tiles.is_empty();

        if !working || backlog {
            await_for_any!(
                actor.wait_periodic(if working { publish_floor } else { max_sleep })
                , actor.wait_avail(&mut commands_in, 1)
                , actor.wait_avail(&mut settings_in, 1)
            );
        }

        while actor.avail_units(&mut settings_in) > 0 {
            if let Some(settings) = actor.try_take(&mut settings_in) {
                state.live_publisher.memory_limit_bytes = settings.memory_limit_bytes;
            }
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
                WorkerCommand::Replace { frame_info } => {
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
                    let prev_zoom = state
                        .tile_session
                        .as_ref()
                        .map(|s| s.location.zoom_pot);
                    let new_zoom = frame_info.0.zoom_pot;
                    match &mut state.tile_session {
                        Some(session) => {
                            session.retarget(frame_info.0.clone(), frame_info.1);
                        }
                        None => {
                            state.tile_session = Some(TileSession::new(
                                frame_info.0.clone()
                                , frame_info.1
                            ));
                        }
                    }
                    if let (Some(session), Some(prev)) = (&mut state.tile_session, prev_zoom) {
                        // retarget already sets mag velocity on zoom change; keep pan at 0
                        if new_zoom == prev {
                            session.set_mag_velocity(0);
                        }
                    }
                    state.live_publisher.set_incomplete(true);
                    state.full_republish_done = false;
                    // Keep in-flight publishes; absolute hoard keys retain continuity.
                }
            }
        }

        let need_full_republish = !state.full_republish_done;
        let workshifts = state.total_workshifts;
        let now = Instant::now();
        // Take session so we can touch live_publisher without overlapping borrows.
        let mut session_slot = state.tile_session.take();
        let published = if let Some(session) = session_slot.as_mut() {
            // One budgeted workshift then flush so the uploader/window keep pace
            // with seat completion (double 100ms shifts starved the publish path).
            session.workshift();
            let pct = session.percent_completed();
            let incomplete = pct < 100.0;
            state.live_publisher.set_incomplete(incomplete);
            let pulse_full = pct >= 70.0 && workshifts % 4 == 0;
            let complete_full = pct >= 100.0 && need_full_republish;
            let has_work = session.has_unsent_publish() || !state.unsent_tiles.is_empty();
            let cadence_ok = complete_full
                || pulse_full
                || state.live_publisher.should_publish(now, has_work);

            let mut tiles = Vec::new();
            if cadence_ok {
                tiles = drain_publish(session);
                // Full republish at completion, and pulse near the end so headed
                // display does not keep stale NORES blocks past the <5s bar.
                if complete_full || pulse_full {
                    let screen_res = session.screen_res;
                    let location = session.location.clone();
                    let all = session.drain_all_answer_tiles();
                    for tile in all {
                        tiles.push(AnswerTilePublish {
                            tile
                            , screen_res
                            , location: location.clone()
                        });
                    }
                }
                if !tiles.is_empty() || complete_full || pulse_full {
                    state.live_publisher.record_publish(now);
                }
            }

            // Memory policy: prune hoard; bump when protected exceeds limit.
            let memory_limit = state.live_publisher.memory_limit_bytes;
            if let Some(needed) = session.prune_for_memory(memory_limit) {
                state.live_publisher.memory_limit_bytes = needed.max(
                    state.live_publisher.memory_limit_bytes
                );
                let _ = actor.try_send(
                    &mut memory_bump_out
                    , MemoryBump { needed_bytes: needed }
                );
            }

            Some((tiles, pct, complete_full))
        } else {
            None
        };
        state.tile_session = session_slot;
        if let Some((tiles, pct, do_full)) = published {
            state.total_workshifts += 1;
            if do_full {
                state.full_republish_done = true;
            }
            if pct < 100.0 {
                state.full_republish_done = false;
            }
            state.unsent_tiles.extend(tiles);
            let pending_before = state.unsent_tiles.len();
            flush_unsent_tiles(&mut actor, &mut tiles_out, &mut state);
            if do_full
                || std::env::var("CZ_DEBUG_FILL")
                    .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                    .unwrap_or(false)
                    && state.total_workshifts % 20 == 1
            {
                let _ = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open("/tmp/cz_fill_debug.log")
                    .and_then(|mut file| {
                        use std::io::Write;
                        writeln!(
                            file
                            , "publish ws={} pct={:.2} do_full={} queued={} leftover={}"
                            , state.total_workshifts
                            , pct
                            , do_full
                            , pending_before
                            , state.unsent_tiles.len()
                        )
                    });
            }
        }
    }
    info!("Computer shutting down.");
    Ok(())
}

fn drain_publish(session: &mut TileSession) -> Vec<AnswerTilePublish> {
    let screen_res = session.screen_res;
    let location = session.location.clone();
    let mut out: Vec<AnswerTilePublish> = session.drain_publish_tiles().into_iter().map(|tile| {
        AnswerTilePublish {
            tile
            , screen_res
            , location: location.clone()
        }
    }).collect();
    for (loc, tile) in session.drain_lookahead_publishes() {
        out.push(AnswerTilePublish {
            tile: *tile
            , screen_res
            , location: loc
        });
    }
    out
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
