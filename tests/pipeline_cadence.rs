//! Dummy-head pipeline cadence. Integration test so it runs **once** (lib
//! harness only) — `src/main.rs` duplicates modules, so `#[cfg(test)]` in
//! `pipeline.rs` used to fail the same pins on `--lib` and `--bin`.
//!
//! Cross-process dir lock (`WgpuTestLock`): an ad-hoc `--all-targets` can still
//! overlap this harness with the lib IPS probe. `full_check.sh` runs cadence
//! after unit/integration. In-process GPU unit tests use `lock_gpu_tests()`
//! (a mutex), not this dir lock. Floors assume **opt-level 3** (house
//! `profile.dev` / `profile.test`). Do not gate on `--release` (overflow
//! checks off). Unoptimized debug will miss the Hz bars.
//!
//! One `#[test]` so rustc cannot run OG and GPU pipelines in parallel
//! (they share the GPU and starve `esc:`).

use critical_zoomer::assemblies::headgroup::dummy_cadence::{
    cadence_lab_settings, CadenceReport, DummyCadenceConfig,
};
use critical_zoomer::assemblies::pipeline::{new_report_slot, run_dummy_cadence_pipeline};
use critical_zoomer::assemblies::structs::{ColorerMode, EscaperMode};
use std::time::Duration;

const HEALTHY_COL_HZ: f64 = 40.0;
const HEALTHY_OG_ESC_HZ: f64 = 15.0;
const HEALTHY_GPU_ESC_HZ: f64 = 40.0;
const MEASURE: Duration = Duration::from_secs(5);

fn run_case(escape: EscaperMode, color: ColorerMode) -> CadenceReport {
    let _lock = critical_zoomer::debug_agent::WgpuTestLock::acquire();
    critical_zoomer::debug_agent::init_cpu_profile_from_env();
    critical_zoomer::debug_agent::enable_escape_rca();
    let _ = critical_zoomer::assemblies::shadergroup::colorer::gpu::GpuColorer::shared();
    let _ = critical_zoomer::assemblies::shadergroup::escaper::gpu::GpuEscaper::shared();
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

fn assert_og(r: &CadenceReport) {
    print_hud(r);
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

fn assert_gpu(r: &CadenceReport) {
    print_hud(r);
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

#[test]
fn steady_state_pipeline_cadence() {
    assert_og(&run_case(EscaperMode::Og, ColorerMode::Gpu));
    // Tear down OG colorer GPU work before the GPU-escaper case; back-to-back
    // graphs on a busy full_check machine otherwise miss the 40 Hz floor.
    std::thread::sleep(Duration::from_millis(500));
    assert_gpu(&run_case(EscaperMode::Gpu, ColorerMode::Gpu));
}
