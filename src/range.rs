use std::ops::*;
use rand::*;
use num_traits::*;
use std::fmt::*;

use rand::distr::uniform::{SampleRange, SampleUniform};

pub trait Value:
PartialOrd + Expand
+ Add<Output=Self> + Sub<Output=Self> + Mul<Output=Self>
+ Zero + One
+ Copy + Clone
+ Float
+ Debug
+ SampleUniform
{}

impl<T> Value for T
where
    T: PartialOrd + Expand
    + Add<Output=Self> + Sub<Output=Self> + Mul<Output=Self>
    + Zero + One
    + Copy + Clone
    + Float
    + Debug
    + SampleUniform
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

trait Expand {
    fn next_down(self) -> Self;
    fn next_up(self) -> Self;
}

impl Expand for f32 {
    fn next_down(self) -> Self {return f32::next_down(self)}
    fn next_up(self) -> Self {return f32::next_up(self)}
}
impl Expand for f64 {
    fn next_down(self) -> Self {return f64::next_down(self)}
    fn next_up(self) -> Self {return f64::next_up(self)}
}




#[derive(Debug, Clone, Copy)]
pub struct Range<T: Value, const MUST_INTEGER: bool> {
    pub lower_bound: T
    , pub upper_bound: T
}

impl<T: Value, const MUST_INTEGER:bool> Range<T, MUST_INTEGER> {
    fn new(value: T) -> Self {
        Range {
            lower_bound: value
            , upper_bound: value
        }
    }

    fn result(lower_bound: T, upper_bound: T) -> Range::<T, MUST_INTEGER> {
        let returned:Range::<T, MUST_INTEGER> = {
            if MUST_INTEGER {
                let lower_bound = lower_bound.next_down().ceil();
                let upper_bound = upper_bound.next_up().floor();
                if lower_bound > upper_bound {panic!("Must integer failed to integer")}
                Range {
                    lower_bound
                    , upper_bound
                }
            } else {
                Range::<T, MUST_INTEGER> {
                    lower_bound: lower_bound.next_down()
                    , upper_bound: upper_bound.next_up()

                }
            }
        };
        println!("new Range: {:?}", returned);
        returned
    }

    fn square(self) -> Self {
        if !self.can_be_zero() {
            Range::choose([
                                         self.lower_bound * self.lower_bound
                                         , self.upper_bound * self.upper_bound
                                     ])
        } else {
            Range::choose([
                                         self.lower_bound * self.lower_bound
                                         , self.upper_bound * self.upper_bound
                                         , T::zero()
                                     ])
        }
    }

    fn can_be_zero(&self) -> bool {
        self.lower_bound <= T::zero() && self.upper_bound >= T::zero()
    }

    pub fn is_agnostic(&self) -> bool {
        self.lower_bound != self.lower_bound || self.upper_bound != self.upper_bound
    }

