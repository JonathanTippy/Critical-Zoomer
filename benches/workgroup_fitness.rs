// Workgroup fitness benchmarks — first-class performance tracking, not tests.
// Baselines and the regression-guard process live in docs/assistant/benchmarks.md.
//
// Run (house rule: nice, center-half CPUs):
//   taskset -c 3-8 nice -n 15 cargo bench --bench workgroup_fitness

#![allow(warnings)]

use std::hint::black_box;
use std::sync::Arc;
use std::time::Instant;

use criterion::*;

use critical_zoomer::assemblies::workgroup::reference_worker::{
    select_reference_request, PublishedReference,
};
use critical_zoomer::assemblies::workgroup::screen_worker::perturb_floatexp::FloatExpPerturbationKernel;
use critical_zoomer::assemblies::workgroup::screen_worker::perturb_kernel::PerturbationKernel;
use critical_zoomer::assemblies::workgroup::screen_worker::workshift::*;
use critical_zoomer::constants::{DEFAULT_WINDOW_RES, HOME_POSITION};
use critical_zoomer::delta_gear::{f64_step, scaled_f64_step, scaled_scale_from_dz, ComputeGear};
use critical_zoomer::floatexp::{ComplexFloatExp, FloatExp};
use critical_zoomer::reference::ReferenceOrbit;
use critical_zoomer::utils::{IntExp, ObjectivePosAndZoom};

fn run_big<R: Send + 'static>(f: impl FnOnce() -> R + Send + 'static) -> R {
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap()
}

fn home_frame() -> (ObjectivePosAndZoom, (u32, u32)) {
    (
        ObjectivePosAndZoom {
            pos: (
                IntExp::from(HOME_POSITION.0),
                IntExp::from(HOME_POSITION.1),
            ),
            zoom_pot: HOME_POSITION.2,
        },
        DEFAULT_WINDOW_RES,
    )
}

fn home_context_f64() -> WorkContext<f64> {
    from_stencil(home_frame(), None).expect("home f64")
}

fn home_context_f64_with_reference() -> WorkContext<f64> {
    let frame = home_frame();
    let req = select_reference_request::<f64>(None, &frame);
    let mut ctx = from_stencil(frame, None).expect("home f64");
    let orbit = ReferenceOrbit::compute(&req.c, req.precision_bits, 4096);
    ctx.latest_reference = Some(Arc::new(PublishedReference {
        orbit,
        c: req.c,
        generation: 1,
    }));
    ctx
}

fn home_context_floatexp() -> WorkContext<FloatExp> {
    from_stencil(home_frame(), None).expect("home FloatExp")
}

fn drain_f64(ctx: &mut WorkContext<f64>) -> usize {
    let mut n = 0;
    while ctx.completed_points.try_pop().is_some() {
        n += 1;
    }
    n
}

fn drain_fe(ctx: &mut WorkContext<FloatExp>) -> usize {
    let mut n = 0;
    while ctx.completed_points.try_pop().is_some() {
        n += 1;
    }
    n
}

fn frame_complete_f64(ctx: &WorkContext<f64>) -> bool {
    ctx.points.iter().all(|p| p.delivered)
}

fn frame_complete_fe(ctx: &WorkContext<FloatExp>) -> bool {
    ctx.points.iter().all(|p| p.delivered)
}

fn fill_f64(ctx: &mut WorkContext<f64>) {
    workshift(0, 0, 0, 0, ctx, None);
}

fn fill_direct(ctx: &mut WorkContext<f64>) {
    workshift_with_kernel(0, 0, 0, 0, ctx, &DirectKernel);
}

fn fill_floatexp(ctx: &mut WorkContext<FloatExp>) {
    workshift_with_kernel(0, 0, 0, 0, ctx, &FloatExpPerturbationKernel);
}

