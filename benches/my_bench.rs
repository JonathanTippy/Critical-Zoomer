use std::hint::black_box;
use criterion::{criterion_group, criterion_main, Criterion};

use critical_zoomer::utils::*;
use critical_zoomer::assemblies::structs::*;

use egui::Color32;

fn HD_Color_frame(loc_desired: (IntExp, IntExp, i32), source: &View<Color32>) -> View<Color32> {
    let mut returned = black_box(View::new(
        (1920, 1080)
        , loc_desired.clone()
        , Color32::BLACK
    ));
    returned.fill_from(source);
    returned
}



fn HD_Color_Bench(c: &mut Criterion) {
    let source = View::new(
        (1, 1)
        , (IntExp::ZERO, IntExp::ZERO, 0).clone()
        , Color32::BLACK
    );


    c.bench_function(
        "hd 1"
        , |b| b
            .iter(|| HD_Color_frame(black_box((IntExp::ZERO, IntExp::ZERO, 0)), black_box(
                &source
            )))
    );
}

criterion_group! {
    name = benches;
    config = Criterion::default();
    targets = HD_Color_Bench
}
criterion_main!(benches);