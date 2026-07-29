#![allow(warnings)]

use steady_state::*;
use arg::MainArg;
mod arg;

use rug::*;

use std::thread;
use assemblies::{headgroup, workgroup, gpu_uploader};

pub mod settings;

pub mod utils;
pub mod range;
pub mod constants;
pub mod gpu_context;
pub mod gpu_budget;
pub mod gear;
pub mod assemblies;
pub mod intexp;
pub mod stacked_intexp;
pub mod floatexp;

const STACK_SIZE:usize = 100 * 1024 * 1024;
fn main() {
    #[cfg(target_os = "linux")]
    if std::env::var("WINIT_X11_SCALE_FACTOR").is_err() {
        std::env::set_var("WINIT_X11_SCALE_FACTOR", "1");
    }


    let builder = thread::Builder::new()
        .name("worker-thread".into())
        .stack_size(STACK_SIZE);

    let handler = builder.spawn(|| {
        let cli_args = MainArg::parse();

        init_logging(LogLevel::Info, None);

        let mut graph = GraphBuilder::default()
            .with_telemtry_production_rate_ms(40)
            .with_default_actor_stack_size(STACK_SIZE)
            .build(cli_args);

        // One device for the whole app, built before the window so eframe adopts
        // it rather than creating a second one the compute side cannot address.
        let gpu = gpu_context::GpuContext::shared();

        build_graph(&mut graph, gpu);

        graph.start();

        graph.block_until_stopped(Duration::from_millis(100));
    }).expect("Failed to spawn thread");

    handler.join().expect("Thread panicked");

}

const NAME_WINDOW: &str = "window";
const NAME_GPU_UPLOADER: &str = "gpu uploader";
const NAME_TILE_PUBLISHER: &str = "tile publisher";
const NAME_TILE_SCHEDULER: &str = "tile scheduler";
const NAME_TILE_WORKER: &str = "tile worker";
const NAME_INTRATILE_SCHEDULER: &str = "intratile scheduler";
const NAME_REFERENCE_WORKER: &str = "reference worker";

