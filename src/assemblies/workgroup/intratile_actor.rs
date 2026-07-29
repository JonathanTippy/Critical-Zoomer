//! Intratile scheduler SteadyState actor (auth workgroup sub-actor).
//! Owns outfill/infill phase machines; talks to the tile worker via SS channels
//! (graph) and a sync RPC inbox (hot path from TileSession on the worker thread).

use steady_state::*;

use crate::assemblies::structs::Tile;
use crate::assemblies::workgroup::actor_messages::{
    IntratileReply, IntratileRequest, BATCH_N,
};
use crate::assemblies::workgroup::workcore::mandelbrot::scheduler_implementations::outfill_infill_scheduler::*;

pub struct IntratileActorState;

pub async fn run(
    actor: SteadyActorShadow,
    from_worker: SteadyRx<IntratileRequest>,
    to_worker: SteadyTx<IntratileReply>,
    rpc_rx: std::sync::mpsc::Receiver<(
        IntratileRequest,
        std::sync::mpsc::SyncSender<IntratileReply>,
    )>,
    state: SteadyState<IntratileActorState>,
) -> Result<(), Box<dyn Error>> {
    internal_behavior(
        actor.into_spotlight([&from_worker], [&to_worker]),
        from_worker,
        to_worker,
        rpc_rx,
        state,
    )
    .await
}