/// Criterion `b.iter` times the whole closure. Returning a `Duration` does **not**
/// make Criterion use that duration — so prior `time_to_*` numbers accidentally
/// included `from_stencil` + `run_big` thread spawn. Use `iter_custom` so the
/// reported median is only fill→publish wall time (matches the pin tests).
fn time_to_first_publish(c: &mut Criterion) {
    let mut group = c.benchmark_group("workgroup_fitness");
    group.sample_size(30);
    group.bench_function("time_to_first_publish", |b| {
        b.iter_custom(|iters| {
            let mut total = std::time::Duration::ZERO;
            for _ in 0..iters {
                total += run_big(|| {
                    let mut ctx = home_context_f64();
                    let start = Instant::now();
                    let first = loop {
                        fill_f64(&mut ctx);
                        let got = drain_f64(&mut ctx);
                        if got > 0 {
                            break got;
                        }
                    };
                    black_box(first);
                    start.elapsed()
                });
            }
            total
        });
    });
    group.finish();
}

fn time_to_full_frame(c: &mut Criterion) {
    let mut group = c.benchmark_group("workgroup_fitness");
    group.sample_size(10);
    group.bench_function("time_to_full_frame", |b| {
        b.iter_custom(|iters| {
            let mut total = std::time::Duration::ZERO;
            for _ in 0..iters {
                total += run_big(|| {
                    let mut ctx = home_context_f64();
                    let start = Instant::now();
                    let mut shifts = 0u32;
                    loop {
                        fill_f64(&mut ctx);
                        drain_f64(&mut ctx);
                        shifts += 1;
                        if frame_complete_f64(&ctx) {
                            break;
                        }
                        if shifts > 200_000 {
                            panic!("home frame did not complete");
                        }
                    }
                    let elapsed = start.elapsed();
                    let ips = ctx.total_iterations as f64 / elapsed.as_secs_f64();
                    println!(
                        "full_stack_ips_f64_gear: {:.0}  ({} iterations, {} shifts, {:.2?}) gear={:?}",
                        ips,
                        ctx.total_iterations,
                        shifts,
                        elapsed,
                        ctx.active_gear
                    );
                    elapsed
                });
            }
            total
        });
    });
    group.finish();
}

fn time_to_full_frame_with_reference(c: &mut Criterion) {
    let mut group = c.benchmark_group("workgroup_fitness");
    group.sample_size(10);
    group.bench_function("time_to_full_frame_with_reference", |b| {
        b.iter_custom(|iters| {
            let mut total = std::time::Duration::ZERO;
            for _ in 0..iters {
                total += run_big(|| {
                    let mut ctx = home_context_f64_with_reference();
                    let start = Instant::now();
                    let mut shifts = 0u32;
                    loop {
                        fill_f64(&mut ctx);
                        drain_f64(&mut ctx);
                        shifts += 1;
                        if frame_complete_f64(&ctx) {
                            break;
                        }
                        if shifts > 200_000 {
                            panic!("home frame with reference did not complete");
                        }
                    }
                    let elapsed = start.elapsed();
                    let ips = ctx.total_iterations as f64 / elapsed.as_secs_f64();
                    println!(
                        "full_stack_ips_f64_gear_ref: {:.0}  ({} iterations, {} shifts, {:.2?})",
                        ips, ctx.total_iterations, shifts, elapsed
                    );
                    elapsed
                });
            }
            total
        });
    });
    group.finish();
}

fn time_to_full_frame_direct_oracle(c: &mut Criterion) {
    let mut group = c.benchmark_group("workgroup_fitness");
    group.sample_size(10);
    group.bench_function("time_to_full_frame_direct_oracle", |b| {
        b.iter_custom(|iters| {
            let mut total = std::time::Duration::ZERO;
            for _ in 0..iters {
                total += run_big(|| {
                    let mut ctx = home_context_f64();
                    let start = Instant::now();
                    let mut shifts = 0u32;
                    loop {
                        fill_direct(&mut ctx);
                        drain_f64(&mut ctx);
                        shifts += 1;
                        if frame_complete_f64(&ctx) {
                            break;
                        }
                        if shifts > 200_000 {
                            panic!("direct home frame did not complete");
                        }
                    }
                    let elapsed = start.elapsed();
                    println!(
                        "full_stack_ips_direct_f64: shifts={} elapsed={:.2?}",
                        shifts, elapsed
                    );
                    elapsed
                });
            }
            total
        });
    });
    group.finish();
}