    fn choose<const N:usize>(options:[T;N]) -> Range<T, MUST_INTEGER> {
        assert!(N>0);
        Range::<T, MUST_INTEGER>::result(
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

    fn ln (self) -> Self {
        Self::choose(
            [self.upper_bound.ln(), self.lower_bound.ln()]
        )
    }
    fn exp (self) -> Self {
        Self::choose(
            [self.upper_bound.exp(), self.lower_bound.exp()]
        )
    }

    fn guess_middle(self) -> T {
        if MUST_INTEGER {
            ((self.lower_bound + self.upper_bound) / (T::one() + T::one())).floor()
        } else {
            (self.lower_bound + self.upper_bound) / (T::one() + T::one())
        }
    }

    pub fn guess_left(self) -> T {
        if MUST_INTEGER {
            self.lower_bound.ceil()
        } else {
            self.lower_bound
        }
    }

    pub fn guess_right(self) -> T {
        if MUST_INTEGER {
            self.upper_bound.floor()
        } else {
            self.upper_bound
        }
    }

    fn guess_random(self) -> T {
        if self.is_agnostic() {
            return T::nan();
        }
        if self.guess_left() > self.guess_right() {
            panic!("bounds inverted");
        }
        if MUST_INTEGER {
            let mut rng = rand::rng();
            let random = rng.random_range(
                self.guess_left() - (T::one()/(T::one()+ T::one()))
                    ..=self.guess_right() + (T::one() / (T::one() + T::one()))
            );
            {if (random.floor() - random).abs() < (random.ceil() - random).abs() {
                random.floor()
            } else {
                random.ceil()
            }}.clamp(self.guess_left(), self.guess_right())
        } else {
            let mut rng = rand::rng();
            rng.random_range(self.lower_bound..=self.upper_bound)
        }
    }
}

fn get_uuid() -> u64 {
    let mut rng = rand::rng();
    let random_number: u64 = rng.random();
    random_number
}

impl<T: Value, const Int: bool> Add<Self> for Range<T, Int> {
    type Output = Self;
    fn add (self, other:Self) -> Self {
        Range::result(
            self.lower_bound + other.lower_bound
            , self.upper_bound + other.upper_bound
        )
    }
}
impl<T: Value, const Int: bool> Add<T> for Range<T, Int> {
    type Output = Self;
    fn add (self, other:T) -> Self {
        Range::result(
            self.lower_bound + other
            , self.upper_bound + other
        )
    }
}

impl<T: Value, const Int: bool> Sub<Self> for Range<T, Int> {
    type Output = Self;
    fn sub (self, other:Self) -> Self {
        Range::result (
            self.lower_bound - other.upper_bound
            , self.upper_bound - other.lower_bound
        )
    }
}
impl<T: Value, const Int: bool> Sub<T> for Range<T, Int> {
    type Output = Self;
    fn sub (self, other:T) -> Self {
        Range::result (
            self.lower_bound - other
            , self.upper_bound - other
        )
    }
}

impl<T: Value, const Int: bool> Mul<Self> for Range<T, Int> {
    type Output = Self;
    fn mul (self, other:Self) -> Self {
        Range::choose([
                                      self.lower_bound * other.lower_bound
                                      , self.lower_bound * other.upper_bound
                                      , self.upper_bound * other.lower_bound
                                      , self.upper_bound * other.upper_bound
                                  ])
    }
}
impl<T: Value, const Int: bool> Mul<T> for Range<T, Int> {
    type Output = Self;
    fn mul (self, other:T) -> Self {
        Range::choose([
                                     self.lower_bound * other
                                     , self.lower_bound * other
                                 ])
    }
}

fn comptest() {
    let mut  a = Range::<f64, false>::new(1.1);
    a = a + 1.1;
    //let c = rug::Float::with_val(10, 5);
    //let mut d = PartialKnowledge::new(c);
}


#[test]
fn int_test() {
    let mut a = Range::<f64, true>::new(5.0);
    let mut b = Range {lower_bound: 0.9, upper_bound: 1.1};
    assert!((a + b).must_eq(Range::new(6.0)));
}

#[test]
fn int_test_2() {
    let mut a = Range::<f64, true>::new(5.0);
    let mut b = Range {lower_bound: 0.9, upper_bound: 1.1};
    assert!((a * b).must_eq(Range::new(5.0)));
}

#[test]
fn test_addition() {
    let a = Range::<f64, true>::new(10.0);
    let b = Range::<f64, true>::new(5.0);
    let c = a + b;
    // Knowledge of exact values remain exact.
    assert!(c.can_eq(Range::new(15.0)));
}

#[test]
fn test_subtraction_range() {
    // [9, 11] - [1, 2] = [7, 10]
    let a = Range::<f64, false> { lower_bound: 9.0, upper_bound: 11.0 };
    let b = Range::<f64, false> { lower_bound: 1.0, upper_bound: 2.0 };
    let c = a - b;
    assert!(c.lower_bound <= 8.0); // account for next_down
    assert!(c.upper_bound >= 10.0); // account for next_up
}

#[test]
fn test_multiplication_by_zero() {
    let a = Range::<f64, false> { lower_bound: -100.0, upper_bound: 100.0 };
    let b = Range::new(0.0);
    let c = a * b;
    // Anything times zero is zero, despite ignorance of 'a'
    assert!(c.can_eq(Range::new(0.0)));
}

#[test]
fn test_integer_collapse_addition() {
    // [5.0, 5.0] + [0.1, 1.9] (must be integer)
    // Possible integers in [5.1, 6.9] is only 6.
    let a = Range::<f64, true>::new(5.0);
    let b = Range::<f64, true> { lower_bound: 0.1, upper_bound: 1.9};
    let c = a + b;
    assert!(c.must_eq(Range::new(6.0)));
}

#[test]
#[should_panic]
fn test_integer_collapse_multiplication() {
    // [2.0, 2.0] * [0.6, 0.9] (must be integer)
    // Range is [1.2, 1.8]. No integers exist.
    let a = Range::<f64, true>::new(2.0);
    let b = Range::<f64, true> { lower_bound: 0.6, upper_bound: 0.9};
    let c = a * b;
}

#[test]
fn test_square_negative_range() {
    // [-2, 3]^2 should be [0, 9]
    let a = Range::<f64, false> { lower_bound: -2.0, upper_bound: 3.0};
    let b = a.square();
    assert!(b.lower_bound <= 0.0);
    assert!(b.upper_bound >= 9.0);
}

#[test]
fn test_must_gt_logic() {
    let a = Range::<f64, false> { lower_bound: 10.0, upper_bound: 11.0 };
    let b = Range::<f64, false> { lower_bound: 5.0, upper_bound: 6.0 };
    assert!(a.must_gt(b));
    assert!(!b.must_gt(a));
}

#[test]
fn test_can_eq_overlap() {
    let a = Range::<f64, false> { lower_bound: 1.0, upper_bound: 10.0 };
    let b = Range::<f64, false> { lower_bound: 5.0, upper_bound: 15.0 };
    assert!(a.can_eq(b));
}

#[test]
fn test_must_ne_separation() {
    let a = Range::<f64, false> { lower_bound: 1.0, upper_bound: 2.0 };
    let b = Range::<f64, false> { lower_bound: 3.0, upper_bound: 4.0 };
    assert!(a.must_ne(b));
}

#[test]
fn test_nan_propagation_ignorance() {
    // If one bound is NaN, the result is agnostic.
    let a = Range::<f64, false> { lower_bound: f64::NAN, upper_bound: 1.0 };
    let b = Range::new(1.0);
    let c = a + b;
    assert!(c.is_agnostic());
}


#[test]
fn test_infinity_collision() {
    // Multiplying infinity by zero should produce NaN, resulting in an agnostic range.
    let a = Range::<f64, false>::new(f64::INFINITY);
    let b = Range::<f64, false>::new(0.0);
    let c = a * b;
    assert!(c.is_agnostic(), "Inf * 0 must conserve ignorance via NaN: {:?}", c);
}

#[test]
#[should_panic]
fn test_integer_impossibility_vacuum() {
    // [0.1, 0.9] with Int=true contains no valid members.
    let empty_int = Range::<f64, true> { lower_bound: 0.1, upper_bound: 0.9};
    let base = Range::new(10.0);
    let result = base + empty_int;
}

#[test]
fn test_alternating_nan_bounds() {
    // One-sided ignorance: lower bound is known, upper bound is a mystery.
    let known_low = Range::<f64, false> { lower_bound: 5.0, upper_bound: f64::NAN };
    let known_high = Range::<f64, false> { lower_bound: f64::NAN, upper_bound: 10.0 };
    let sum = known_low + known_high;
    // (5 + NaN) and (NaN + 10) should both be NaN.
    assert!(sum.is_agnostic(), "Mixed NaN bounds must conserve ignorance");
}