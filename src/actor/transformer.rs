use steady_state::*;

use crate::actor::window::*;
use crate::actor::computer::*;


/// State struct for the worker actor.
/// Tracks the number of heartbeats, values processed, messages sent, and the batch size.
/// The batch_size is set to half the channel capacity for double-buffering.
pub(crate) struct RendererState {
    pub(crate) viewport_position_real: String,
    pub(crate) viewport_position_imag: String,
    pub(crate) viewport_zoom: String
}

pub(crate) struct Screen {
    real_center: String
    , imag_center: String
    , zoom: String
    , screen_height: u32
    , screen_width: u32
}

pub(crate) enum JobType {
    Mandelbrot
    , TrackPoint
    , Julia
}

// all jobs should be either completed or cancelled or time out
// The worker will run at a worker clock speed of 20tps or 50mspt
// that means it will split jobs to just under that size so it can be responsive

pub(crate) enum ZoomerJob {
    StartJob {
        job_type: JobType
        , job_id: u64
        , screen: Screen
        , minus_screens: Vec<Screen>
        , timeout: Duration
    }
    , CancelJob {
        job_id: u64
    }
}

pub(crate) struct ZoomerReport {
    time_xyz: Vec<(String, Instant)>
}
pub(crate) struct ScreenPixels {
    pixels: Vec<u32>
    , command_uuid: Option<u64>
    , report: Option<ZoomerReport>
}

pub async fn run(
    actor: SteadyActorShadow,
    commands_in: SteadyRx<ZoomerCommandPackage>,
    pixels_in: SteadyRx<PixelGroup>,
    pixels_out: SteadyTx<ScreenPixels>,
    jobs_out: SteadyTx<Vec<ZoomerJob>>,
    state: SteadyState<RendererState>,
) -> Result<(), Box<dyn Error>> {
    internal_behavior(
        actor.into_spotlight([&commands_in, &pixels_in], [&pixels_out, &jobs_out]),
        commands_in,
        pixels_in,
        pixels_out,
        jobs_out,
        state
    )
        .await
}

async fn internal_behavior<A: SteadyActor>(
    mut actor: A,
    commands_in: SteadyRx<ZoomerCommandPackage>,
    pixels_in: SteadyRx<PixelGroup>,
    pixels_out: SteadyTx<ScreenPixels>,
    jobs_out: SteadyTx<Vec<ZoomerJob>>,
    state: SteadyState<RendererState>,
) -> Result<(), Box<dyn Error>> {

    // Lock all channels for exclusive access within this actor.

    let mut commands_in = commands_in.lock().await;
    let mut pixels_in = pixels_in.lock().await;
    let mut pixels_out = pixels_out.lock().await;
    let mut jobs_out = jobs_out.lock().await;


    // Initialize the actor's state, setting batch_size to half the generator channel's capacity.
    // This ensures that the producer can fill one half while the consumer processes the other.
    let mut state = state.lock(|| RendererState {
        viewport_position_real: String::from("0"),
        viewport_position_imag: String::from("0"),
        viewport_zoom: String::from("1")
    }).await;


    let max_actor_wait = Duration::from_millis(40);

    // Main processing loop.
    // The actor runs until all input channels are closed and empty, and the output channel is closed.
    while actor.is_running(
        || i!(pixels_in.is_closed_and_empty())
            && i!(pixels_out.mark_closed())
    ) {
        // Wait for a periodic timer (clock speed of actor)
        await_for_all_or_proceed_upon!(
            actor.wait_periodic(max_actor_wait),
            actor.wait_avail(&mut commands_in, 1),
            actor.wait_avail(&mut pixels_in, 1)
        );

        // do transformer stuff
        if actor.avail_units(&mut commands_in) > 0 {
            let commands;
            match actor.try_take(&mut commands_in) {
                Option::Some(c) => {commands = c}
                , Option::None => {panic!("something is very wrong...")}
            }
            debug_assert!(commands.commands.len() < NUMBER_OF_COMMANDS as usize, "too many commands in a package!");

            // do more transformer stuff
        }

        // do patcher stuff
        if actor.avail_units(&mut pixels_in) > 0 {
            let pixels;
            match actor.try_take(&mut pixels_in) {
                Option::Some(p) => {pixels = p}
                , Option::None => {panic!("something is very wrong...")}
            }

            // do more patcher stuff
        }
    }

    // Final shutdown log, reporting all statistics.
    info!("Transformer shutting down");
    Ok(())
}