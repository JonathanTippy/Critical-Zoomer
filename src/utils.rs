use rug::*;
use std::cmp::*;
use std::ops::*;
use crate::intexp::*;

use crate::constants::*;

pub const INTEXP_WARNING_SIZE:u32 = 100;

#[inline]
pub fn zoom_from_pot(zoom: i32) -> f64 {
    if zoom > 0 {(1 << zoom) as f64} else {1.0 / (1<<-zoom) as f64}
}

#[inline]
pub fn signed_shift(input: i32, shift: i64) -> i32 {
    (input << ((shift + (shift.abs()))>>1)) >> (-((shift - (shift.abs()))>>1))
    /*if shift >= 0 {
        input << shift
    } else {
        input >> (-shift)
    }*/
}

#[inline]
pub fn shift(input:i32, shift:i32) -> i32 {
    if shift >= 0 {
        input << shift as u32
    } else {
        input >> (-shift) as u32
    }
}

/*#[inline]
pub fn shift_signed_assume_left(input: i32, shift: i64) -> i32 {
    if shift >= 0 {
        input
    }
}*/

#[derive(Clone, Debug, PartialEq)]
pub struct ObjectivePosAndZoom {
    pub pos: (IntExp, IntExp)
    , pub zoom_pot: i32
}




use std::cmp::Ordering::*;



pub trait Shiftable {
    fn shift(self, shift:i32) -> Self;
}

impl Shiftable for Integer {
    fn shift(self, shift:i32) -> Self {
        if shift >= 0 {
            self << shift as u32
        } else {
            self >> (-shift) as u32
        }
    }
}

impl Shiftable for f64 {
    fn shift(self, shift:i32) -> Self {
        self * zoom_from_pot(shift)
    }
}

pub fn f32_to_i16(input: f32) -> i16 {
    let p = input * (2<<12) as f32;
    p as i16
}

pub fn i16_to_f32(input: i16) -> f32 {
    let p:f32 = input as f32 / (2<<12) as f32;
    p
}

#[inline]
pub fn index_from_pos(pos:&(i32, i32), wid:u32) -> usize {
    (pos.0 + pos.1*wid as i32) as usize
}

#[inline]
pub fn index_from_pos_safe(pos:&(i32, i32), res:(u32, u32)) -> Option<usize> {

    let valid = (
        res.0 as i32 > pos.0 && pos.0 >= 0
        && res.1 as i32 > pos.1 && pos.1 >= 0
    );

    if valid {
        Some((pos.0 + pos.1*res.0 as i32) as usize)
    } else {None}
}

pub fn pos_from_index(i: usize, wid:u32) -> (i32, i32) {
    (i as i32 % wid as i32, i as i32/wid as i32)
}

const fn init (i:usize) -> u8 { i as u8 }

const ALL_U8S: [u8; 256] = {
    let mut returned = [0;256];
    let mut i = 0;
    while i < 256 {
        returned[i] = i as u8;
        i+=1
    }
    returned
};



impl Default for ObjectivePosAndZoom {
    fn default() -> Self {
        HOME_POSITION.into()
    }
}

impl From<(i32, i32, i32)> for ObjectivePosAndZoom {
    fn from(input:(i32, i32, i32)) -> ObjectivePosAndZoom {
        ObjectivePosAndZoom {
            pos: (IntExp::from(input.0), IntExp::from(input.1))
            , zoom_pot: input.2
        }
    }
}

#[test]
fn test_intexp_speed() {
    let mut rand = rand::RandState::new();
    let a = std::time::Instant::now();

    let mut int = Integer::from(Integer::random_bits(3600000, &mut rand));


    //let mut int = Integer::u_pow_u(2, 36000000).complete();

    println!("creating int took {} milliseconds", a.elapsed().as_millis());
    let a = std::time::Instant::now();

    int -= 1;
    println!("subtracting 1 took {} milliseconds", a.elapsed().as_millis());

    let mut test_val = IntExp { val: int, exp: -3600000 };

    let a = std::time::Instant::now();


    test_val = test_val - IntExp::from(1);
    println!("subtracting 1 took {} milliseconds", a.elapsed().as_millis());
    let a = std::time::Instant::now();

    test_val = test_val * IntExp::from(2);
    println!("multiplying by 2 took {} milliseconds", a.elapsed().as_millis());
}