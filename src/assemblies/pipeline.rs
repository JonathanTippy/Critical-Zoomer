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

#[cfg(test)]
mod steady_state_pipeline_cadence_tests {
    use super::*;
    use crate::assemblies::headgroup::dummy_cadence::{
        cadence_lab_settings, CadenceReport, DummyCadenceConfig,
    };
    use crate::assemblies::structs::{ColorerMode, EscaperMode};
    use std::sync::Mutex;
    use std::time::Duration;

    /// Only one full-graph cadence run at a time (GPU + telemetry port).
    static CADENCE_LOCK: Mutex<()> = Mutex::new(());

    /// Healthy color emit floor near 60 Hz content (release GPU color).
    const HEALTHY_COL_HZ: f64 = 40.0;
    /// OG escape stamps at the head today track roughly publish-class rates under
    /// coalesce (see live HUD esc≈pub). Floor catches collapse (~single digits)
    /// without requiring esc≈col until always-emit stamp delivery is hardened.
    const HEALTHY_OG_ESC_HZ: f64 = 15.0;
    /// Aspirational content-class escape rate — GPU escape fails this today (~9).
    const HEALTHY_GPU_ESC_HZ: f64 = 40.0;
    const MEASURE: Duration = Duration::from_secs(5);

    fn run_case(escape: EscaperMode, color: ColorerMode) -> CadenceReport {
        let _guard = CADENCE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // Pre-init shade GPUs on this thread so actor threads do not race adapter create.
        let _ = crate::assemblies::shadergroup::colorer::gpu::GpuColorer::shared();
        let _ = crate::assemblies::shadergroup::escaper::gpu::GpuEscaper::shared();
        let report = new_report_slot();
        let cfg = DummyCadenceConfig {
            settings: cadence_lab_settings(escape, color),
            measure_after_first_frame: MEASURE,
            report: report.clone(),
        };
        run_dummy_cadence_pipeline(cfg)
    }

    fn print_hud(r: &CadenceReport) {
        eprintln!(
            "dummy-head cadence: pub:{:.0}  esc:{:.0}  col:{:.0}  drop:{}  color:{}  escape:{}  first_color_ms:{:.0}  measure_s:{:.1}  totals pub/esc/col:{}/{}/{}  mean_esc:{:.1}",
            r.pub_hz,
            r.esc_hz,
            r.col_hz,
            r.packages_dropped,
            r.color_label,
            r.escape_label,
            r.first_color_after_ms,
            r.measure_secs,
            r.pub_total,
            r.esc_total,
            r.col_total,
            r.esc_total as f64 / r.measure_secs.max(0.001),
        );
    }

    /// Real graph + dummy head; OG escape must keep content-class emit rate.
    #[test]
    fn steady_state_pipeline_cadence_og_escape() {
        let r = run_case(EscaperMode::Og, ColorerMode::Gpu);
        print_hud(&r);
        let mean_esc = r.esc_total as f64 / r.measure_secs.max(0.001);
        let mean_col = r.col_total as f64 / r.measure_secs.max(0.001);
        assert!(
            r.first_color_after_ms >= 0.0,
            "expected at least one colored frame"
        );
        assert!(
            mean_col >= HEALTHY_COL_HZ,
            "mean col:{:.1} (1s col:{:.1}) below healthy floor {HEALTHY_COL_HZ}",
            mean_col,
            r.col_hz
        );
        assert!(
            mean_esc >= HEALTHY_OG_ESC_HZ,
            "mean esc:{:.1} (1s esc:{:.1}) below healthy OG floor {HEALTHY_OG_ESC_HZ}",
            mean_esc,
            r.esc_hz
        );
        assert_eq!(r.escape_label, "OG");
    }

    /// Same graph with GPU escape — pins the live ghost (esc~9 / escaper 100% CPU).
    /// Requires content-class esc (≥40); expected red until GPU escape keeps cadence.
    #[test]
    fn steady_state_pipeline_cadence_gpu_escape() {
        let r = run_case(EscaperMode::Gpu, ColorerMode::Gpu);
        print_hud(&r);
        let mean_esc = r.esc_total as f64 / r.measure_secs.max(0.001);
        let mean_col = r.col_total as f64 / r.measure_secs.max(0.001);
        assert!(
            r.first_color_after_ms >= 0.0,
            "expected at least one colored frame"
        );
        assert!(
            mean_col >= HEALTHY_COL_HZ,
            "mean col:{:.1} (1s col:{:.1}) below healthy floor {HEALTHY_COL_HZ}",
            mean_col,
            r.col_hz
        );
        assert_eq!(r.escape_label, "GPU");
        assert!(
            mean_esc >= HEALTHY_GPU_ESC_HZ,
            "mean esc:{:.1} (1s esc:{:.1}) below healthy GPU floor {HEALTHY_GPU_ESC_HZ} (GPU escape cadence ghost)",
            mean_esc,
            r.esc_hz
        );
    }
}
