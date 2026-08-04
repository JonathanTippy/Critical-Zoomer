// read delivery.md for project context
//! SteadyState + sync-RPC payloads for the authoritative workgroup actor graph.

use crate::assemblies::workgroup::structs::CalibratedAnswer;
use crate::assemblies::workgroup::workcore::mandelbrot::scheduler_implementations::outfill_infill_scheduler::OutfillInfillSchedulerState;
use crate::assemblies::workgroup::workcore::mandelbrot::OrbitId;
use crate::constants::GPU_WORKER_BATCH_N;
use crate::utils::ObjectivePosAndZoom;

pub const BATCH_N: usize = GPU_WORKER_BATCH_N;

/// Scheduler → tile worker.
#[derive(Clone, Debug)]
pub enum SchedulerToWorker {
    Retarget {
        frame_info: (ObjectivePosAndZoom, (u32, u32), f64),
    },
    SetAttention(i32, i32),
}

/// Reference worker → tile worker.
#[derive(Clone, Copy, Debug)]
pub struct ReferenceDelivery {
    pub bound_orbit_id: OrbitId,
    pub bound_mag: i32,
}

/// Tile worker ⇄ intratile scheduler. State shuttles with each request.
pub enum IntratileRequest {
    InitForTile {
        extent: (usize, usize),
        screen_edge_touches: [bool; 4],
        known: Vec<((usize, usize), CalibratedAnswer)>,
    },
    GetNextSeats {
        state: OutfillInfillSchedulerState,
    },
    Update {
        state: OutfillInfillSchedulerState,
        updates: [Option<((usize, usize), CalibratedAnswer)>; BATCH_N],
    },
    ReclaimOrphaned {
        state: OutfillInfillSchedulerState,
    },
    ForceProgress {
        state: OutfillInfillSchedulerState,
    },
    NeedsPeriodResolve {
        state: OutfillInfillSchedulerState,
    },
    TakePeriodLocals {
        state: OutfillInfillSchedulerState,
    },
    ApplyPeriodResolved {
        state: OutfillInfillSchedulerState,
        local: (usize, usize),
        period: u32,
    },
    MarkPeriodDone {
        state: OutfillInfillSchedulerState,
    },
    HasWork {
        state: OutfillInfillSchedulerState,
    },
    /// Lightweight SteadyState edge pulse (no outfill state).
    GraphPulse,
    ReseedAfterAbsorb {
        state: OutfillInfillSchedulerState,
    },
    AbsorbKnown {
        state: OutfillInfillSchedulerState,
        local: (usize, usize),
        answer: CalibratedAnswer,
    },
}

pub enum IntratileReply {
    Init {
        state: OutfillInfillSchedulerState,
    },
    Seats {
        state: OutfillInfillSchedulerState,
        seats: [Option<((usize, usize), Option<CalibratedAnswer>)>; BATCH_N],
    },
    State {
        state: OutfillInfillSchedulerState,
    },
    Flag {
        state: OutfillInfillSchedulerState,
        value: bool,
    },
    PeriodLocals {
        state: OutfillInfillSchedulerState,
        locals: Vec<(usize, usize)>,
    },
    Ack,
}

/// Blocking RPC client used from TileSession on the tile-worker SoloAct thread.
#[derive(Clone)]
pub struct IntratileClient {
    tx: std::sync::mpsc::SyncSender<(
        IntratileRequest,
        std::sync::mpsc::SyncSender<IntratileReply>,
    )>,
}

impl IntratileClient {
    pub fn new(
        tx: std::sync::mpsc::SyncSender<(
            IntratileRequest,
            std::sync::mpsc::SyncSender<IntratileReply>,
        )>,
    ) -> Self {
        IntratileClient { tx }
    }

    pub fn rpc(&self, req: IntratileRequest) -> IntratileReply {
        // #region agent log
        let n = crate::assemblies::workgroup::debug_session::rpc_tick();
        let kind = match &req {
            IntratileRequest::InitForTile { .. } => "InitForTile",
            IntratileRequest::GetNextSeats { .. } => "GetNextSeats",
            IntratileRequest::Update { .. } => "Update",
            IntratileRequest::ReclaimOrphaned { .. } => "ReclaimOrphaned",
            IntratileRequest::ForceProgress { .. } => "ForceProgress",
            IntratileRequest::NeedsPeriodResolve { .. } => "NeedsPeriodResolve",
            IntratileRequest::TakePeriodLocals { .. } => "TakePeriodLocals",
            IntratileRequest::ApplyPeriodResolved { .. } => "ApplyPeriodResolved",
            IntratileRequest::MarkPeriodDone { .. } => "MarkPeriodDone",
            IntratileRequest::HasWork { .. } => "HasWork",
            IntratileRequest::GraphPulse => "GraphPulse",
            IntratileRequest::ReseedAfterAbsorb { .. } => "ReseedAfterAbsorb",
            IntratileRequest::AbsorbKnown { .. } => "AbsorbKnown",
        };
        let t0 = std::time::Instant::now();
        // #endregion
        let (reply_tx, reply_rx) = std::sync::mpsc::sync_channel(1);
        self.tx
            .send((req, reply_tx))
            .expect("intratile scheduler actor alive");
        let reply = reply_rx.recv().expect("intratile scheduler reply");
        // #region agent log
        if crate::assemblies::workgroup::debug_session::should_sample(n) {
            let ms = t0.elapsed().as_secs_f64() * 1000.0;
            crate::assemblies::workgroup::debug_session::log(
                "H-ITS-BIND",
                "actor_messages.rs:rpc",
                "intratile_sync_rpc",
                &format!(
                    "{{\"n\":{n},\"kind\":\"{kind}\",\"elapsed_ms\":{ms:.3},\"via\":\"sync_rpc_not_ss\"}}"
                ),
            );
        }
        // #endregion
        reply
    }
}
