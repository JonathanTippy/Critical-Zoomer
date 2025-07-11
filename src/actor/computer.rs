use steady_state::*;

use crate::actor::window::*;
use crate::actor::transformer::*;



pub(crate) struct ComputerState {
}

pub(crate) struct PixelGroup {
    pub(crate) pixels: Vec<u32>
    , pub(crate) job_uuid: Option<u64>
    , pub(crate) report: Option<ZoomerReport>
}


pub async fn run(
    actor: SteadyActorShadow,
    jobs_in: SteadyRx<Vec<ZoomerJob>>,
    pixels_out: SteadyTx<PixelGroup>,
    state: SteadyState<ComputerState>,
) -> Result<(), Box<dyn Error>> {
    // The worker is tested by its simulated neighbors, so we always use internal_behavior.
    internal_behavior(
        actor.into_spotlight([&jobs_in], [&pixels_out]),
        jobs_in,
        pixels_out,
        state,
    )
        .await
}

/// The core logic for the worker actor.
/// This function implements high-throughput, cache-friendly batch processing.
///
/// Key performance strategies:      //#!#//
/// - **Double-buffering**: The channel is logically split into two halves. While one half is being filled by the producer, the consumer processes the other half.
/// - **Full-channel consumption**: The worker processes both halves (two slices) before yielding, maximizing cache line reuse and minimizing context switches.
/// - **Pre-allocated buffers**: All batch buffers are allocated once and reused, ensuring zero-allocation hot paths.
/// - **Mechanically sympathetic**: The design aligns with CPU cache and memory bus behavior for optimal throughput.
async fn internal_behavior<A: SteadyActor>(
    mut actor: A,
    jobs_in: SteadyRx<Vec<ZoomerJob>>,
    pixels_out: SteadyTx<PixelGroup>,
    state: SteadyState<ComputerState>,
) -> Result<(), Box<dyn Error>> {
    let mut jobs_in = jobs_in.lock().await;
    let mut pixels_out = pixels_out.lock().await;

    let mut state = state.lock(|| ComputerState {
    }).await;

    // Lock all channels for exclusive access within this actor.

    let max_latency = Duration::from_millis(40);

    // Main processing loop.
    // The actor runs until all input channels are closed and empty, and the output channel is closed.
    while actor.is_running(
        || i!(pixels_out.mark_closed())
    ) {
        // Wait for all required conditions:
        // - A periodic timer
        await_for_any!(  //#!#//
            actor.wait_periodic(max_latency)
        );
    }

    // Final shutdown log, reporting all statistics.
    info!("Computer shutting down.");
    Ok(())
}