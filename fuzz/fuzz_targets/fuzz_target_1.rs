#![no_main]

use critical_zoomer::intexp::IntExp;
use libfuzzer_sys::fuzz_target;
use rug::Integer;

fn take_i32(data: &mut &[u8]) -> Option<i32> {
    if data.len() < 4 {
        return None;
    }
    let (head, rest) = data.split_at(4);
    *data = rest;
    Some(i32::from_le_bytes([head[0], head[1], head[2], head[3]]))
}

fn take_u8(data: &mut &[u8]) -> Option<u8> {
    if data.is_empty() {
        return None;
    }
    let v = data[0];
    *data = &data[1..];
    Some(v)
}

fn clamp_exp(e: i32) -> i32 {
    e.clamp(-64, 64)
}

fuzz_target!(|data: &[u8]| {
    let mut data = data;
    let Some(av) = take_i32(&mut data) else { return };
    let Some(ae) = take_i32(&mut data) else { return };
    let Some(bv) = take_i32(&mut data) else { return };
    let Some(be) = take_i32(&mut data) else { return };
    let Some(op) = take_u8(&mut data) else { return };

    let a = IntExp {
        val: Integer::from(av),
        exp: clamp_exp(ae),
    };
    let b = IntExp {
        val: Integer::from(bv),
        exp: clamp_exp(be),
    };

    match op % 4 {
        0 => {
            let left = a.clone() + b.clone();
            let right = b + a;
            assert_eq!(left, right, "IntExp add must be commutative");
        }
        1 => {
            let s = a.clone().shift(3);
            let t = a << 3u32;
            assert_eq!(s.val, t.val);
            assert_eq!(s.exp, t.exp);
        }
        2 => {
            let s = a.clone() << 2u32;
            assert_eq!(s.val, a.val);
            assert_eq!(s.exp, a.exp + 2);
        }
        _ => {
            let s = a.clone() >> 2u32;
            assert_eq!(s.val, a.val);
            assert_eq!(s.exp, a.exp - 2);
        }
    }
});
