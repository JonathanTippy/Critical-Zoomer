use steady_state::*;
use arg::MainArg;
mod arg;


// The actor module contains all the actor implementations for this pipeline.
// Each actor is in its own submodule for clarity and separation of concerns.
pub(crate) mod actor {
    pub(crate) mod window;
    pub(crate) mod work_controller;
    pub(crate) mod screen_worker;
    pub(crate) mod colorer;
    pub(crate) mod work_collector;
    pub(crate) mod escaper;
}

pub(crate) mod action {
    pub(crate) mod sampling;
    pub(crate) mod settings;
    pub(crate) mod rolling;
    pub(crate) mod workshift;
    pub(crate) mod utils;
    pub(crate) mod widgetize;
    pub(crate) mod color;
    pub(crate) mod do_work;
    pub(crate) mod partial_knowledge;
    pub(crate) mod serialize;
    pub(crate) mod constants;
    pub(crate) mod collect;
}

use std::thread;

const STACK_SIZE:usize = 200 * 1024 * 1024; // 200 MiB
fn main() {


    let builder = thread::Builder::new()
        .name("worker-thread".into())
        .stack_size(STACK_SIZE);

    let handler = builder.spawn(|| {
        // Parse command-line arguments (rate, beats, etc.) using clap.
        let cli_args = MainArg::parse();

        // Initialize logging at Info level for runtime diagnostics and performance output.
        init_logging(LogLevel::Info);

        // Build the actor graph with all channels and actors, using the parsed arguments.
        let mut graph = GraphBuilder::default()
            .with_telemtry_production_rate_ms(200)
            .with_stack_size(STACK_SIZE)
            .build(cli_args);

        // Construct the full actor pipeline and channel topology.
        build_graph(&mut graph);

        // Start the entire actor system. All actors and channels are now live.
        graph.start();

        // The system runs until an actor requests shutdown or the timeout is reached.
        graph.block_until_stopped(Duration::from_secs(1));
    }).expect("Failed to spawn thread");

    handler.join().expect("Thread panicked");

}

// Actor names for use in graph construction and testing.

const NAME_WINDOW: &str = "window";
const NAME_SETTINGS_WINDOW: &str = "settings";
const NAME_COLORER: &str = "colorer";
const NAME_WORK_CONTROLLER: &str = "work controller";
const NAME_SCREEN_WORKER:&str = "screen worker";
const NAME_WORK_COLLECTOR: &str = "work collector";
const NAME_ESCAPER: &str = "point escaper";

