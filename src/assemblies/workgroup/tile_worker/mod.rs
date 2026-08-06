// read delivery.md for project context
//! Tile worker SteadyState actor (auth workgroup sub-actor).
//! Hosts TileSession workshifts; emits answer tiles to the GPU uploader.

use steady_state::*;

use crate::assemblies::workgroup::actor_messages::{
    IntratileClient, IntratileReply, IntratileRequest, ReferenceDelivery, SchedulerToWorker,
};
use crate::assemblies::workgroup::tile_publisher::MemoryBump;
use crate::assemblies::workgroup::tile_session::TileSession;
use crate::assemblies::structs::*;
use crate::utils::ObjectivePosAndZoom;

pub struct AnswerTilePublish {
    pub tile: Tile<Answer>,
    pub screen_res: (usize, usize),
    pub location: ObjectivePosAndZoom,
}

pub struct WorkerState {
    tile_session: Option<TileSession>,
    total_workshifts: u32,
    unsent_tiles: Vec<AnswerTilePublish>,
    full_republish_done: bool,
    /// Incomplete flag for publisher cadence signaling via tile stream presence.
    incomplete: bool,
    memory_limit_bytes: usize,
}

/// Latest-wins coalesce of a drained command burst.
#[derive(Clone, Debug, Default)]
pub struct CoalescedWorkerCommands {
    pub attention: Option<(i32, i32)>,
    pub retarget: Option<(ObjectivePosAndZoom, (u32, u32), f64)>,
}

/// Fully drain a command sequence, keeping only the newest attention and retarget.
// r[impl cz.play.actor-drain+1]
// r[impl cz.play.latest-wins+1]
pub fn coalesce_scheduler_commands(
    msgs: impl IntoIterator<Item = SchedulerToWorker>,
) -> CoalescedWorkerCommands {
    let mut out = CoalescedWorkerCommands::default();
    for msg in msgs {
        match msg {
            SchedulerToWorker::SetAttention(x, y) => {
                out.attention = Some((x, y));
            }
            SchedulerToWorker::Retarget { frame_info } => {
                out.retarget = Some(frame_info);
            }
        }
    }
    out
}

/// Actors re-check inboxes at this cadence (start-of-loop poll).
// r[impl cz.play.actor-poll+1]
pub const PLAY_INPUT_POLL_MS: u64 = 1;

/// Workshift budget for one tile-worker loop iteration (auth play-minimize).
/// After retarget or while zooming with zero screen seats, burst long enough to
/// finish first seats — 1ms×1-step quanta left headed GPU paths ~1–2s idle.
pub fn workshift_budget_ms_for_session(
    had_retarget: bool,
    mag_velocity: i32,
    seats_done: usize,
) -> u128 {
    let play_burst = had_retarget || (mag_velocity != 0 && seats_done == 0);
    if play_burst {
        8
    } else if mag_velocity != 0 {
        1
    } else {
        16
    }
}

pub async fn run(
    actor: SteadyActorShadow,
    commands_in: SteadyRx<SchedulerToWorker>,
    reference_in: SteadyRx<ReferenceDelivery>,
    tiles_out: SteadyTx<AnswerTilePublish>,
    bypass_out: SteadyTx<GpuTileHandle>,
    memory_bump_out: SteadyTx<MemoryBump>,
    to_intratile: SteadyTx<IntratileRequest>,
    from_intratile: SteadyRx<IntratileReply>,
    _intratile_rpc: IntratileClient,
    state: SteadyState<WorkerState>,
) -> Result<(), Box<dyn Error>> {
    // Auth: worker owns default spiral locally. Intratile is guidance-only;
    // do not install sync-RPC that shuttles full outfill state (H-ITS-BIND).
    internal_behavior(
        actor.into_spotlight(
            [&commands_in, &reference_in, &from_intratile],
            [&tiles_out, &bypass_out, &memory_bump_out, &to_intratile],
        ),
        commands_in,
        reference_in,
        tiles_out,
        bypass_out,
        memory_bump_out,
        to_intratile,
        from_intratile,
        state,
    )
    .await
}

/// Put `tile_session` back into `WorkerState` even when workshift panics, so
/// SteadyState persistence does not save `tile_session: None` across regeneration.
struct SessionRestore {
    dest: *mut Option<TileSession>,
    slot: Option<TileSession>,
}

impl SessionRestore {
    fn new(state: &mut WorkerState) -> Self {
        let slot = state.tile_session.take();
        SessionRestore {
            dest: &mut state.tile_session,
            slot,
        }
    }
}

