// read delivery.md for project context
//! Live-path bridge: mutating outfill ops run on the intratile actor thread via
//! sync RPC when the tile-worker SoloAct has installed a client. Immutable
//! queries stay local (state still lives in TileSession between RPCs).

use std::cell::RefCell;

use crate::assemblies::workgroup::structs::CalibratedAnswer;
use crate::assemblies::workgroup::actor_messages::{
    IntratileClient, IntratileReply, IntratileRequest, BATCH_N,
};
use crate::assemblies::workgroup::workcore::mandelbrot::scheduler_implementations::outfill_infill_scheduler::{
    OutfillInfillScheduler, OutfillInfillSchedulerState,
};

thread_local! {
    static CLIENT: RefCell<Option<IntratileClient>> = const { RefCell::new(None) };
}

pub fn install(client: IntratileClient) {
    CLIENT.with(|c| *c.borrow_mut() = Some(client));
}

pub fn clear() {
    CLIENT.with(|c| *c.borrow_mut() = None);
}

fn with_client<R>(f: impl FnOnce(&IntratileClient) -> R) -> Option<R> {
    CLIENT.with(|c| c.borrow().as_ref().map(f))
}

fn placeholder_state() -> OutfillInfillSchedulerState {
    OutfillInfillScheduler::init_for_tile_extent((1, 1))
}

pub fn get_next_n_seats(
    scheduler_state: &mut OutfillInfillSchedulerState,
) -> Option<[Option<((usize, usize), Option<CalibratedAnswer>)>; BATCH_N]> {
    with_client(|client| {
        // #region agent log
        // Spec: worker should spiral locally when lacking intratile guidance.
        // Live path always RPCs when client is installed — this is the bind.
        // #endregion
        let state = std::mem::replace(scheduler_state, placeholder_state());
        match client.rpc(IntratileRequest::GetNextSeats { state }) {
            IntratileReply::Seats { state, seats } => {
                *scheduler_state = state;
                seats
            }
            other => panic!("intratile GetNextSeats unexpected reply"),
        }
    })
}

pub fn update(
    scheduler_state: &mut OutfillInfillSchedulerState,
    updates: &[Option<((usize, usize), CalibratedAnswer)>; BATCH_N],
) -> bool {
    with_client(|client| {
        let state = std::mem::replace(scheduler_state, placeholder_state());
        match client.rpc(IntratileRequest::Update {
            state,
            updates: *updates,
        }) {
            IntratileReply::State { state } => {
                *scheduler_state = state;
            }
            other => panic!("intratile Update unexpected reply"),
        }
    })
    .is_some()
}

pub fn reclaim_orphaned_active(scheduler_state: &mut OutfillInfillSchedulerState) -> Option<bool> {
    with_client(|client| {
        let state = std::mem::replace(scheduler_state, placeholder_state());
        match client.rpc(IntratileRequest::ReclaimOrphaned { state }) {
            IntratileReply::Flag { state, value } => {
                *scheduler_state = state;
                value
            }
            other => panic!("intratile Reclaim unexpected reply"),
        }
    })
}

pub fn force_progress(scheduler_state: &mut OutfillInfillSchedulerState) -> Option<bool> {
    with_client(|client| {
        let state = std::mem::replace(scheduler_state, placeholder_state());
        match client.rpc(IntratileRequest::ForceProgress { state }) {
            IntratileReply::Flag { state, value } => {
                *scheduler_state = state;
                value
            }
            other => panic!("intratile ForceProgress unexpected reply"),
        }
    })
}

pub fn apply_period_resolved(
    scheduler_state: &mut OutfillInfillSchedulerState,
    local: (usize, usize),
    period: u32,
) -> bool {
    with_client(|client| {
        let state = std::mem::replace(scheduler_state, placeholder_state());
        match client.rpc(IntratileRequest::ApplyPeriodResolved {
            state,
            local,
            period,
        }) {
            IntratileReply::State { state } => {
                *scheduler_state = state;
            }
            other => panic!("intratile ApplyPeriodResolved unexpected reply"),
        }
    })
    .is_some()
}

pub fn mark_period_resolve_done(scheduler_state: &mut OutfillInfillSchedulerState) -> bool {
    with_client(|client| {
        let state = std::mem::replace(scheduler_state, placeholder_state());
        match client.rpc(IntratileRequest::MarkPeriodDone { state }) {
            IntratileReply::State { state } => {
                *scheduler_state = state;
            }
            other => panic!("intratile MarkPeriodDone unexpected reply"),
        }
    })
    .is_some()
}

pub fn init_for_tile_extent_screen(
    extent: (usize, usize),
    touches_screen_border: bool,
    known: Vec<((usize, usize), CalibratedAnswer)>,
) -> Option<OutfillInfillSchedulerState> {
    with_client(|client| {
        match client.rpc(IntratileRequest::InitForTile {
            extent,
            screen_edge_touches: [touches_screen_border; 4],
            known,
        }) {
            IntratileReply::Init { state } => state,
            other => panic!("intratile Init unexpected reply"),
        }
    })
}

pub fn absorb_known(
    scheduler_state: &mut OutfillInfillSchedulerState,
    local: (usize, usize),
    answer: CalibratedAnswer,
) -> bool {
    with_client(|client| {
        let state = std::mem::replace(scheduler_state, placeholder_state());
        match client.rpc(IntratileRequest::AbsorbKnown {
            state,
            local,
            answer,
        }) {
            IntratileReply::State { state } => {
                *scheduler_state = state;
            }
            other => panic!("intratile AbsorbKnown unexpected reply"),
        }
    })
    .is_some()
}

pub fn reseed_after_absorb(scheduler_state: &mut OutfillInfillSchedulerState) -> bool {
    with_client(|client| {
        let state = std::mem::replace(scheduler_state, placeholder_state());
        match client.rpc(IntratileRequest::ReseedAfterAbsorb { state }) {
            IntratileReply::State { state } => {
                *scheduler_state = state;
            }
            other => panic!("intratile Reseed unexpected reply"),
        }
    })
    .is_some()
}
