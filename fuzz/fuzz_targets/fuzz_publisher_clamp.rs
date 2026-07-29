#![no_main]

use critical_zoomer::assemblies::structs::{Answer, MandelbrotResult};
use critical_zoomer::assemblies::workgroup::structs::{
    CalibratedAnswer, CalibratedHighlights, CalibratedMandelbrotResult,
};
use critical_zoomer::assemblies::workgroup::tile_publisher::{
    agnostic_wide, exact_outside, publish_seat,
};
use critical_zoomer::range::Range;
use libfuzzer_sys::fuzz_target;

fn take_u64(data: &mut &[u8]) -> Option<u64> {
    if data.len() < 8 {
        return None;
    }
    let (head, rest) = data.split_at(8);
    *data = rest;
    Some(u64::from_le_bytes(head.try_into().ok()?))
}

fn false_hl() -> CalibratedHighlights {
    let f = Range {
        lower_bound: false,
        upper_bound: false,
    };
    CalibratedHighlights {
        in_filament: f,
        out_filament: f,
        small_time_edge: f,
        node: f,
    }
}

fuzz_target!(|data: &[u8]| {
    let mut data = data;
    let Some(lo) = take_u64(&mut data) else { return };
    let Some(hi) = take_u64(&mut data) else { return };
    let Some(bias_esc) = take_u64(&mut data) else { return };
    let (lower, upper) = if lo <= hi { (lo, hi) } else { (hi, lo) };
    let cal = CalibratedAnswer {
        result: CalibratedMandelbrotResult::Outside {
            escape_time_r2: Range {
                lower_bound: lower,
                upper_bound: upper,
            },
            escape_z: (
                Range {
                    lower_bound: 2.0,
                    upper_bound: 2.0,
                },
                Range {
                    lower_bound: 0.0,
                    upper_bound: 0.0,
                },
            ),
        },
        min_magnitude_time: Range {
            lower_bound: 0,
            upper_bound: 0,
        },
        min_magnitude: Range {
            lower_bound: 1.0,
            upper_bound: 1.0,
        },
        highlights: false_hl(),
        escape_time_angle: 0,
        min_magnitude_angle: 0,
    };
    let bias = Answer {
        result: MandelbrotResult::Outside {
            escape_time_r2: bias_esc,
            escape_z: (2.0, 0.0),
        },
        min_magnitude_time: 0,
        min_magnitude: 1.0,
        escape_time_angle: 0,
        min_magnitude_angle: 0,
    };
    let out = publish_seat(cal, Some(bias));
    if let MandelbrotResult::Outside { escape_time_r2, .. } = out.result {
        assert!(
            escape_time_r2 >= lower && escape_time_r2 <= upper,
            "publisher clamp must stay in [{lower},{upper}], got {escape_time_r2}"
        );
    }
    let _ = publish_seat(agnostic_wide(), None);
    let _ = publish_seat(exact_outside(10), Some(bias));
});
