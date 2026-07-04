use std::ops::*;
use rand::*;
use num_traits::*;
use std::fmt::*;

use rand::distr::uniform::{SampleRange, SampleUniform};

pub trait Value:
PartialOrd
+ Copy
+ Debug
{}

impl<T> Value for T
where
    T: PartialOrd
    + Copy
    + Debug
{}

// special min and max which propagate NAN values to conserve ignorance
fn min<T: PartialOrd>(a: T, b: T) -> T {
    if a==a {} else {return a}
    if b==b {} else {return b}
    if a < b {a} else {b}
}
fn max<T: PartialOrd>(a: T, b: T) -> T {
    if a==a {} else {return a}
    if b==b {} else {return b}
    if a > b {a} else {b}
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Range<T: Value> {
    pub lower_bound: T
    , pub upper_bound: T
}

impl<T: Value> Range<T> {
    fn new(value: T) -> Self {
        Range {
            lower_bound: value
            , upper_bound: value
        }
    }

    fn result(lower_bound: T, upper_bound: T) -> Range::<T> {
        let returned:Range::<T> = {
            Range::<T> {
                lower_bound: lower_bound
                , upper_bound: upper_bound
            }
        };
        println!("new Range: {:?}", returned);
        returned
    }

    pub fn is_agnostic(&self) -> bool {
        self.lower_bound != self.lower_bound || self.upper_bound != self.upper_bound
    }

    fn choose<const N:usize>(options:[T;N]) -> Range<T> {
        assert!(N>0);
        Range::<T>::result(
            {
                let mut lower = options[0];
                for n in options {lower = min(lower, n)}
                lower
            }, {
                let mut upper = options[0];
                for n in options {upper = max(upper, n)}
                upper
            }
        )
    }

    pub fn can_eq(&self, other:Self) -> bool {
        self.lower_bound <= other.upper_bound && self.upper_bound >= other.lower_bound
    }

    fn must_eq(&self, other:Self) -> bool {
        self.lower_bound == self.upper_bound
            && other.lower_bound == other.upper_bound
            && self.lower_bound == other.lower_bound
    }

    fn can_ne(&self, other:Self) -> bool {
        self.lower_bound != self.upper_bound || other.lower_bound != other.upper_bound
            || self.lower_bound != other.lower_bound || self.upper_bound != other.upper_bound
    }

    fn must_ne(&self, other:Self) -> bool {
        self.lower_bound > other.upper_bound || self.upper_bound < other.lower_bound
    }

    fn can_lt (&self, other:Self) -> bool {
        self.lower_bound < other.upper_bound
    }

    pub fn must_lt (&self, other:Self) -> bool {
        self.upper_bound < other.lower_bound
    }

    fn can_gt (&self, other:Self) -> bool {
        self.upper_bound > other.lower_bound
    }

    pub fn must_gt (&self, other:Self) -> bool {
        self.lower_bound > other.upper_bound
    }



    pub fn guess_left(self) -> T {
        self.lower_bound
    }

    pub fn guess_right(self) -> T {
        self.upper_bound
    }


    pub fn guess_biased(self, bias: T) -> T {
        if self.lower_bound <= bias && self.upper_bound >= bias {
            return bias
        }
        if self.lower_bound > bias {return self.lower_bound}
        if self.upper_bound < bias {return self.upper_bound}
        bias
    }
}

fn get_uuid() -> u64 {
    let mut rng = rand::rng();
    let random_number: u64 = rng.random();
    random_number
}

#[test]
fn test_must_gt_logic() {
    let a = Range::<f64> { lower_bound: 10.0, upper_bound: 11.0 };
    let b = Range::<f64> { lower_bound: 5.0, upper_bound: 6.0 };
    assert!(a.must_gt(b));
    assert!(!b.must_gt(a));
}

#[test]
fn test_can_eq_overlap() {
    let a = Range::<f64> { lower_bound: 1.0, upper_bound: 10.0 };
    let b = Range::<f64> { lower_bound: 5.0, upper_bound: 15.0 };
    assert!(a.can_eq(b));
}

#[test]
fn test_must_ne_separation() {
    let a = Range::<f64> { lower_bound: 1.0, upper_bound: 2.0 };
    let b = Range::<f64> { lower_bound: 3.0, upper_bound: 4.0 };
    assert!(a.must_ne(b));
}
