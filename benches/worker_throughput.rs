//! Mandelbrot worker iteration throughput.
//!
//! Reports iterations/sec for the live hot loops:
//! - naive CPU (`iterate_point_bout`)
//! - perturbation CPU zero-orbit (`iterate_perturbation_bout`)
//! - perturbation CPU with a live period-2 reference orbit
//!
//! An "iteration" is one `iteration_count += 1` step inside a bout.
//! Timing covers bouts only (no initialize_batch / series skip).
//! Fixtures are exterior points with long escape times so periodicity
//! confirmation and reseed overhead do not dominate the measurement.

#![allow(warnings)]

use std::hint::black_box;
use std::time::{Duration, Instant};

use criterion::{
    criterion_group, criterion_main, BenchmarkId, Criterion, Throughput,
};

use critical_zoomer::assemblies::workgroup_new::workcore::mandelbrot::worker_implementations::naive_cpu_worker::iterate_point_bout;
use critical_zoomer::assemblies::workgroup_new::workcore::mandelbrot::worker_implementations::periodicity_detector::*;
use critical_zoomer::assemblies::workgroup_new::workcore::mandelbrot::worker_implementations::perturbation_cpu_worker::{
    iterate_perturbation_bout
    , PerturbationCpuWorkerState
};
use critical_zoomer::assemblies::workgroup_new::workcore::mandelbrot::*;
use critical_zoomer::gear::Gear;
use critical_zoomer::gpu_budget::SubmissionBudget;
use critical_zoomer::stacked_intexp::StackedIntExp;

/// Iterations accumulated per Criterion sample.
const TARGET_ITERS: u64 = 200_000;
const BOUT: u32 = 1_000;

/// Exterior points that *escape* (not periodicity-finish) after many iterations.
/// Chosen by the baseline probe so reseeds stay rare within TARGET_ITERS.
const FIXTURES: &[(&str, (f64, f64))] = &[
    // Just outside the main cardioid cusp — classic slow-escape exterior.
    // Escape depth is ~3e5 iters, so a TARGET_ITERS sample fits in one seed.
    ("cusp_out", (0.2500001, 0.0)),
    // Slightly further out from the cusp — still deep, fewer iters.
    ("cusp_far", (0.251, 0.0)),
    // Outside near the period-2 bulb neck.
    ("neck", (-0.75, 0.02)),
];

fn epsilon(c: (f64, f64)) -> f64 {
    1e-12f64.max(c.0.abs().max(c.1.abs()) * 1e-6)
}

fn fresh_point(c: (f64, f64), orbit_id: OrbitId) -> ActivePoint<f64, CpuPeriodicityDetector> {
    let z = (0.0, 0.0);
    let derivative = (1.0, 0.0);
    ActivePoint {
        c
        , z
        , derivative
        , real_squared: 0.0
        , imag_squared: 0.0
        , real_imag: 0.0
        , iteration_count: 0
        , min_magnitude: f64::MAX
        , min_magnitude_time: 0
        , periodicity_detector: CpuPeriodicityDetector::init(0, z, derivative)
        , escaped: false
        , finished: false
        , orbit_id
        , seat_linear: 0
    }
}

fn characterize_naive(c: (f64, f64), cap: u64) -> (u64, bool, bool) {
    let eps = epsilon(c);
    let mut point = fresh_point(c, ZERO_ORBIT_ID);
    while !point.finished && point.iteration_count < cap {
        iterate_point_bout(&mut point, 4.0, eps, BOUT);
    }
    (point.iteration_count, point.escaped, point.finished)
}

fn characterize_perturb(c: (f64, f64), cap: u64) -> (u64, bool, bool) {
    let eps = epsilon(c);
    let mut state = PerturbationCpuWorkerState::default();
    state.seat_orbit_ids = vec![ZERO_ORBIT_ID];
    state.iterations_per_bout = BOUT;
    let mut point = fresh_point(c, ZERO_ORBIT_ID);
    while !point.finished && point.iteration_count < cap {
        iterate_perturbation_bout(&mut state, &mut point, eps);
    }
    (point.iteration_count, point.escaped, point.finished)
}

