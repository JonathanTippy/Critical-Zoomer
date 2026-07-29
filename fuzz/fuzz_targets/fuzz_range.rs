#![no_main]

use critical_zoomer::range::Range;
use libfuzzer_sys::fuzz_target;

fn take_f64(data: &mut &[u8]) -> Option<f64> {
    if data.len() < 8 {
        return None;
    }
    let (head, rest) = data.split_at(8);
    *data = rest;
    Some(f64::from_le_bytes(head.try_into().ok()?))
}

fuzz_target!(|data: &[u8]| {
    let mut data = data;
    let Some(lo) = take_f64(&mut data) else { return };
    let Some(hi) = take_f64(&mut data) else { return };
    let Some(bias) = take_f64(&mut data) else { return };
    if !lo.is_finite() || !hi.is_finite() || !bias.is_finite() {
        return;
    }
    let (lower, upper) = if lo <= hi { (lo, hi) } else { (hi, lo) };
    let range = Range {
        lower_bound: lower,
        upper_bound: upper,
    };
    let g = range.guess_biased(bias);
    assert!(g >= lower && g <= upper, "guess_biased must clamp into [{lower}, {upper}], got {g}");
});