fn build_graph(graph: &mut Graph, _gpu: gpu_context::SharedGpu) {
    let channel_builder = graph.channel_builder()
        .with_compute_refresh_window_floor(Duration::from_secs(4),Duration::from_secs(24))
        .with_filled_trigger(Trigger::AvgAbove(Filled::p90()), AlertColor::Red)
        .with_filled_trigger(Trigger::AvgAbove(Filled::p60()), AlertColor::Orange)
        .with_avg_rate()
        .with_capacity(10);

    // Auth workgroup layout:
    // window ──stencil──► tile scheduler → tile worker ⇄ intratile scheduler
    //       └──stencil──► reference worker → tile worker
    // tile worker → gpu uploader → tile publisher → window
    //            ╲───────────────────↗ (GPU-native bypass)

    let (
        uploader_tx_to_publisher
        , publisher_rx_from_uploader
    ) = channel_builder.with_capacity(512).build();

    let (
        publisher_tx_to_window
        , window_rx_from_publisher
    ) = channel_builder.with_capacity(512).build();

    // Stencil fan-out: [0] tile scheduler, [1] reference worker (whole screen).
    let (
        window_tx_stencil
        , stencil_rx_bundle
    ) = channel_builder.with_capacity(64).build_channel_bundle::<_, 2>();

    let (
        window_tx_attention
        , scheduler_rx_attention
    ) = channel_builder.with_capacity(64).build();

    let (
        window_tx_to_stuff
        , stuff_rx_from_window
    ) = channel_builder.with_capacity(64).build_channel_bundle();

    let (
        scheduler_tx_to_worker
        , worker_rx_from_scheduler
    ) = channel_builder.with_capacity(64).build();

    let (
        reference_tx_to_worker
        , worker_rx_from_reference
    ) = channel_builder.with_capacity(8).build();

    let (
        worker_tx_to_uploader
        , uploader_rx_from_worker
    ) = channel_builder.with_capacity(512).build();

    let (
        worker_tx_bypass_publisher
        , publisher_rx_bypass
    ) = channel_builder.with_capacity(512).build();

    let (
        worker_tx_memory_bump
        , publisher_rx_memory_bump
    ) = channel_builder.with_capacity(8).build();

    let (
        publisher_tx_memory_bump
        , window_rx_memory_bump
    ) = channel_builder.with_capacity(8).build();

    let (
        worker_tx_to_intratile
        , intratile_rx_from_worker
    ) = channel_builder.with_capacity(64).build();

    let (
        intratile_tx_to_worker
        , worker_rx_from_intratile
    ) = channel_builder.with_capacity(64).build();

    let (intratile_rpc_tx, intratile_rpc_rx) = std::sync::mpsc::sync_channel(64);
    let intratile_client = workgroup::actor_messages::IntratileClient::new(intratile_rpc_tx);
    let intratile_rpc_rx_slot = std::sync::Arc::new(std::sync::Mutex::new(Some(intratile_rpc_rx)));

    let actor_builder = graph.actor_builder()
        .with_thread_info()
        .with_mcpu_trigger(Trigger::AvgAbove(MCPU::m768()), AlertColor::Red)
        .with_mcpu_trigger(Trigger::AvgAbove(MCPU::m512()), AlertColor::Orange)
        .with_mcpu_trigger(Trigger::AvgAbove(MCPU::m256()), AlertColor::Yellow)
        .with_load_avg()
        .with_mcpu_avg();

    let state = new_state();
    actor_builder.with_name(NAME_WINDOW)
        .build(move |context|
            headgroup::window::run(
                context
                , window_rx_from_publisher.clone()
                , window_tx_stencil.clone()
                , window_tx_to_stuff.clone()
                , window_tx_attention.clone()
                , window_rx_memory_bump.clone()
                , state.clone()
            )
               , SoloAct);

    let settings_rx_to_publisher = stuff_rx_from_window[0].clone();
    let state = new_state();
    actor_builder.with_name(NAME_TILE_PUBLISHER)
        .build(move |context|
            assemblies::workgroup::tile_publisher::run_actor(
                context
                , publisher_rx_from_uploader.clone()
                , publisher_rx_bypass.clone()
                , publisher_tx_to_window.clone()
                , publisher_rx_memory_bump.clone()
                , publisher_tx_memory_bump.clone()
                , settings_rx_to_publisher.clone()
                , state.clone()
            )
               , SoloAct);

    let state = new_state();
    actor_builder.with_name(NAME_GPU_UPLOADER)
        .build(move |context|
            gpu_uploader::run(
                context
                , uploader_rx_from_worker.clone()
                , uploader_tx_to_publisher.clone()
                , state.clone()
            )
               , SoloAct);

    let scheduler_rx_from_window = stencil_rx_bundle[0].clone();
    let reference_rx_from_window = stencil_rx_bundle[1].clone();

    let state = new_state();
    actor_builder.with_name(NAME_TILE_SCHEDULER)
        .build(move |context|
            workgroup::tile_scheduler_actor::run(
                context
                , scheduler_rx_from_window.clone()
                , scheduler_rx_attention.clone()
                , scheduler_tx_to_worker.clone()
                , state.clone()
            )
               , SoloAct);

    let state = new_state();
    actor_builder.with_name(NAME_REFERENCE_WORKER)
        .build(move |context|
            workgroup::reference_actor::run(
                context
                , reference_rx_from_window.clone()
                , reference_tx_to_worker.clone()
                , state.clone()
            )
               , SoloAct);

    let state = new_state();
    let intratile_rpc_rx_slot = intratile_rpc_rx_slot.clone();
    actor_builder.with_name(NAME_INTRATILE_SCHEDULER)
        .build(move |context| {
            let rpc_rx = intratile_rpc_rx_slot
                .lock()
                .expect("intratile rpc slot")
                .take()
                .expect("intratile rpc rx installed once");
            workgroup::intratile_actor::run(
                context
                , intratile_rx_from_worker.clone()
                , intratile_tx_to_worker.clone()
                , rpc_rx
                , state.clone()
            )
        }
               , SoloAct);

    let state = new_state();
    let intratile_client_for_worker = intratile_client.clone();
    actor_builder.with_name(NAME_TILE_WORKER)
        .build(move |context|
            workgroup::tile_worker::run(
                context
                , worker_rx_from_scheduler.clone()
                , worker_rx_from_reference.clone()
                , worker_tx_to_uploader.clone()
                , worker_tx_bypass_publisher.clone()
                , worker_tx_memory_bump.clone()
                , worker_tx_to_intratile.clone()
                , worker_rx_from_intratile.clone()
                , intratile_client_for_worker.clone()
                , state.clone()
            )
               , SoloAct);
}