impl Drop for SessionRestore {
    fn drop(&mut self) {
        // SAFETY: `dest` points at `state.tile_session` for this stack frame only;
        // we do not move `state` while the guard lives.
        unsafe {
            if (*self.dest).is_none() {
                *self.dest = self.slot.take();
            }
        }
    }
}

async fn internal_behavior<A: SteadyActor>(
    mut actor: A,
    commands_in: SteadyRx<SchedulerToWorker>,
    reference_in: SteadyRx<ReferenceDelivery>,
    tiles_out: SteadyTx<AnswerTilePublish>,
    bypass_out: SteadyTx<GpuTileHandle>,
    memory_bump_out: SteadyTx<MemoryBump>,
    to_intratile: SteadyTx<IntratileRequest>,
    from_intratile: SteadyRx<IntratileReply>,
    state: SteadyState<WorkerState>,
) -> Result<(), Box<dyn Error>> {
    let mut commands_in = commands_in.lock().await;
    let mut reference_in = reference_in.lock().await;
    let mut tiles_out = tiles_out.lock().await;
    let mut bypass_out = bypass_out.lock().await;
    let mut memory_bump_out = memory_bump_out.lock().await;
    let mut to_intratile = to_intratile.lock().await;
    let mut from_intratile = from_intratile.lock().await;

    let mut state = state
        .lock(|| WorkerState {
            tile_session: None,
            total_workshifts: 0,
            unsent_tiles: Vec::new(),
            full_republish_done: false,
            incomplete: true,
            memory_limit_bytes: 1_000_000_000,
        })
        .await;

    // Play / responsiveness: always re-enter the loop ≈1000Hz so input is
    // drained before more work on a stale retarget.
    // r[impl cz.play.actor-poll+1]
    let input_pace = Duration::from_millis(PLAY_INPUT_POLL_MS);

    while actor.is_running(|| {
        i!(tiles_out.mark_closed());
        i!(bypass_out.mark_closed())
    }) {
        // 1) Always check inputs at a quick pace (never skip when "busy").
        await_for_any!(
            actor.wait_periodic(input_pace),
            actor.wait_avail(&mut commands_in, 1),
            actor.wait_avail(&mut reference_in, 1),
        );

        // 2) Fully drain every inbox; 3) latest-wins on coalescible traffic.
        while actor.avail_units(&mut from_intratile) > 0 {
            let _ = actor.try_take(&mut from_intratile);
        }

        while actor.avail_units(&mut reference_in) > 0 {
            let _ = actor.try_take(&mut reference_in);
        }

        let mut drained = Vec::new();
        while actor.avail_units(&mut commands_in) > 0 {
            if let Some(msg) = actor.try_take(&mut commands_in) {
                drained.push(msg);
            } else {
                break;
            }
        }
        let drained_n = drained.len();
        let coalesced = coalesce_scheduler_commands(drained);
        let had_retarget = coalesced.retarget.is_some();
        // #region agent log
        if coalesced.retarget.is_some() || coalesced.attention.is_some() || drained_n > 1 {
            crate::assemblies::workgroup::debug_session::log(
                "H-INPUT-DRAIN",
                "tile_worker/mod.rs:loop",
                "commands_coalesced",
                &format!(
                    "{{\"drained_n\":{drained_n},\"has_retarget\":{},\"has_attention\":{},\"latest_wins\":true}}",
                    coalesced.retarget.is_some(),
                    coalesced.attention.is_some()
                ),
            );
        }
        // #endregion

        if let Some(frame_info) = coalesced.retarget {
            // Immediately prioritize newest view — never workshift the old target.
            let mut preempt_lookahead = Vec::new();
            if let Some(session) = state.tile_session.as_mut() {
                session.flush_pending_lookahead();
                let screen_res = session.screen_res;
                for (loc, tile) in session.drain_lookahead_publishes() {
                    preempt_lookahead.push(AnswerTilePublish {
                        tile: *tile,
                        screen_res,
                        location: loc,
                    });
                }
            }
            state.unsent_tiles.clear();
            let (location, res, mag_velocity) = frame_info;
            let mode = crate::assemblies::headgroup::window::inputs::mag_velocity_mode(
                mag_velocity,
            );
            match &mut state.tile_session {
                Some(session) => {
                    session.retarget(location, res);
                    session.set_mag_velocity(mode);
                }
                None => {
                    let mut session = TileSession::new(location, res);
                    session.set_mag_velocity(mode);
                    state.tile_session = Some(session);
                }
            }
            state.unsent_tiles.extend(preempt_lookahead);
            state.incomplete = true;
            state.full_republish_done = false;
        }
        if let Some((x, y)) = coalesced.attention {
            if let Some(session) = &mut state.tile_session {
                session.set_attention((x, y));
            }
        }

        let need_full_republish = !state.full_republish_done;
        let workshifts = state.total_workshifts;
        let mut session_restore = SessionRestore::new(&mut state);
        let published = if let Some(session) = session_restore.slot.as_mut() {
            // Play / retarget: burst while screen seats are empty so first publish
            // lands inside the 100ms bar; steady zoom uses 1ms quanta between drains.
            let budget_ms = workshift_budget_ms_for_session(
                had_retarget,
                session.mag_velocity(),
                session.seats_done_for_test(),
            );
            let t0 = std::time::Instant::now();
            session.workshift_budget_ms(budget_ms);
            let work_ms = t0.elapsed().as_secs_f64() * 1000.0;
            // #region agent log
            if work_ms > 3.0 || had_retarget {
                crate::assemblies::workgroup::debug_session::log(
                    "H-WORK-QUANTUM",
                    "tile_worker/mod.rs:workshift",
                    "post_input_workshift",
                    &format!(
                        "{{\"work_ms\":{work_ms:.3},\"after_retarget\":{had_retarget},\"pct\":{:.2}}}",
                        session.percent_completed()
                    ),
                );
            }
            // #endregion
            let pct = session.percent_completed();
            let incomplete = pct < 100.0;
            state.incomplete = incomplete;

            if incomplete && workshifts % 64 == 0 {
                let _ = actor.try_send(&mut to_intratile, IntratileRequest::GraphPulse);
            }

            let complete_full = pct >= 100.0 && need_full_republish;
            let has_work = session.has_unsent_publish()
                || session.has_pending_gpu_bypass()
                || !state.unsent_tiles.is_empty();
            let cadence_ok = complete_full || has_work || incomplete;

            let mut tiles = Vec::new();
            if cadence_ok {
                tiles = drain_publish(session);
                if complete_full {
                    let screen_res = session.screen_res;
                    let location = session.location.clone();
                    let all = session.drain_all_answer_tiles();
                    for tile in all {
                        tiles.push(AnswerTilePublish {
                            tile,
                            screen_res,
                            location: location.clone(),
                        });
                    }
                }
            }

            let memory_limit = state.memory_limit_bytes;
            if let Some(needed) = session.prune_for_memory(memory_limit) {
                // #region agent log
                crate::assemblies::workgroup::debug_session::log(
                    "H-STALL",
                    "tile_worker/mod.rs:workshift",
                    "memory_bump",
                    &format!(
                        "{{\"old_limit\":{memory_limit},\"needed\":{needed},\"workshifts\":{}}}",
                        state.total_workshifts
                    ),
                );
                // #endregion
                state.memory_limit_bytes = needed.max(state.memory_limit_bytes);
                let _ = actor.try_send(
                    &mut memory_bump_out,
                    MemoryBump {
                        needed_bytes: needed,
                    },
                );
            }

            Some((tiles, pct, complete_full))
        } else {
            None
        };
        state.tile_session = session_restore.slot.take();
        if let Some((tiles, pct, do_full)) = published {
            state.total_workshifts += 1;
            if do_full {
                state.full_republish_done = true;
            }
            if pct < 100.0 {
                state.full_republish_done = false;
            }
            state.unsent_tiles.extend(tiles);
            flush_unsent_tiles(&mut actor, &mut tiles_out, &mut bypass_out, &mut state);
        }
    }
    info!("Tile worker shutting down.");
    Ok(())
}

