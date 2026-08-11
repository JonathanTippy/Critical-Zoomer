// Shadergroup fitness — escaper vs colorer wall cost vs pixel count.
// Baselines: docs/assistant/benchmarks.md
//
// Run:
//   taskset -c 3-8 nice -n 15 cargo bench --bench shadergroup_fitness

#![allow(warnings)]

use std::hint::black_box;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use criterion::*;

use critical_zoomer::assemblies::shadergroup::colorer::color::color;
use critical_zoomer::assemblies::shadergroup::escaper::{escape_frame, ZoomerValuesScreen};
use critical_zoomer::assemblies::workgroup::screen_worker::workshift::*;
use critical_zoomer::assemblies::workgroup::work_collector::ResultsPackage;
use critical_zoomer::constants::{DEFAULT_WINDOW_RES, HOME_POSITION};
use critical_zoomer::settings::{Settings, DEFAULT_COLORING_SCRIPT};
use critical_zoomer::utils::{IntExp, ObjectivePosAndZoom};

fn run_big<R: Send + 'static>(f: impl FnOnce() -> R + Send + 'static) -> R {
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap()
}

/// Resolution with approximately `scale` × default pixel count (same aspect).
fn scaled_res(pixel_scale: f64) -> (u32, u32) {
    let linear = pixel_scale.sqrt();
    let w = ((DEFAULT_WINDOW_RES.0 as f64) * linear).round().max(1.0) as u32;
    let h = ((DEFAULT_WINDOW_RES.1 as f64) * linear).round().max(1.0) as u32;
    (w, h)
}

fn home_frame(res: (u32, u32)) -> (ObjectivePosAndZoom, (u32, u32)) {
    (
        ObjectivePosAndZoom {
            pos: (
                IntExp::from(HOME_POSITION.0),
                IntExp::from(HOME_POSITION.1),
            ),
            zoom_pot: HOME_POSITION.2,
        },
        res,
    )
}

fn fill_package(res: (u32, u32)) -> ResultsPackage<f64> {
    run_big(move || {
        let mut ctx = from_stencil(home_frame(res), None).expect("home");
        while !ctx.points.iter().all(|p| p.delivered) {
            workshift_with_kernel(0, 0, 0, 0, &mut ctx, &DirectKernel);
            while ctx.completed_points.pop().is_some() {}
        }
        let mut results = Vec::with_capacity(ctx.points.len());
        for p in &ctx.points {
            results.push(if p.repeats {
                CompletedPoint::Repeats {
                    period: p.period,
                    smallness: p.smallness_squared,
                    small_time: p.small_time,
                }
            } else if p.escapes {
                CompletedPoint::Escapes {
                    escape_time: p.iterations,
                    escape_location: (p.z.0, p.z.1),
                    escape_derivative: p.dc,
                    start_location: (p.c.0, p.c.1),
                    smallness: p.smallness_squared,
                    small_time: p.small_time,
                }
            } else {
                CompletedPoint::Dummy {}
            });
        }
        ResultsPackage {
            results,
            screen_res: res,
            location: home_frame(res).0,
            hud: Default::default(),
        }
    })
}

fn settings_default() -> Settings {
    let mut s = Settings::DEFAULT;
    s.coloring_script = Some(DEFAULT_COLORING_SCRIPT.to_vec());
    s
}

fn package_cache(pixel_scale: f64) -> &'static ResultsPackage<f64> {
    // Separate caches so Criterion groups don't fight over one OnceLock.
    match (pixel_scale * 100.0).round() as i32 {
        100 => {
            static P: OnceLock<ResultsPackage<f64>> = OnceLock::new();
            P.get_or_init(|| fill_package(scaled_res(1.0)))
        }
        150 => {
            static P: OnceLock<ResultsPackage<f64>> = OnceLock::new();
            P.get_or_init(|| fill_package(scaled_res(1.5)))
        }
        200 => {
            static P: OnceLock<ResultsPackage<f64>> = OnceLock::new();
            P.get_or_init(|| fill_package(scaled_res(2.0)))
        }
        _ => panic!("unsupported pixel_scale {pixel_scale}"),
    }
}

fn bench_escape(c: &mut Criterion, name: &str, pixel_scale: f64) {
    let pkg = package_cache(pixel_scale);
    let settings = settings_default();
    let radius = settings.bailout_radius.clone().determine() as f32;
    let pixels = pkg.results.len();
    eprintln!(
        "shadergroup escape {name}: res={:?} pixels={pixels} (~{pixel_scale}× default)",
        pkg.screen_res
    );

    c.bench_function(name, |b| {
        b.iter_custom(|iters| {
            let mut total = Duration::ZERO;
            for _ in 0..iters {
                let t0 = Instant::now();
                let screen = escape_frame(pkg, radius, &settings);
                black_box(screen);
                total += t0.elapsed();
            }
            total
        })
    });
}

fn bench_color(c: &mut Criterion, name: &str, pixel_scale: f64) {
    let pkg = package_cache(pixel_scale);
    let settings = settings_default();
    let radius = settings.bailout_radius.clone().determine() as f32;
    let screen: ZoomerValuesScreen = escape_frame(pkg, radius, &settings);
    let pixels = screen.values.len();
    eprintln!(
        "shadergroup color {name}: res={:?} pixels={pixels} (~{pixel_scale}× default)",
        screen.res
    );

    c.bench_function(name, |b| {
        b.iter_custom(|iters| {
            let mut total = Duration::ZERO;
            for _ in 0..iters {
                let mut s = settings.clone();
                let t0 = Instant::now();
                let out = color(&screen, &mut s);
                black_box(out);
                total += t0.elapsed();
            }
            total
        })
    });
}

fn shadergroup_escape(c: &mut Criterion) {
    let mut group = c.benchmark_group("shadergroup_escape");
    group.sample_size(20);
    // Criterion API: use bench_function via helpers on `c` with distinct names.
    drop(group);
    bench_escape(c, "escape_1_0x_default_pixels", 1.0);
    bench_escape(c, "escape_1_5x_default_pixels", 1.5);
    bench_escape(c, "escape_2_0x_default_pixels", 2.0);
}

fn shadergroup_color(c: &mut Criterion) {
    bench_color(c, "color_1_0x_default_pixels", 1.0);
    bench_color(c, "color_1_5x_default_pixels", 1.5);
    bench_color(c, "color_2_0x_default_pixels", 2.0);
}

criterion_group!(benches, shadergroup_escape, shadergroup_color);
criterion_main!(benches);
