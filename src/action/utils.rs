use rug::*;
use std::cmp::*;
use std::ops::*;

pub(crate) const INTEXP_WARNING_SIZE:u32 = 100;

#[inline]
pub(crate) fn zoom_from_pot(zoom: i32) -> f64 {
    if zoom > 0 {(1 << zoom) as f64} else {1.0 / (1<<-zoom) as f64}
}

#[inline]
pub(crate) fn signed_shift(input: i32, shift: i64) -> i32 {
    (input << ((shift + (shift.abs()))>>1)) >> (-((shift - (shift.abs()))>>1))
    /*if shift >= 0 {
        input << shift
    } else {
        input >> (-shift)
    }*/
}

#[inline]
pub(crate) fn shift(input:i32, shift:i32) -> i32 {
    if shift >= 0 {
        input << shift as u32
    } else {
        input >> (-shift) as u32
    }
}

/*#[inline]
pub(crate) fn shift_signed_assume_left(input: i32, shift: i64) -> i32 {
    if shift >= 0 {
        input
    }
}*/

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ObjectivePosAndZoom {
    pub(crate) pos: (IntExp, IntExp)
    , pub(crate) zoom_pot: i32
}



#[derive(Clone, Debug, PartialEq)]
pub(crate) struct IntExp {
    pub(crate) val: Integer
    , pub(crate) exp: i32
}

impl Add for IntExp {
    type Output = Self;
    fn add(self, other:Self) -> Self {

        //let smallest_negative_exp = min(min(0, self.exp), other.exp);

        let smallest_exp = min(self.exp, other.exp);

        let self_shift = self.exp - smallest_exp;

        let other_shift = other.exp - smallest_exp;

        assert!(self_shift >= 0 && other_shift >= 0);

        let sum = (self.val << self_shift as u32) + (other.val << other_shift as u32);

        Self {
            val: sum
            , exp: smallest_exp
        }
    }
}

impl Sub for IntExp {
    type Output = Self;
    fn sub(self, other:Self) -> Self {

        let smallest_exp = min(self.exp, other.exp);

        let self_shift = self.exp - smallest_exp;

        let other_shift = other.exp - smallest_exp;

        assert!(self_shift >= 0 && other_shift >= 0);

        let sum = (self.val << self_shift as u32) - (other.val << other_shift as u32);

        Self {
            val: sum
            , exp: smallest_exp
        }
    }
}

impl Mul for IntExp {
    type Output = Self;
    fn mul(self, other:Self) -> Self {

        Self {
            val: self.val * other.val
            , exp: self.exp + other.exp
        }
    }
}

impl Shl<u32> for IntExp {
    type Output = IntExp;

    fn shl(self, rhs: u32) -> Self::Output {
        Self{
            val: self.val
            , exp: self.exp + rhs as i32
        }
    }
}

impl Shr<u32> for IntExp {
    type Output = IntExp;

    fn shr(self, rhs: u32) -> Self::Output {
        Self{
            val: self.val
            , exp: self.exp - rhs as i32
        }
    }
}

impl std::fmt::Display for IntExp {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {

        if self.val.significant_bits() > INTEXP_WARNING_SIZE {
            println!("WARMING: intexp passed warning size");
        }
        if self.exp >= 0 {
            f.write_str(&(self.val.clone()<<self.exp as u32).to_string())?;
            Ok(())
        } else {
            f.write_str(&(self.val.clone()>>(-self.exp as u32)).to_string())?;
            f.write_str(".")?;
            f.write_str("...")?;
            Ok(())
        }


    }
}


impl IntExp {
    pub(crate) fn shift(self, exp: i32) -> IntExp {
        if exp >= 0 {
            return self << exp as u32;
        } else {
            return self >> (-exp) as u32;
        }
    }
    pub(crate) fn round(self, bits: usize) -> IntExp {
        IntExp{
            val: self.val >> 1
            , exp: self.exp + 1
        }
    }
}


impl From<i32> for IntExp {
    fn from(value: i32) -> Self {
        Self{val:Integer::from(value), exp:0}
    }

}

impl Into<i32> for IntExp {
    fn into(self) -> i32 {
        (self.val.shift(self.exp)).to_i32_wrapping()
    }
}

impl Into<f64> for IntExp {
    fn into(self) -> f64 {
        self.val.to_f64() * 1.0.shift(self.exp)
    }
}

trait Shiftable {
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

pub(crate) fn f64_to_i16(input: f64) -> i16 {
    let p = input * (2<<12) as f64;
    p as i16
}

pub(crate) fn i16_to_f64(input: i16) -> f64 {
    let p:f64 = input as f64 / (2<<12) as f64;
    p
}