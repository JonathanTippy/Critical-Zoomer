// Workgroup fitness benchmarks — first-class performance tracking, not tests.
// Baselines and the regression-guard process live in docs/assistant/benchmarks.md.
//
// Run (house rule: nice, center-half CPUs):
//   taskset -c 3-8 nice -n 15 cargo bench --bench workgroup_fitness

#![allow(warnings)]

use std::hint::black_box;
use std::time::{Duration, Instant};

use criterion::*;

use critical_zoomer::assemblies::workgroup::screen_worker::workshift::*;
use critical_zoomer::constants::{DEFAULT_WINDOW_RES, HOME_POSITION};
use critical_zoomer::utils::{IntExp, ObjectivePosAndZoom};

// WorkContext build still uses run_big for headroom with the rest of the
// workgroup fixtures.
fn run_big<R: Send + 'static>(f: impl FnOnce() -> R + Send + 'static) -> R {
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap()
}

/// Builds the home-view context the same way the worker does on Replace:
/// controller frame_info → from_stencil → lazy seat init on start.
fn home_context() -> WorkContext<f64> {
    let res = DEFAULT_WINDOW_RES;
    // Live path: window flips imag into the stencil, controller flips again into
    // frame_info — for HOME that lands frame_info.pos == HOME (real, imag, zoom).
    // from_stencil flips once more to recover the compute-grid origin.
    let frame_info = (
        ObjectivePosAndZoom {
            pos: (
                IntExp::from(HOME_POSITION.0),
                IntExp::from(HOME_POSITION.1),
            ),
            zoom_pot: HOME_POSITION.2,
        },
        res,
    );
    from_stencil(frame_info, None).expect("home view must admit an f64 grid")
}

fn drain(ctx: &mut WorkContext<f64>) -> usize {
    let mut n = 0;
    while ctx.completed_points.try_pop().is_some() {
        n += 1;
    }
    n
}

fn frame_complete(ctx: &WorkContext<f64>) -> bool {
    ctx.points.iter().all(|p| p.delivered)
}

// cz.perf.play-minimize / cz.perf.play-8bump-100ms: how quickly the first work lands.
fn time_to_first_publish(c: &mut Criterion) {
    let mut group = c.benchmark_group("workgroup_fitness");
    group.sample_size(30);
    group.bench_function("time_to_first_publish", |b| {
        b.iter(|| {
            run_big(|| {
                let mut ctx = home_context();
                let start = Instant::now();
                let first = loop {
                    workshift(0, 0, 0, 0, &mut ctx);
                    let got = drain(&mut ctx);
                    if got > 0 {
                        break got;
                    }
                };
                black_box(first);
                start.elapsed()
            })
        });
    });
    group.finish();
}

// cz.perf.home-100tps (re-expressed): home view wall-clock fill time.
// Also prints full-stack IPS (cz.perf.min-300m-ips-cpu method: real workgroup
// loop, scheduling included).
fn time_to_full_frame(c: &mut Criterion) {
    let mut group = c.benchmark_group("workgroup_fitness");
    group.sample_size(10);
    group.bench_function("time_to_full_frame", |b| {
        b.iter(|| {
            run_big(|| {
                let mut ctx = home_context();
                let start = Instant::now();
                let mut shifts = 0u32;
                loop {
                    workshift(0, 0, 0, 0, &mut ctx);
                    drain(&mut ctx);
                    shifts += 1;
                    if frame_complete(&ctx) {
                        break;
                    }
                    if shifts > 200_000 {
                        panic!("home frame did not complete — investigate before benchmarking");
                    }
                }
                let elapsed = start.elapsed();
                let ips = ctx.total_iterations as f64 / elapsed.as_secs_f64();
                println!(
                    "full_stack_ips: {:.0}  ({} iterations, {} shifts, {:.2?})",
                    ips, ctx.total_iterations, shifts, elapsed
                );
                elapsed
            })
        });
    });
    group.finish();
}

criterion_group!(benches, time_to_first_publish, time_to_full_frame);
criterion_main!(benches);