fn drain_publish(session: &mut TileSession) -> Vec<AnswerTilePublish> {
    let screen_res = session.screen_res;
    let location = session.location.clone();
    let mut out: Vec<AnswerTilePublish> = session
        .drain_publish_tiles()
        .into_iter()
        .map(|tile| AnswerTilePublish {
            tile,
            screen_res,
            location: location.clone(),
        })
        .collect();
    for (loc, tile) in session.drain_lookahead_publishes() {
        out.push(AnswerTilePublish {
            tile: *tile,
            screen_res,
            location: loc,
        });
    }
    out
}

fn flush_unsent_tiles<A: SteadyActor>(
    actor: &mut A,
    tiles_out: &mut Tx<AnswerTilePublish>,
    bypass_out: &mut Tx<GpuTileHandle>,
    state: &mut WorkerState,
) {
    let prefer_bypass = state
        .tile_session
        .as_ref()
        .map(|s| s.worker_is_gpu_preferred() && s.worker_gpu_device_held())
        .unwrap_or(false);
    let production = crate::assemblies::workgroup::production_atlas::ProductionAtlas::shared();
    let use_bypass = prefer_bypass && production.is_some();

    let mut blocked = Vec::new();
    let mut sent_uploader = 0u32;
    let mut sent_bypass = 0u32;

    // GPU-resident whole tiles: slot already on production atlas (D-PUB-4).
    if production.is_some() {
        if let Some(session) = state.tile_session.as_mut() {
            let mut requeue = Vec::new();
            for handle in session.drain_gpu_bypass_handles() {
                match actor.try_send(bypass_out, handle) {
                    SendOutcome::Success => sent_bypass += 1,
                    SendOutcome::Blocked(h)
                    | SendOutcome::Timeout(h)
                    | SendOutcome::Closed(h) => requeue.push(h),
                }
            }
            for h in requeue {
                session.requeue_gpu_bypass(h);
            }
        }
    }

    let pending = std::mem::take(&mut state.unsent_tiles);
    let mut iter = pending.into_iter();
    while let Some(msg) = iter.next() {
        if use_bypass {
            let gpu_tile = GPUTile::from_answer_tile(
                &msg.tile,
                msg.screen_res,
                msg.location.clone(),
            );
            let handle = crate::assemblies::gpu_uploader::place_on_production_atlas(
                &production,
                gpu_tile,
            );
            match actor.try_send(bypass_out, handle) {
                SendOutcome::Success => {
                    sent_bypass += 1;
                    continue;
                }
                SendOutcome::Blocked(h)
                | SendOutcome::Timeout(h)
                | SendOutcome::Closed(h) => {
                    if let (Some(slot), Some(atlas)) = (h.production_slot, production.as_ref()) {
                        if let Ok(mut atlas) = atlas.lock() {
                            atlas.release(slot);
                        }
                    }
                }
            }
        }
        match actor.try_send(tiles_out, msg) {
            SendOutcome::Success => {
                sent_uploader += 1;
            }
            SendOutcome::Blocked(msg)
            | SendOutcome::Timeout(msg)
            | SendOutcome::Closed(msg) => {
                blocked.push(msg);
                blocked.extend(iter);
                break;
            }
        }
    }
    // #region agent log
    let n = crate::assemblies::workgroup::debug_session::pub_tick();
    if crate::assemblies::workgroup::debug_session::should_sample(n)
        || !blocked.is_empty()
        || sent_bypass > 0
    {
        let route = if sent_bypass > 0 && sent_uploader == 0 {
            "bypass_publisher"
        } else if sent_bypass > 0 {
            "mixed_bypass_and_uploader"
        } else {
            "gpu_uploader"
        };
        crate::assemblies::workgroup::debug_session::log(
            "H-BYPASS",
            "tile_worker/mod.rs:flush",
            "publish_route",
            &format!(
                "{{\"n\":{n},\"route\":\"{route}\",\"prefer_bypass\":{use_bypass},\"sent_bypass\":{sent_bypass},\"sent_uploader\":{sent_uploader},\"blocked\":{},\"mem_limit\":{}}}",
                blocked.len(),
                state.memory_limit_bytes
            ),
        );
    }
    // #endregion
    state.unsent_tiles = blocked;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intexp::IntExp;

    fn retarget(zoom: i32, res: (u32, u32), vel: f64) -> SchedulerToWorker {
        SchedulerToWorker::Retarget {
            frame_info: (
                ObjectivePosAndZoom {
                    pos: (IntExp::from(zoom), IntExp::ZERO),
                    zoom_pot: zoom,
                },
                res,
                vel,
            ),
        }
    }

    // r[verify cz.play.latest-wins+1]
    // r[verify cz.play.actor-drain+1]
    #[test]
    fn coalesce_keeps_only_latest_retarget_and_attention() {
        let msgs = vec![
            SchedulerToWorker::SetAttention(1, 1),
            retarget(0, (64, 64), 1.0),
            SchedulerToWorker::SetAttention(9, 9),
            retarget(8, (800, 480), 2.0),
        ];
        let c = coalesce_scheduler_commands(msgs);
        assert_eq!(c.attention, Some((9, 9)));
        let (loc, res, vel) = c.retarget.expect("retarget");
        assert_eq!(loc.zoom_pot, 8);
        assert_eq!(res, (800, 480));
        assert_eq!(vel, 2.0);
    }

    // r[verify cz.play.actor-drain+1]
    #[test]
    fn coalesce_empty_is_noop() {
        let c = coalesce_scheduler_commands(Vec::<SchedulerToWorker>::new());
        assert!(c.attention.is_none());
        assert!(c.retarget.is_none());
    }

    // r[verify cz.play.actor-drain+1]
    #[test]
    fn coalesce_drains_full_burst_before_coalesce() {
        // All five attentions are consumed; only the last remains.
        let msgs = (0..5).map(|i| SchedulerToWorker::SetAttention(i, i));
        let c = coalesce_scheduler_commands(msgs);
        assert_eq!(c.attention, Some((4, 4)));
        assert!(c.retarget.is_none());
    }

    // r[verify cz.play.latest-wins+1]
    #[test]
    fn coalesce_latest_attention_overwrites_earlier() {
        let msgs = vec![
            SchedulerToWorker::SetAttention(0, 0),
            SchedulerToWorker::SetAttention(3, 7),
        ];
        let c = coalesce_scheduler_commands(msgs);
        assert_eq!(c.attention, Some((3, 7)));
    }

    // r[verify cz.play.latest-wins+1]
    #[test]
    fn coalesce_latest_retarget_overwrites_earlier() {
        let msgs = vec![retarget(1, (64, 64), 0.0), retarget(12, (128, 128), 9.0)];
        let c = coalesce_scheduler_commands(msgs);
        let (loc, res, vel) = c.retarget.expect("retarget");
        assert_eq!(loc.zoom_pot, 12);
        assert_eq!(res, (128, 128));
        assert_eq!(vel, 9.0);
    }

    // r[verify cz.play.actor-poll+1]
    #[test]
    fn play_input_poll_is_one_millisecond() {
        assert_eq!(PLAY_INPUT_POLL_MS, 1);
    }

    // r[verify cz.play.actor-poll+1]
    #[test]
    fn play_input_poll_is_faster_than_frame_budget() {
        // Must re-check inputs well under a 17ms frame so backlog cannot grow.
        assert!(PLAY_INPUT_POLL_MS < 17);
    }

    // r[verify cz.play.actor-poll+1]
    #[test]
    fn play_input_poll_is_non_zero() {
        assert!(PLAY_INPUT_POLL_MS >= 1);
    }

    // r[verify cz.perf.play-minimize+1]
    // r[verify cz.perf.play-8bump-100ms+1]
    #[test]
    fn play_budget_bursts_while_screen_seats_empty() {
        assert_eq!(workshift_budget_ms_for_session(true, 1, 0), 8);
        assert_eq!(workshift_budget_ms_for_session(false, 1, 0), 8);
        assert_eq!(workshift_budget_ms_for_session(false, 1, 12), 1);
        assert_eq!(workshift_budget_ms_for_session(false, 0, 0), 16);
    }

    // r[verify cz.perf.play-minimize+1]
    #[test]
    fn play_budget_after_retarget_beats_one_ms_stall() {
        assert!(
            workshift_budget_ms_for_session(true, 1, 0) > 1,
            "headed retarget must not stay on 1ms while seats_done==0"
        );
    }

    fn dummy_worker_state(tile_session: Option<TileSession>) -> WorkerState {
        WorkerState {
            tile_session,
            total_workshifts: 0,
            unsent_tiles: Vec::new(),
            full_republish_done: false,
            incomplete: true,
            memory_limit_bytes: 1_000_000_000,
        }
    }

    // r[verify cz.play.actor-poll+1]
    #[test]
    fn session_restore_puts_back_session_after_panic() {
        let loc = ObjectivePosAndZoom {
            pos: (IntExp::ZERO, IntExp::ZERO),
            zoom_pot: 0,
        };
        let session = TileSession::new(loc, (64, 64));
        let mut state = dummy_worker_state(Some(session));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut restore = SessionRestore::new(&mut state);
            assert!(state.tile_session.is_none());
            let _session = restore.slot.as_mut().expect("session");
            panic!("simulated workshift panic");
        }));
        assert!(result.is_err());
        assert!(
            state.tile_session.is_some(),
            "SteadyState must not persist tile_session=None after actor regeneration"
        );
    }

    // r[verify cz.play.actor-poll+1]
    #[test]
    fn session_restore_noop_when_slot_empty() {
        let mut state = dummy_worker_state(None);
        {
            let _restore = SessionRestore::new(&mut state);
            assert!(state.tile_session.is_none());
        }
        assert!(state.tile_session.is_none());
    }
}
