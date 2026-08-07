// Workgroup fitness benchmarks — first-class performance tracking, not tests.
// Baselines and the regression-guard process live in docs/assistant/benchmarks.md.
//
// Run (house rule: nice, center-half CPUs):
//   taskset -c 3-8 nice -n 15 cargo bench --bench workgroup_fitness

#![allow(warnings)]

use std::collections::VecDeque;
use std::hint::black_box;
use std::time::{Duration, Instant};

use criterion::*;

use critical_zoomer::assemblies::workgroup::screen_worker::workshift::*;
use critical_zoomer::assemblies::workgroup::work_controller::get_points;
use critical_zoomer::constants::{DEFAULT_WINDOW_RES, HOME_POSITION};
use critical_zoomer::utils::IntExp;

// WorkContext<f64> carries an inline ~5 MB Stec; build and run it on a
// big-stack thread (same pattern as the craftsmanship tests).
fn run_big<R: Send + 'static>(f: impl FnOnce() -> R + Send + 'static) -> R {
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap()
}

/// Builds the home-view context exactly as the work controller does
/// (work_controller.rs handle_sampler_stuff): negated imag, shuffled perimeter
/// as scredge, shuffled mixmap.
fn home_context() -> WorkContext<f64> {
    let res = DEFAULT_WINDOW_RES;
    let loc = (
        IntExp::from(HOME_POSITION.0),
        IntExp::from(0) - IntExp::from(HOME_POSITION.1),
    );
    let zoom_pot = HOME_POSITION.2 as i64;

    let n = (res.0 * res.1) as usize;
    let mut random_map: Vec<usize> = (0..n).collect();
    {
        use rand::seq::SliceRandom;
        random_map.shuffle(&mut rand::rng());
    }

    let mut edges = Vec::new();
    for i in 0..(res.0 - 1) as i32 {
        edges.push((i, 0))
    }
    for i in 0..(res.1 - 1) as i32 {
        edges.push(((res.0 - 1) as i32, i))
    }
    for i in 0..(res.0) as i32 {
        edges.push((i, (res.1 - 1) as i32))
    }
    for i in 1..(res.1 - 1) as i32 {
        edges.push((0, i))
    }
    {
        use rand::seq::SliceRandom;
        edges.shuffle(&mut rand::rng());
    }

    WorkContext {
        points: get_points(res, loc, zoom_pot),
        completed_points: Stec { stuff: [(CompletedPoint::Dummy {}, 0); 100000], len: 0 },
        index: 0,
        random_index: 0,
        time_created: Instant::now(),
        time_workshift_started: Instant::now(),
        percent_completed: 0.0,
        random_map,
        workshifts: 0,
        total_iterations: 0,
        spent_tokens_today: 0,
        total_iterations_today: 0,
        total_points_today: 0,
        total_bouts_today: 0,
        last_update: 0,
        res,
        scredge_poses: VecDeque::from(edges),
        edge_queue: VecDeque::new(),
        out_queue: VecDeque::new(),
        in_queue: VecDeque::new(),
        zoomed: false,
        attention: (0, 0),
    }
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
