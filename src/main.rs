use steady_state::*;
use arg::MainArg;
mod arg;

// The actor module contains all the actor implementations for this pipeline.
// Each actor is in its own submodule for clarity and separation of concerns.
pub(crate) mod actor {
    pub(crate) mod window;
    pub(crate) mod transformer;
    pub(crate) mod computer;
}

fn main() -> Result<(), Box<dyn Error>> {
    // Parse command-line arguments (rate, beats, etc.) using clap.
    let cli_args = MainArg::parse();

    // Initialize logging at Info level for runtime diagnostics and performance output.
    init_logging(LogLevel::Info)?;

    // Build the actor graph with all channels and actors, using the parsed arguments.
    let mut graph = GraphBuilder::default()
        .with_telemtry_production_rate_ms(200)
        .build(cli_args);

    // Construct the full actor pipeline and channel topology.
    build_graph(&mut graph);

    // Start the entire actor system. All actors and channels are now live.
    graph.start();

    // The system runs until an actor requests shutdown or the timeout is reached.
    graph.block_until_stopped(Duration::from_secs(1))
}

// Actor names for use in graph construction and testing.

const NAME_WINDOW: &str = "window";
const NAME_TRANSFORMER: &str = "transformer";
const NAME_COMPUTER: &str = "computer";


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
        .with_capacity(10);

    // Channel capacities are set extremely large for high-throughput, batch-friendly operation.
    // - Heartbeat channel: moderate size for timing signals
    // - Generator and computer channels: 1,048,576 messages (1<<20) for massive batch processing


    // window to and from transformer channels
    let (
        window_tx_to_transformer
        , transformer_rx_from_window
    ) = channel_builder.build();
    let (
        transformer_tx_to_window
        , window_rx_from_transformer
    ) = channel_builder.build();

    // transformer to and from computer channels
    let (
        transformer_tx_to_computer
        , computer_rx_from_transformer
    ) = channel_builder.build();
    let (
        computer_tx_to_transformer
        , transformer_rx_from_computer
    ) = channel_builder.build();



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

    let state = new_state();
    actor_builder.with_name(NAME_WINDOW)
        .build(move |context|
            actor::window::run(context, window_rx_from_transformer.clone(), window_tx_to_transformer.clone(), state.clone()) //#!#//
               //, MemberOf(&mut responsive_team));
               , SoloAct);

    let state = new_state();
    actor_builder.with_name(NAME_TRANSFORMER)
        .build(move |context|
                   actor::transformer::run(context, transformer_rx_from_window.clone(), transformer_rx_from_computer.clone(), transformer_tx_to_window.clone(), transformer_tx_to_computer.clone(), state.clone()) //#!#//
               //, MemberOf(&mut responsive_team));
               , SoloAct);

    let state = new_state();
    actor_builder.with_name(NAME_COMPUTER)
        .build(move |context|
                   actor::computer::run(context, computer_rx_from_transformer.clone(), computer_tx_to_transformer.clone(), state.clone()) //#!#//
               , SoloAct);
}