#![allow(warnings)]


use std::hint::black_box;
use criterion::*;

use critical_zoomer::intexp::*;
use critical_zoomer::assemblies::structs::*;

use egui::Color32;

fn HD_Unit_frame(loc_desired: (IntExp, IntExp, i32), source: &View<()>) -> View<()> {
    let mut returned = black_box(View::new(
PointStencil{
            location: loc_desired.clone()
            , resolution: (1920, 1080)
            , serial_number : 0
            , focus: None, hover: None
        }
        , ()
    ));
    returned.fill_from(source);
    returned
}

fn HD_Color_frame(loc_desired: (IntExp, IntExp, i32), source: &View<Color32>) -> View<Color32> {
    let mut returned = black_box(View::new(
PointStencil{
            location: loc_desired.clone()
            , resolution: (1920, 1080)
            , serial_number : 0
            , focus: None, hover: None
        }
        , Color32::BLACK
    ));
    returned.fill_from(source);
    returned
}

fn HD_Answer_frame(loc_desired: (IntExp, IntExp, i32), source: &View<Answer>) -> View<Answer> {
    let mut returned = black_box(View::new(
PointStencil{
            location: loc_desired.clone()
            , resolution: (1920, 1080)
            , serial_number : 0
            , focus: None, hover: None
        }
        , Answer::TESTVAL
    ));
    returned.fill_from(source);
    returned
}



fn HD_Color_Bench(c: &mut Criterion) {
    let source = View::new(
        PointStencil{
            location: (IntExp::ZERO, IntExp::ZERO, 0)
            , resolution: (1, 1)
            , serial_number : 0
            , focus: None, hover: None
        }
        , Color32::BLACK
    );


    c.bench_function(
        "HD_Color_frame"
        , |b| b
            .iter_with_large_drop(|| HD_Color_frame(black_box((IntExp::ZERO, IntExp::ZERO, 0)), black_box(
                &source
            )))
    );
}

fn HD_Answer_Bench(c: &mut Criterion) {
    let source = View::new(
        PointStencil{
            location: (IntExp::ZERO, IntExp::ZERO, 0)
            , resolution: (1, 1)
            , serial_number : 0
            , focus: None, hover: None
        }
        , Answer::TESTVAL
    );


    c.bench_function(
        "HD_Answer_frame"
        , |b| b
            .iter_with_large_drop(|| HD_Answer_frame(black_box((IntExp::ZERO, IntExp::ZERO, 0)), black_box(
                &source
            )))
    );
}

fn HD_Unit_Bench(c: &mut Criterion) {
    let source = View::new(
        PointStencil{
            location: (IntExp::ZERO, IntExp::ZERO, 0)
            , resolution: (1, 1)
            , serial_number : 0
            , focus: None, hover: None
        }
        , ()
    );

    c.bench_function(
        "HD_Unit_frame"
        , |b| b
            .iter_with_large_drop(|| HD_Unit_frame(black_box((IntExp::ZERO, IntExp::ZERO, 0)), black_box(
                &source
            )))
    );
}

use std::time::*;

criterion_group! {
    name = benches;
    config = Criterion::default()
        .noise_threshold(0.10);
        //.warm_up_time(Duration::from_secs(5));
        //.significance_level(0.01)
        //.measurement_time(Duration::from_secs(30));
    targets = HD_Unit_Bench, HD_Answer_Bench, HD_Color_Bench
}
criterion_main!(benches);