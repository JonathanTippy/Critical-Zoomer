#![allow(warnings)]

use steady_state::*;
use arg::MainArg;
mod arg;

use rug::*;

use std::thread;
use assemblies::{headgroup, workgroup, gpu_uploader};

pub mod actor {}
pub mod settings;

pub mod utils;
pub mod range;
pub mod constants;
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

        build_graph(&mut graph);

        graph.start();

        graph.block_until_stopped(Duration::from_millis(100));
    }).expect("Failed to spawn thread");

    handler.join().expect("Thread panicked");

}

const NAME_WINDOW: &str = "window";
const NAME_GPU_UPLOADER: &str = "gpu uploader";
const NAME_WORK_CONTROLLER: &str = "work controller";
const NAME_SCREEN_WORKER:&str = "screen worker";

fn build_graph(graph: &mut Graph) {
    let channel_builder = graph.channel_builder()
        .with_compute_refresh_window_floor(Duration::from_secs(4),Duration::from_secs(24))
        .with_filled_trigger(Trigger::AvgAbove(Filled::p90()), AlertColor::Red)
        .with_filled_trigger(Trigger::AvgAbove(Filled::p60()), AlertColor::Orange)
        .with_avg_rate()
        .with_capacity(10);

    let (
        uploader_tx_to_window
        , window_rx_from_uploader
    ) = channel_builder.with_capacity(512).build();

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

    let (
        work_controller_tx_to_screen_worker
        , screen_worker_rx_from_work_controller
    ) = channel_builder.with_capacity(10).build();

    let (
        screen_worker_tx_to_uploader
        , uploader_rx_from_screen_worker
    ) = channel_builder.with_capacity(512).build();

    // Publisher (interim: screen_worker) → headgroup memory bumps.
    let (
        screen_worker_tx_memory_bump
        , window_rx_memory_bump
    ) = channel_builder.with_capacity(8).build();

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
                , window_rx_from_uploader.clone()
                , window_tx_to_work_controller.clone()
                , window_tx_to_stuff.clone()
                , window_tx_to_worker.clone()
                , window_rx_memory_bump.clone()
                , state.clone()
            )
               , SoloAct);

    let state = new_state();
    actor_builder.with_name(NAME_GPU_UPLOADER)
        .build(move |context|
            gpu_uploader::run(
                context
                , uploader_rx_from_screen_worker.clone()
                , uploader_tx_to_window.clone()
                , state.clone()
            )
               , SoloAct);

    let state = new_state();
    actor_builder.with_name(NAME_WORK_CONTROLLER)
        .build(move |context|
                   workgroup::work_controller::run(context, work_controller_rx_from_window.clone(), work_controller_tx_to_screen_worker.clone(), state.clone())
               , SoloAct);

    let state = new_state();
    // Settings bundle slot 0 → screen_worker (memory limit); slot 1 reserved.
    let settings_rx_to_screen_worker = stuff_rx_from_window[0].clone();
    actor_builder.with_name(NAME_SCREEN_WORKER)
        .build(move |context|
                   workgroup::screen_worker::run(
                       context
                       , screen_worker_rx_from_work_controller.clone()
                       , screen_worker_tx_to_uploader.clone()
                       , worker_rx_from_window.clone()
                       , settings_rx_to_screen_worker.clone()
                       , screen_worker_tx_memory_bump.clone()
                       , state.clone()
                   )
               , SoloAct);
}
