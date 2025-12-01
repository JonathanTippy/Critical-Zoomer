use rug::*;
use std::cmp::*;
use std::ops::*;

#[inline]
pub(crate) fn zoom_from_pot(zoom: i64) -> f64 {
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
}