use rug::*;
use std::cmp::*;
use std::ops::*;
use crate::utils::*;
use crate::constants::*;


use std::cmp::{min, Ordering};
use std::cmp::Ordering::{Equal, Greater, Less};
use std::ops::{Add, Mul, Shl, Shr, Sub};

#[derive(Clone, Debug, Ord, Eq)]
pub struct IntExp {
    pub val: Integer
    ,
    pub exp: i32
}

impl IntExp {
    pub const ZERO: IntExp = IntExp { val: Integer::ZERO, exp: 0 };

    pub fn shift(self, exp: i32) -> IntExp {
        if exp >= 0 {
            return self << exp as u32;
        } else {
            return self >> (-exp) as u32;
        }
    }
    pub fn round(self, bits: usize) -> IntExp {
        IntExp {
            val: self.val >> 1
            ,
            exp: self.exp + 1
        }
    }
    pub fn set_precision(self, POT: i32) -> IntExp {
        match (-self.exp).cmp(&POT) {
            Equal => {
                self
            }
            Greater => {
                IntExp {
                    val: self.val >> (-self.exp - POT)
                    ,
                    exp: -POT
                }
            }
            Less => {
                IntExp {
                    val: self.val << -(-self.exp - POT)
                    ,
                    exp: -POT
                }
            }
        }
    }
}


impl From<i32> for IntExp {
    fn from(value: i32) -> Self {
        Self { val: Integer::from(value), exp: 0 }
    }
}

impl Into<i32> for IntExp {
    fn into(self) -> i32 {
        (self.val.shift(self.exp)).to_i32_wrapping()
    }
}
/*
impl From<IntExp> for f64 {
    fn from(a: IntExp) -> f64 {
        a.val.to_f64() * 2.0f64.powf(a.exp as f64)
    }
}
*/
/*impl Into<f64> for IntExp {
    fn into(self) -> f64 {
        self.val.to_f64() * 2.0f64.powf(self.exp as f64)
    }
}*/
impl IntExp {
    pub fn to_f64(self) -> f64 {
        self.val.to_f64() * 2.0f64.powf(self.exp as f64)
    }

    pub fn to_f32(self) -> f32 {
        self.val.to_f32() * 2.0f32.powf(self.exp as f32)
    }
}
impl Into<isize> for IntExp {
    fn into(self) -> isize {
        if self.val > Integer::from(isize::MAX) {
            return isize::MAX
        }
        if self.val < Integer::from(isize::MIN) {
            return isize::MIN
        }
        self.val.shift(self.exp)
            .to_isize().unwrap()
    }
}

impl From<isize> for IntExp {
    fn from(val: isize) -> IntExp {
        IntExp { val: Integer::from(val), exp: 0 }
    }
}

impl From<usize> for IntExp {
    fn from(value: usize) -> IntExp {
        IntExp {
            val: Integer::from(value)
            ,
            exp: 1
        }
    }
}

impl PartialEq for IntExp {
    fn eq(&self, other: &Self) -> bool {
        (self.clone() - other.clone()).val == 0
    }
}

impl PartialOrd for IntExp {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        None
    }
    fn lt(&self, other: &Self) -> bool {
        (self.clone() - other.clone()).val < 0
    }
    fn gt(&self, other: &Self) -> bool {
        (self.clone() - other.clone()).val > 0
    }
    fn le(&self, other: &Self) -> bool {
        !((self.clone() - other.clone()).val > 0)
    }
    fn ge(&self, other: &Self) -> bool {
        !((self.clone() - other.clone()).val < 0)
    }
}


impl Add for IntExp {
    type Output = Self;
    fn add(self, other: Self) -> Self {

        //let smallest_negative_exp = min(min(0, self.exp), other.exp);

        let smallest_exp = min(self.exp, other.exp);

        let self_shift = self.exp - smallest_exp;

        let other_shift = other.exp - smallest_exp;

        assert!(self_shift >= 0 && other_shift >= 0);

        let sum = (self.val << self_shift as u32) + (other.val << other_shift as u32);

        Self {
            val: sum
            ,
            exp: smallest_exp
        }
    }
}

impl Sub for IntExp {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        let smallest_exp = min(self.exp, other.exp);

        let self_shift = self.exp - smallest_exp;

        let other_shift = other.exp - smallest_exp;

        assert!(self_shift >= 0 && other_shift >= 0);

        let sum = (self.val << self_shift as u32) - (other.val << other_shift as u32);

        Self {
            val: sum
            ,
            exp: smallest_exp
        }
    }
}

impl Mul for IntExp {
    type Output = Self;
    fn mul(self, other: Self) -> Self {
        Self {
            val: self.val * other.val
            ,
            exp: self.exp + other.exp
        }
    }
}

impl Shl<u32> for IntExp {
    type Output = IntExp;

    fn shl(self, rhs: u32) -> Self::Output {
        Self {
            val: self.val
            ,
            exp: self.exp + rhs as i32
        }
    }
}

impl Shr<u32> for IntExp {
    type Output = IntExp;

    fn shr(self, rhs: u32) -> Self::Output {
        Self {
            val: self.val
            ,
            exp: self.exp - rhs as i32
        }
    }
}

impl std::fmt::Display for IntExp {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if self.val.significant_bits() > INTEXP_WARNING_SIZE {
            println!("WARNING: intexp passed warning size");
        }
        if self.exp >= 0 {
            f.write_str(&(self.val.clone() << self.exp as u32).to_string())?;
            Ok(())
        } else {
            f.write_str(&(self.val.clone() >> (-self.exp as u32)).to_string())?;
            f.write_str(".")?;
            f.write_str("...")?;
            Ok(())
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InexactIntExp;

impl TryFrom<IntExp> for f64 {
    type Error = InexactIntExp;

    fn try_from(e: IntExp) -> Result<f64, Self::Error> {
        const MAX_EXP: i32 = 1023;
        const MIN_EXP: i32 = -1074;
        const SIG_BITS: u32 = 53;

        if !e.val.is_zero() && (e.exp > MAX_EXP || e.exp < MIN_EXP) {
            return Err(InexactIntExp);
        }
        if e.val.significant_bits() > SIG_BITS {
            return Err(InexactIntExp);
        }
        Ok(e.to_f64())
    }
}

impl TryFrom<IntExp> for f32 {
    type Error = InexactIntExp;

    fn try_from(e: IntExp) -> Result<f32, Self::Error> {
        const MAX_EXP: i32 = 127;
        const MIN_EXP: i32 = -149;
        const SIG_BITS: u32 = 24;

        if !e.val.is_zero() && (e.exp > MAX_EXP || e.exp < MIN_EXP) {
            return Err(InexactIntExp);
        }
        if e.val.significant_bits() > SIG_BITS {
            return Err(InexactIntExp);
        }
        Ok(e.to_f32())
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