fn worker_1080p_full_frame(c: &mut Criterion) {
    let mut group = c.benchmark_group("workgroup_resolution");
    group.sample_size(10);
    group.bench_function("worker_1080p_full_frame", |b| {
        b.iter_custom(|iters| {
            let mut total = std::time::Duration::ZERO;
            for _ in 0..iters {
                total += run_big(|| {
                    let mut ctx = from_stencil((home_frame().0, (1920, 1080)), None).expect("1080p");
                    let start = Instant::now();
                    let mut shifts = 0u32;
                    while !frame_complete_f64(&ctx) {
                        fill_f64(&mut ctx);
                        drain_f64(&mut ctx);
                        shifts += 1;
                        if shifts > 1_000_000 {
                            panic!("1080p frame did not complete");
                        }
                    }
                    black_box(shifts);
                    start.elapsed()
                });
            }
            total
        });
    });
    group.finish();
}

/// Microbench: scaled-f64 vs full FloatExp for a deep delta step train.
fn scaled_vs_floatexp_steps(c: &mut Criterion) {
    let mut group = c.benchmark_group("gear_micro");
    group.sample_size(50);
    let z_ref = (0.25f64, 0.1f64);
    let dz0 = ComplexFloatExp::new(FloatExp::from(1e-200), FloatExp::ZERO);
    let dc = dz0;
    group.bench_function("scaled_f64_1k_steps", |b| {
        b.iter(|| {
            let mut scale = scaled_scale_from_dz(dz0);
            let s0 = scale.to_f64();
            let mut w = (dz0.re.to_f64() / s0, 0.0);
            let d = (dc.re.to_f64() / s0, 0.0);
            for _ in 0..1000 {
                let (wn, sc, gear) = scaled_f64_step(z_ref, w, d, scale, false);
                if gear != ComputeGear::ScaledF64 {
                    break;
                }
                w = wn;
                scale = sc;
            }
            black_box((w, scale))
        });
    });
    group.bench_function("floatexp_1k_steps", |b| {
        b.iter(|| {
            let mut dz = dz0;
            let two = ComplexFloatExp::new(FloatExp::TWO, FloatExp::ZERO);
            let z_fe = ComplexFloatExp::new(FloatExp::from(z_ref.0), FloatExp::from(z_ref.1));
            for _ in 0..1000 {
                dz = z_fe * dz * two + dz * dz + dc;
            }
            black_box(dz)
        });
    });
    group.bench_function("f64_1k_steps", |b| {
        b.iter(|| {
            let mut dz = (1e-8, 0.0);
            let dc = (1e-8, 0.0);
            let mut dd = (1.0, 0.0);
            for _ in 0..1000 {
                let (dzn, ddn, _) = f64_step(z_ref, dz, dc, dd, false);
                dz = dzn;
                dd = ddn;
            }
            black_box((dz, dd))
        });
    });
    group.finish();
}

/// Probe: CPU DirectKernel IPS vs Naive GPU IPS on a small home shell (adapter-gated).
/// Method note: compare to measured FLOP ratio (D-NGPU-5); absolute billions suspended.
fn naive_gpu_ips_probe(c: &mut Criterion) {
    use critical_zoomer::assemblies::workgroup::screen_worker::naive_gpu::{
        workshift_naive_gpu, NaiveGpuContext,
    };
    let Some(mut gpu) = NaiveGpuContext::try_new() else {
        eprintln!("naive_gpu_ips_probe: no adapter — skipped");
        return;
    };
    gpu.set_wave_n(1024);
    let mut group = c.benchmark_group("naive_gpu_ips");
    group.sample_size(20);
    group.bench_function("cpu_direct_home_shift", |b| {
        b.iter(|| {
            let mut ctx = home_context_f64();
            fill_direct(&mut ctx);
            black_box(ctx.total_iterations_today)
        });
    });
    group.bench_function("gpu_naive_home_shift", |b| {
        b.iter(|| {
            let mut ctx = home_context_f64();
            workshift_naive_gpu(0, 0, 0, 0, &mut ctx, &mut gpu);
            black_box(ctx.total_iterations_today)
        });
    });
    group.finish();
}

criterion_group!(
    benches,
    time_to_first_publish,
    time_to_full_frame,
    time_to_full_frame_with_reference,
    time_to_full_frame_direct_oracle,
    worker_1080p_full_frame,
    scaled_vs_floatexp_steps,
    naive_gpu_ips_probe
);
criterion_main!(benches);