fn build_graph(graph: &mut Graph) {
    // Channel builder is configured with advanced telemetry and alerting features.
    // - Red/orange alerts for congestion
    // - Percentile-based monitoring for channel fill levels
    // - Real-time average rate tracking
    let channel_builder = graph.channel_builder()
        // Smoother rates over a longer window
        .with_compute_refresh_window_floor(Duration::from_secs(4),Duration::from_secs(24))
        // Red alert if channel is >90% full on average (critical congestion)
        .with_filled_trigger(Trigger::AvgAbove(Filled::p90()), AlertColor::Red)
        // Orange alert if channel is >60% full on average (early warning)
        .with_filled_trigger(Trigger::AvgAbove(Filled::p60()), AlertColor::Orange)
        // Track average message rate for each channel
        .with_avg_rate()
        .with_capacity(2);

    // Channel capacities are set extremely large for high-throughput, batch-friendly operation.
    // - Heartbeat channel: moderate size for timing signals
    // - Generator and computer channels: 1,048,576 messages (1<<20) for massive batch processing


    let (
        colorer_tx_to_window
        , window_rx_from_colorer
    ) = channel_builder.with_capacity(2).build();



    //window to worker state update channel

    let (
        window_tx_to_work_controller
        , work_controller_rx_from_window
    ) = channel_builder.with_capacity(50).build();

    let (
        window_tx_to_worker
        , worker_rx_from_window
    ) = channel_builder.with_capacity(50).build();

    let (
        window_tx_to_stuff
        , stuff_rx_from_window
    ) = channel_builder.with_capacity(50).build_channel_bundle();

    //work controller to worker commands channel

    let (
        work_controller_tx_to_screen_worker
        , screen_worker_rx_from_work_controller
    ) = channel_builder.with_capacity(2).build();

    // worker to work collector responses channel

    let (
        screen_worker_tx_to_work_collector
        , work_collector_rx_from_screen_worker
    ) = channel_builder.with_capacity(50).build();

    // work collector to escaper chanel

    let (
        work_collector_tx_to_escaper
        , escaper_rx_from_work_collector
    ) = channel_builder.with_capacity(2).build();

    // escaper to colorer channel

    let (
        escaper_tx_to_colorer
        , colorer_rx_from_escaper
    ) = channel_builder.with_capacity(2).build();

    // The actor builder is configured to collect thread/core info and load metrics.
    // - with_thread_info: enables reporting of OS thread and CPU core (requires core_affinity feature in Cargo.toml)
    // - with_load_avg, with_mcpu_avg: enables real-time load and CPU usage metrics
    let actor_builder = graph.actor_builder()
        .with_thread_info()
        .with_mcpu_trigger(Trigger::AvgAbove(MCPU::m768()), AlertColor::Red)
        .with_mcpu_trigger(Trigger::AvgAbove(MCPU::m512()), AlertColor::Orange)
        .with_mcpu_trigger(Trigger::AvgAbove(MCPU::m256()), AlertColor::Yellow)
        .with_load_avg()
        .with_mcpu_avg();

    // NOTE: The core_affinity and display features in Cargo.toml ensure that actors remain on their assigned CPU core.
    // This is critical for cache locality and consistent performance. Without core_affinity, actors could move between cores,
    // but would still not move between threads (each actor or team is always bound to a thread).

    // Actor grouping: Troupe (team) vs SoloAct
    // - MemberOf(&mut team): actors are grouped to share a single thread, cooperatively yielding to each other.
    //   This is optimal for lightweight actors or those that coordinate closely (e.g., generator and heartbeat).
    // - SoloAct: actor runs on its own dedicated thread, ideal for CPU-intensive or batch-heavy actors (e.g., computer, logger).

    //let mut responsive_team = graph.actor_troupe();

    let (colorer_settings, escaper_settings) = (stuff_rx_from_window[0].clone(), stuff_rx_from_window[1].clone());

    let state = new_state();
    actor_builder.with_name(NAME_WINDOW)
        .build(move |context|
            actor::window::run(context, window_rx_from_colorer.clone(), window_tx_to_work_controller.clone(), window_tx_to_stuff.clone(), window_tx_to_worker.clone(), state.clone()) //#!#//
               //, MemberOf(&mut responsive_team));
               , SoloAct);

    let state = new_state();
    actor_builder.with_name(NAME_COLORER)
        .build(move |context|
                   actor::colorer::run(context, colorer_rx_from_escaper.clone(), colorer_settings.clone(), colorer_tx_to_window.clone(), state.clone()) //#!#//
               //, MemberOf(&mut responsive_team));
               , SoloAct);

    let state = new_state();
    actor_builder.with_name(NAME_WORK_CONTROLLER)
        .build(move |context|
                   actor::work_controller::run(context, work_controller_rx_from_window.clone(), work_controller_tx_to_screen_worker.clone(), state.clone()) //#!#//
               //, MemberOf(&mut responsive_team));
               , SoloAct);

    let state = new_state();
    actor_builder.with_name(NAME_SCREEN_WORKER)
        .build(move |context|
                   actor::screen_worker::run(context, screen_worker_rx_from_work_controller.clone(), screen_worker_tx_to_work_collector.clone(), worker_rx_from_window.clone(), state.clone()) //#!#//
               //, MemberOf(&mut responsive_team));
               , SoloAct);

    let state = new_state();
    actor_builder.with_name(NAME_WORK_COLLECTOR)
        .build(move |context|
            actor::work_collector::run(context, work_collector_rx_from_screen_worker.clone(), work_collector_tx_to_escaper.clone(), state.clone())
            , SoloAct
        );

    let state = new_state();
    actor_builder.with_name(NAME_ESCAPER)
        .build(move |context|
                   actor::escaper::run(context, escaper_rx_from_work_collector.clone(), escaper_settings.clone(), escaper_tx_to_colorer.clone(), state.clone())
               , SoloAct
        );
}