async fn internal_behavior<A: SteadyActor>(
    mut actor: A,
    from_worker: SteadyRx<IntratileRequest>,
    to_worker: SteadyTx<IntratileReply>,
    rpc_rx: std::sync::mpsc::Receiver<(
        IntratileRequest,
        std::sync::mpsc::SyncSender<IntratileReply>,
    )>,
    state: SteadyState<IntratileActorState>,
) -> Result<(), Box<dyn Error>> {
    let mut from_worker = from_worker.lock().await;
    let mut to_worker = to_worker.lock().await;
    let _state = state.lock(|| IntratileActorState).await;

    // Always re-check SS inputs at a quick pace (RPC path drains fully below).
    let max_sleep = Duration::from_millis(1);
    // #region agent log
    let mut rpc_batch = 0u64;
    let mut ss_msgs = 0u64;
    let mut idle_timeouts = 0u64;
    // #endregion

    while actor.is_running(|| i!(to_worker.mark_closed())) {
        // Block on sync RPC so TileSession hot-path round-trips stay low-latency.
        match rpc_rx.recv_timeout(max_sleep) {
            Ok((req, reply_tx)) => {
                // #region agent log
                rpc_batch += 1;
                let t0 = std::time::Instant::now();
                // #endregion
                let _ = reply_tx.send(process(req));
                let mut drained = 1u32;
                while let Ok((req, reply_tx)) = rpc_rx.try_recv() {
                    let _ = reply_tx.send(process(req));
                    drained += 1;
                }
                // #region agent log
                let n = crate::assemblies::workgroup::debug_session::its_wake_tick();
                if crate::assemblies::workgroup::debug_session::should_sample(n) {
                    let ms = t0.elapsed().as_secs_f64() * 1000.0;
                    crate::assemblies::workgroup::debug_session::log(
                        "H-ITS-BIND",
                        "intratile_actor.rs:loop",
                        "rpc_batch_processed",
                        &format!(
                            "{{\"wake\":{n},\"rpc_batches\":{rpc_batch},\"drained\":{drained},\"batch_ms\":{ms:.3},\"ss_msgs\":{ss_msgs},\"idle_timeouts\":{idle_timeouts}}}"
                        ),
                    );
                }
                // #endregion
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // #region agent log
                idle_timeouts += 1;
                let n = crate::assemblies::workgroup::debug_session::its_wake_tick();
                if crate::assemblies::workgroup::debug_session::should_sample(n) {
                    crate::assemblies::workgroup::debug_session::log(
                        "H-ITS-SPIN",
                        "intratile_actor.rs:loop",
                        "idle_timeout_wake",
                        &format!(
                            "{{\"wake\":{n},\"idle_timeouts\":{idle_timeouts},\"rpc_batches\":{rpc_batch},\"ss_msgs\":{ss_msgs}}}"
                        ),
                    );
                }
                // #endregion
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }

        // SteadyState edge: seat/phase traffic for the live graph.
        while actor.avail_units(&mut from_worker) > 0 {
            let Some(req) = actor.try_take(&mut from_worker) else {
                break;
            };
            // #region agent log
            ss_msgs += 1;
            // #endregion
            let reply = process(req);
            match actor.try_send(&mut to_worker, reply) {
                SendOutcome::Success => {}
                SendOutcome::Blocked(_)
                | SendOutcome::Timeout(_)
                | SendOutcome::Closed(_) => {}
            }
        }
    }
    info!("Intratile scheduler shutting down.");
    Ok(())
}

fn process(req: IntratileRequest) -> IntratileReply {
    match req {
        IntratileRequest::InitForTile {
            extent,
            screen_edge_touches,
            known,
        } => {
            let mut state = OutfillInfillScheduler::init_for_tile_extent_screen(
                extent,
                screen_edge_touches.iter().any(|t| *t),
            );
            let mut tile: Tile<()> = Tile::new((0, 0), 0);
            for (local, answer) in known {
                OutfillInfillScheduler::absorb_known(&mut state, local, answer);
                let _ = &mut tile;
            }
            OutfillInfillScheduler::reseed_after_absorb(&mut state);
            IntratileReply::Init { state }
        }
        IntratileRequest::GetNextSeats { mut state } => {
            let mut tile: Tile<()> = Tile::new((0, 0), 0);
            let seats = OutfillInfillScheduler::get_next_n_seats::<BATCH_N>(&mut state, &mut tile);
            IntratileReply::Seats { state, seats }
        }
        IntratileRequest::Update { mut state, updates } => {
            let mut tile: Tile<()> = Tile::new((0, 0), 0);
            OutfillInfillScheduler::update(&mut state, &mut tile, &updates);
            IntratileReply::State { state }
        }
        IntratileRequest::ReclaimOrphaned { mut state } => {
            let value = OutfillInfillScheduler::reclaim_orphaned_active(&mut state);
            IntratileReply::Flag { state, value }
        }
        IntratileRequest::ForceProgress { mut state } => {
            let value = OutfillInfillScheduler::force_progress(&mut state);
            IntratileReply::Flag { state, value }
        }
        IntratileRequest::NeedsPeriodResolve { state } => {
            let value = OutfillInfillScheduler::needs_period_resolve(&state);
            IntratileReply::Flag { state, value }
        }
        IntratileRequest::TakePeriodLocals { state } => {
            let locals = OutfillInfillScheduler::take_period_resolve_locals(&state);
            IntratileReply::PeriodLocals { state, locals }
        }
        IntratileRequest::ApplyPeriodResolved {
            mut state,
            local,
            period,
        } => {
            OutfillInfillScheduler::apply_period_resolved(&mut state, local, period);
            IntratileReply::State { state }
        }
        IntratileRequest::MarkPeriodDone { mut state } => {
            OutfillInfillScheduler::mark_period_resolve_done(&mut state);
            IntratileReply::State { state }
        }
        IntratileRequest::HasWork { state } => {
            let value = OutfillInfillScheduler::has_work(&state);
            IntratileReply::Flag { state, value }
        }
        IntratileRequest::GraphPulse => IntratileReply::Ack,
        IntratileRequest::ReseedAfterAbsorb { mut state } => {
            OutfillInfillScheduler::reseed_after_absorb(&mut state);
            IntratileReply::State { state }
        }
        IntratileRequest::AbsorbKnown {
            mut state,
            local,
            answer,
        } => {
            OutfillInfillScheduler::absorb_known(&mut state, local, answer);
            IntratileReply::State { state }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_rpc_bind_smoke_writes_debug_log() {
        // Post-fix: outfill must stay local even if a client is installed.
        // Prefers GPU path; should not flood IntratileClient::rpc.
        let handle = std::thread::Builder::new()
            .stack_size(32 * 1024 * 1024)
            .spawn(|| {
                use crate::assemblies::workgroup::actor_messages::{
                    IntratileClient, IntratileReply, IntratileRequest,
                };
                use crate::assemblies::workgroup::live_intratile;
                use crate::assemblies::workgroup::tile_session::TileSession;
                use crate::intexp::IntExp;
                use crate::utils::ObjectivePosAndZoom;

                let (rpc_tx, rpc_rx) = std::sync::mpsc::sync_channel::<(
                    IntratileRequest,
                    std::sync::mpsc::SyncSender<IntratileReply>,
                )>(64);
                let server = std::thread::Builder::new()
                    .stack_size(32 * 1024 * 1024)
                    .name("its-rpc-smoke".into())
                    .spawn(move || {
                        while let Ok((req, reply_tx)) = rpc_rx.recv() {
                            let _ = reply_tx.send(process(req));
                        }
                    })
                    .expect("its server");

                let client = IntratileClient::new(rpc_tx);
                live_intratile::install(client);
                let loc = ObjectivePosAndZoom {
                    pos: (IntExp::from(-1), IntExp::ZERO),
                    zoom_pot: 0,
                };
                let mut session = TileSession::new(loc, (160, 96));
                for _ in 0..24 {
                    session.workshift();
                }
                let deeper = ObjectivePosAndZoom {
                    pos: (IntExp::from(-1), IntExp::ZERO),
                    zoom_pot: 3,
                };
                session.retarget(deeper, (160, 96));
                for _ in 0..24 {
                    session.workshift();
                }
                live_intratile::clear();
                drop(session);
                let _ = server.join();
            })
            .expect("spawn smoke");
        handle.join().expect("live rpc smoke panicked");
    }
}
