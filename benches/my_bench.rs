use std::hint::black_box;
use criterion::{criterion_group, criterion_main, Criterion};

use critical_zoomer::utils::*;
use critical_zoomer::assemblies::structs::*;

fn HD_Color_frame(loc_desired: (IntExp, IntExp, i32), source: &View<Color>) -> View<Color> {
    let mut returned = black_box(View::new(
        (1920, 1080)
        , loc_desired.clone()
        , Color{rgb:(0,0,0)}
    ));
    returned.fill_from(source);
    returned
}


fn HD_Color_Bench_zoom_out(c: &mut Criterion) {
    let source = View::new(
        (1, 1)
        , (IntExp::ZERO, IntExp::ZERO, 0).clone()
        , Color { rgb: (0, 0, 0) }
    );


    c.bench_function(
        "hd 4"
        , |b| b
            .iter(|| HD_Color_frame(black_box((IntExp::ZERO, IntExp::ZERO, -1)), black_box(
                &source
            )))
    );
}

fn HD_Color_Bench_zoom_in(c: &mut Criterion) {
    let source = View::new(
        (1, 1)
        , (IntExp::ZERO, IntExp::ZERO, 0).clone()
        , Color { rgb: (0, 0, 0) }
    );


    c.bench_function(
        "hd 3"
        , |b| b
            .iter(|| HD_Color_frame(black_box((IntExp::ZERO, IntExp::ZERO, 1)), black_box(
                &source
            )))
    );
}


fn HD_Color_Bench_misses(c: &mut Criterion) {
    let source = View::new(
        (1, 1)
        , (IntExp::ZERO, IntExp::ZERO, 0).clone()
        , Color { rgb: (0, 0, 0) }
    );


    c.bench_function(
        "hd 1"
        , |b| b
            .iter(|| HD_Color_frame(black_box((IntExp::ZERO, IntExp::ZERO, 0)), black_box(
                &source
            )))
    );
}

fn HD_Color_Bench_hits(c: &mut Criterion) {
    let source = View::new(
        (1920, 1080)
        , (IntExp::ZERO, IntExp::ZERO, 0).clone()
        , Color { rgb: (0, 0, 0) }
    );


    c.bench_function(
        "hd 2"
        , |b| b
            .iter(|| HD_Color_frame(black_box((IntExp::ZERO, IntExp::ZERO, 0)), black_box(
                &source
            )))
    );
}

use std::time::*;

criterion_group! {
    name = benches;
    config = Criterion::default()
    //.sample_size(10)
    //.measurement_time(Duration::from_millis(1000))
    //.warm_up_time(Duration::from_millis(100))
    ;//.noise_threshold(0.10);
    targets = HD_Color_Bench_noalloc, HD_Color_Bench_misses, HD_Color_Bench_hits, HD_Color_Bench_zoom_in, HD_Color_Bench_zoom_out
}
criterion_main!(benches);