fn run_naive(c: (f64, f64), target: u64) -> (Duration, u64) {
    let eps = epsilon(c);
    let mut total = 0u64;
    let start = Instant::now();
    while total < target {
        let mut point = fresh_point(c, ZERO_ORBIT_ID);
        while !point.finished && total < target {
            let before = point.iteration_count;
            let bout = ((target - total) as u32).min(BOUT).max(1);
            iterate_point_bout(&mut point, 4.0, eps, bout);
            let gained = point.iteration_count - before;
            if gained == 0 {
                break;
            }
            total += gained;
        }
        black_box(point.iteration_count);
    }
    (start.elapsed(), total)
}

fn run_perturb_zero(c: (f64, f64), target: u64) -> (Duration, u64) {
    let eps = epsilon(c);
    let mut state = PerturbationCpuWorkerState::default();
    state.seat_orbit_ids = vec![ZERO_ORBIT_ID];
    state.iterations_per_bout = BOUT;
    let mut total = 0u64;
    let start = Instant::now();
    while total < target {
        let mut point = fresh_point(c, ZERO_ORBIT_ID);
        while !point.finished && total < target {
            let before = point.iteration_count;
            let bout = ((target - total) as u32).min(BOUT).max(1);
            state.iterations_per_bout = bout;
            iterate_perturbation_bout(&mut state, &mut point, eps);
            let gained = point.iteration_count - before;
            if gained == 0 {
                break;
            }
            total += gained;
        }
        black_box(point.iteration_count);
    }
    (start.elapsed(), total)
}

/// Perturbation relative to the period-2 nucleus at (-1, 0).
/// `pixel_c` is the absolute pixel coordinate; delta_c = pixel_c - ref_c.
fn run_perturb_period2(pixel_c: (f64, f64), target: u64) -> (Duration, u64) {
    let ref_c = (-1.0, 0.0);
    let delta_c = (pixel_c.0 - ref_c.0, pixel_c.1 - ref_c.1);
    let eps = epsilon(pixel_c);
    let mut state = PerturbationCpuWorkerState::default();
    let orbit_id = state.references.try_add_nucleus_at_f64(ref_c);
    assert_ne!(orbit_id, ZERO_ORBIT_ID, "period-2 nucleus should add");
    state.seat_orbit_ids = vec![orbit_id];
    state.iterations_per_bout = BOUT;
    let mut total = 0u64;
    let start = Instant::now();
    while total < target {
        let mut point = fresh_point(delta_c, orbit_id);
        while !point.finished && total < target {
            let before = point.iteration_count;
            let bout = ((target - total) as u32).min(BOUT).max(1);
            state.iterations_per_bout = bout;
            iterate_perturbation_bout(&mut state, &mut point, eps);
            let gained = point.iteration_count - before;
            if gained == 0 {
                // Glitch rebind resets to zero; keep going on the rebound.
                if point.orbit_id == ZERO_ORBIT_ID && !point.finished {
                    continue;
                }
                break;
            }
            total += gained;
        }
        black_box(point.iteration_count);
    }
    (start.elapsed(), total)
}

fn rate_m(iters: u64, elapsed: Duration) -> f64 {
    let secs = elapsed.as_secs_f64().max(1e-12);
    (iters as f64 / secs) / 1e6
}

fn print_baseline_snapshot() {
    println!();
    println!("=== fixture characterization (cap=200000) ===");
    for &(name, c) in FIXTURES {
        let (ni, ne, nf) = characterize_naive(c, 200_000);
        let (pi, pe, pf) = characterize_perturb(c, 200_000);
        println!(
            "  {name:12}  naive: iters={ni:<6} escaped={ne} finished={nf}  |  perturb: iters={pi:<6} escaped={pe} finished={pf}"
        );
    }
    println!();
    println!("=== worker throughput baseline (single-threaded, TARGET_ITERS={TARGET_ITERS}) ===");
    println!("    (Miter/s = million counted iterations per second)");
    for &(name, c) in FIXTURES {
        let (naive_t, naive_n) = run_naive(c, TARGET_ITERS);
        let (pert_t, pert_n) = run_perturb_zero(c, TARGET_ITERS);
        let (p2_t, p2_n) = run_perturb_period2(c, TARGET_ITERS);
        println!(
            "  {name:12}  naive={:>8.2} Miter/s  perturb_zero={:>8.2} Miter/s  perturb_p2={:>8.2} Miter/s"
            , rate_m(naive_n, naive_t)
            , rate_m(pert_n, pert_t)
            , rate_m(p2_n, p2_t)
        );
    }
    // Longer sustained sample on the slowest-escape exterior fixture.
    let deep = FIXTURES[0].1;
    let sustain = 2_000_000u64;
    let (naive_t, naive_n) = run_naive(deep, sustain);
    let (pert_t, pert_n) = run_perturb_zero(deep, sustain);
    let (p2_t, p2_n) = run_perturb_period2(deep, sustain);
    println!();
    println!("=== sustained 2M-iter sample @ {} ===", FIXTURES[0].0);
    println!(
        "  naive={:>8.2} Miter/s  perturb_zero={:>8.2} Miter/s  perturb_p2={:>8.2} Miter/s"
        , rate_m(naive_n, naive_t)
        , rate_m(pert_n, pert_t)
        , rate_m(p2_n, p2_t)
    );
    println!("=======================================================================");
    println!();
}

