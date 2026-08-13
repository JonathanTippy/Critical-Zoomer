//! Production actor graph wiring — live window or dummy-head cadence lab.
//!
//! Tests drive the real workgroup + shadergroup with [`HeadKind::DummyCadence`].

use crate::assemblies::{headgroup, shadergroup, workgroup};
use crate::assemblies::headgroup::dummy_cadence::{self, DummyCadenceConfig};
use crate::settings::Settings;
use steady_state::*;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub const STACK_SIZE: usize = 100 * 1024 * 1024;

const NAME_WINDOW: &str = "window";
const NAME_DUMMY_CADENCE: &str = "dummy cadence";
const NAME_COLORER: &str = "colorer";
const NAME_WORK_CONTROLLER: &str = "work controller";
const NAME_SCREEN_WORKER: &str = "screen worker";
const NAME_REFERENCE_WORKER: &str = "reference worker";
const NAME_WORK_COLLECTOR: &str = "work collector";
const NAME_ESCAPER: &str = "point escaper";

/// Which head actor owns stencil / settings / attention / pixel consume.
pub enum HeadKind {
    LiveWindow,
    DummyCadence(DummyCadenceConfig),
}

/// Same channel topology and SoloAct layout as the production binary.
pub fn build_pipeline(graph: &mut Graph, head: HeadKind) {
    let channel_builder = graph
        .channel_builder()
        .with_compute_refresh_window_floor(Duration::from_secs(4), Duration::from_secs(24))
        .with_filled_trigger(Trigger::AvgAbove(Filled::p90()), AlertColor::Red)
        .with_filled_trigger(Trigger::AvgAbove(Filled::p60()), AlertColor::Orange)
        .with_avg_rate()
        .with_capacity(10);

    // r[impl cz.craft.small-channels+1]
    let (colorer_tx_to_window, window_rx_from_colorer) =
        channel_builder.with_capacity(10).build();

    let (window_tx_to_work_controller, work_controller_rx_from_window) =
        channel_builder.with_capacity(50).build();

    let (window_tx_to_worker, worker_rx_from_window) =
        channel_builder.with_capacity(50).build();

    let (window_tx_to_stuff, stuff_rx_from_window) = channel_builder
        .with_capacity(50)
        .build_channel_bundle::<Settings, 4>();

    let (work_controller_tx_to_screen_worker, screen_worker_rx_from_work_controller) =
        channel_builder.with_capacity(10).build();

    let (screen_worker_tx_to_reference_worker, reference_worker_rx_from_screen_worker) =
        channel_builder.with_capacity(10).build();
    let (reference_worker_tx_to_screen_worker, screen_worker_rx_from_reference_worker) =
        channel_builder.with_capacity(1).build();

    let (screen_worker_tx_to_work_collector, work_collector_rx_from_screen_worker) =
        channel_builder.with_capacity(50).build();

    let (work_collector_tx_to_escaper, escaper_rx_from_work_collector) =
        channel_builder.with_capacity(10).build();

    let (escaper_tx_to_colorer, colorer_rx_from_escaper) =
        channel_builder.with_capacity(10).build();

    let actor_builder = graph
        .actor_builder()
        .with_thread_info()
        .with_mcpu_trigger(Trigger::AvgAbove(MCPU::m768()), AlertColor::Red)
        .with_mcpu_trigger(Trigger::AvgAbove(MCPU::m512()), AlertColor::Orange)
        .with_mcpu_trigger(Trigger::AvgAbove(MCPU::m256()), AlertColor::Yellow)
        .with_load_avg()
        .with_mcpu_avg();

    let (colorer_settings, escaper_settings, worker_settings, collector_settings) = (
        stuff_rx_from_window[0].clone(),
        stuff_rx_from_window[1].clone(),
        stuff_rx_from_window[2].clone(),
        stuff_rx_from_window[3].clone(),
    );

    match head {
        HeadKind::LiveWindow => {
            let state = new_state();
            actor_builder.with_name(NAME_WINDOW).build(
                move |context| {
                    headgroup::window::run(
                        context,
                        window_rx_from_colorer.clone(),
                        window_tx_to_work_controller.clone(),
                        window_tx_to_stuff.clone(),
                        window_tx_to_worker.clone(),
                        state.clone(),
                    )
                },
                SoloAct,
            );
        }
        HeadKind::DummyCadence(cfg) => {
            let state = new_state();
            actor_builder.with_name(NAME_DUMMY_CADENCE).build(
                move |context| {
                    dummy_cadence::run(
                        context,
                        window_rx_from_colorer.clone(),
                        window_tx_to_work_controller.clone(),
                        window_tx_to_stuff.clone(),
                        window_tx_to_worker.clone(),
                        state.clone(),
                        cfg.clone(),
                    )
                },
                SoloAct,
            );
        }
    }

    let state = new_state();
    actor_builder.with_name(NAME_COLORER).build(
        move |context| {
            shadergroup::colorer::run(
                context,
                colorer_rx_from_escaper.clone(),
                colorer_settings.clone(),
                colorer_tx_to_window.clone(),
                state.clone(),
            )
        },
        SoloAct,
    );

    let state = new_state();
    actor_builder.with_name(NAME_WORK_CONTROLLER).build(
        move |context| {
            workgroup::work_controller::run(
                context,
                work_controller_rx_from_window.clone(),
                work_controller_tx_to_screen_worker.clone(),
                state.clone(),
            )
        },
        SoloAct,
    );

    let state = new_state();
    actor_builder.with_name(NAME_SCREEN_WORKER).build(
        move |context| {
            workgroup::screen_worker::run(
                context,
                screen_worker_rx_from_work_controller.clone(),
                screen_worker_tx_to_work_collector.clone(),
                worker_rx_from_window.clone(),
                screen_worker_tx_to_reference_worker.clone(),
                screen_worker_rx_from_reference_worker.clone(),
                worker_settings.clone(),
                state.clone(),
            )
        },
        SoloAct,
    );

    let state = new_state();
    actor_builder.with_name(NAME_REFERENCE_WORKER).build(
        move |context| {
            workgroup::reference_worker::run(
                context,
                reference_worker_rx_from_screen_worker.clone(),
                reference_worker_tx_to_screen_worker.clone(),
                state.clone(),
            )
        },
        SoloAct,
    );

    let state = new_state();
    actor_builder.with_name(NAME_WORK_COLLECTOR).build(
        move |context| {
            workgroup::work_collector::run(
                context,
                work_collector_rx_from_screen_worker.clone(),
                work_collector_tx_to_escaper.clone(),
                collector_settings.clone(),
                state.clone(),
            )
        },
        SoloAct,
    );

    let state = new_state();
    actor_builder.with_name(NAME_ESCAPER).build(
        move |context| {
            shadergroup::escaper::run(
                context,
                escaper_rx_from_work_collector.clone(),
                escaper_settings.clone(),
                escaper_tx_to_colorer.clone(),
                state.clone(),
            )
        },
        SoloAct,
    );
}

/// Run the full pipeline under a dummy head until it shuts down; return cadence report.
pub fn run_dummy_cadence_pipeline(
    cfg: DummyCadenceConfig,
) -> dummy_cadence::CadenceReport {
    let report_slot = cfg.report.clone();
    let builder = std::thread::Builder::new()
        .name("cz-dummy-cadence".into())
        .stack_size(STACK_SIZE);
    builder
        .spawn(move || {
            let cli_args = crate::arg::MainArg {
                rate_ms: 2,
                beats: 500_000,
            };
            let mut graph = GraphBuilder::default()
                .with_telemtry_production_rate_ms(40)
                .with_default_actor_stack_size(STACK_SIZE)
                .build(cli_args);
            build_pipeline(&mut graph, HeadKind::DummyCadence(cfg));
            graph.start();
            graph.block_until_stopped(Duration::from_secs(120));
        })
        .expect("spawn cadence pipeline")
        .join()
        .expect("cadence pipeline panicked");

    let report = report_slot
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
        .expect("dummy cadence did not publish a report");
    report
}

/// Shared report cell for [`DummyCadenceConfig`].
pub fn new_report_slot() -> Arc<Mutex<Option<dummy_cadence::CadenceReport>>> {
    Arc::new(Mutex::new(None))
}
