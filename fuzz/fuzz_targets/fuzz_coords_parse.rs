#![no_main]

use critical_zoomer::assemblies::headgroup::window::coords::parse_complex;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let s = String::from_utf8_lossy(data);
    // Must not panic on arbitrary UTF-8-ish input.
    let _ = parse_complex(&s);
    let _ = parse_complex(&s.trim());
    let decorated = format!("( {} )", s);
    let _ = parse_complex(&decorated);
});