fn worker_throughput(c: &mut Criterion) {
    print_baseline_snapshot();

    let mut group = c.benchmark_group("worker_iters");
    group.throughput(Throughput::Elements(TARGET_ITERS));
    group.sample_size(20);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(4));
    group.noise_threshold(0.10);

    for &(name, point_c) in FIXTURES {
        group.bench_with_input(
            BenchmarkId::new("naive", name)
            , &point_c
            , |b, &c_val| {
                b.iter_custom(|samples| {
                    let mut elapsed = Duration::ZERO;
                    for _ in 0..samples {
                        let (t, n) = run_naive(c_val, TARGET_ITERS);
                        assert!(n >= TARGET_ITERS / 2, "naive under-iterated: {n}");
                        elapsed += t;
                        black_box(n);
                    }
                    elapsed
                });
            }
        );

        group.bench_with_input(
            BenchmarkId::new("perturb_zero", name)
            , &point_c
            , |b, &c_val| {
                b.iter_custom(|samples| {
                    let mut elapsed = Duration::ZERO;
                    for _ in 0..samples {
                        let (t, n) = run_perturb_zero(c_val, TARGET_ITERS);
                        assert!(n >= TARGET_ITERS / 2, "perturb_zero under-iterated: {n}");
                        elapsed += t;
                        black_box(n);
                    }
                    elapsed
                });
            }
        );

        group.bench_with_input(
            BenchmarkId::new("perturb_p2", name)
            , &point_c
            , |b, &c_val| {
                b.iter_custom(|samples| {
                    let mut elapsed = Duration::ZERO;
                    for _ in 0..samples {
                        let (t, n) = run_perturb_period2(c_val, TARGET_ITERS);
                        assert!(n >= TARGET_ITERS / 2, "perturb_p2 under-iterated: {n}");
                        elapsed += t;
                        black_box(n);
                    }
                    elapsed
                });
            }
        );
    }

    group.finish();
}

fn gear_select_and_stacked_mul(c: &mut Criterion) {
    let mut group = c.benchmark_group("gear_ladder");
    group.bench_function("select_bits_sweep", |b| {
        b.iter(|| {
            for bits in [8u32, 20, 40, 64, 128, 256] {
                black_box(Gear::select(bits, true));
                black_box(Gear::select(bits, false));
            }
        })
    });
    group.bench_function("stacked_i32_4_mul", |b| {
        let a = StackedIntExp::<4>::from(12345);
        let cval = StackedIntExp::<4>::from(-6789);
        b.iter(|| black_box(a * cval))
    });
    group.finish();
}

fn frame_budget_observe(c: &mut Criterion) {
    let mut group = c.benchmark_group("frame_budget");
    group.bench_function("observe_smoothing", |b| {
        b.iter(|| {
            let mut budget = SubmissionBudget::new();
            for _ in 0..100 {
                let n = budget.iterations_for(1024);
                budget.observe(1024, n, Duration::from_micros(200));
                black_box(n);
            }
        })
    });
    // Dispatch-size sweep: how iterations_for responds to slow vs fast work.
    for micros in [50u64, 200, 800, 2000] {
        group.bench_with_input(
            BenchmarkId::new("after_observe_us", micros),
            &micros,
            |b, &us| {
                b.iter(|| {
                    let mut budget = SubmissionBudget::new();
                    budget.observe(1024, 1000, Duration::from_micros(us));
                    black_box(budget.iterations_for(1024))
                })
            },
        );
    }
    group.finish();
}

criterion_group!(benches, worker_throughput, gear_select_and_stacked_mul, frame_budget_observe);
criterion_main!(benches);
