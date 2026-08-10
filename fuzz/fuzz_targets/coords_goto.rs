#![no_main]

use libfuzzer_sys::fuzz_target;

use critical_zoomer::assemblies::headgroup::window::coords::{
    commands_from_goto_line, goto_line_is_valid, parse_complex,
};

// Arbitrary UTF-8 must never panic the goto / location parsers.
fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else {
        return;
    };
    let _ = goto_line_is_valid(s);
    let _ = commands_from_goto_line(s);
    let _ = parse_complex(s);
});
