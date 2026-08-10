#![no_main]

use libfuzzer_sys::fuzz_target;

use critical_zoomer::assemblies::workgroup::c_generator::{admit_generator, pick_stack_admission};
use critical_zoomer::utils::IntExp;

// Pack fuzz bytes into a shallow admission probe. Must not panic — fail-closed
// `None` is fine; aborting on edge pots/res is not.
fuzz_target!(|data: &[u8]| {
    if data.len() < 12 {
        return;
    }
    let re_bits = i32::from_le_bytes(data[0..4].try_into().unwrap());
    let im_bits = i32::from_le_bytes(data[4..8].try_into().unwrap());
    let zoom = i64::from(data.get(8).copied().unwrap_or(0) as i8);
    let w = (data.get(9).copied().unwrap_or(2) as u32 % 64).max(1);
    let h = (data.get(10).copied().unwrap_or(2) as u32 % 64).max(1);
    let scale = (data.get(11).copied().unwrap_or(0) as i32 % 40) - 20;
    let re = IntExp::from(re_bits.rem_euclid(1_000_000)).shift(scale);
    let im = IntExp::from(im_bits.rem_euclid(1_000_000)).shift(scale);
    let loc = (re.clone(), im.clone());
    let center = (re, im);
    let _ = admit_generator::<f64>(&loc, zoom, (w, h), None, &center);
    let _ = pick_stack_admission(&loc, zoom, (w, h), None, &center);
});
