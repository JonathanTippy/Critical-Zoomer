use std::ops::*;
use rand::*;
use num_traits::*;
use std::fmt::*;
trait Value:
PartialOrd + Expand
+ Add<Output=Self> + Sub<Output=Self> + Mul<Output=Self>
+ From<u8> + From<i16> + From<u32> + From<i32>
+ Copy + Clone
+ Float
+ Debug
{}

impl<T> Value for T
where
    T: PartialOrd + Expand
    + Add<Output=Self> + Sub<Output=Self> + Mul<Output=Self>
    + From<u8> + From<i16> + From<u32> + From<i32>
    + Copy + Clone
    + Float
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
pub(crate) struct PartialKnowledge<T: Value> {
    pub(crate) lower_bound: T
    , pub(crate) upper_bound: T
    , pub(crate) must_integer: bool
}

impl<T: Value> PartialKnowledge<T> {
    fn new(value: T) -> Self {
        PartialKnowledge {
            lower_bound: value
            , upper_bound: value
            , must_integer: value.floor() == value
        }
    }

    fn result(lower_bound: T, upper_bound: T, must_integer: bool) -> Self {
        let returned = {
        if must_integer {
            let lower_bound = lower_bound.next_down().ceil();
            let upper_bound = upper_bound.next_up().floor();
            if lower_bound > upper_bound {panic!("Must integer failed to integer")}
            PartialKnowledge {
                lower_bound // non-integer results can be discarded
                , upper_bound // non-integer results can be discarded
                , must_integer: true
            }
        } else {
            PartialKnowledge {
                lower_bound: lower_bound.next_down()
                , upper_bound: upper_bound.next_up()
                , must_integer: false
            }
        }
        };
        println!("new partialKnowledge: {:?}", returned);
        returned
    }

    fn square(self) -> Self {
        if !self.can_be_zero() {
            PartialKnowledge::choose([
                self.lower_bound * self.lower_bound
                , self.upper_bound * self.upper_bound
            ], self.must_integer)
        } else {
            PartialKnowledge::choose([
                self.lower_bound * self.lower_bound
                , self.upper_bound * self.upper_bound
                , 0.into()
            ], self.must_integer)
        }
    }

    fn can_be_zero(&self) -> bool {
        self.lower_bound <= 0.into() && self.upper_bound >= 0.into()
    }

    fn is_agnostic(&self) -> bool {
        self.lower_bound != self.lower_bound || self.upper_bound != self.upper_bound
    }

    fn choose<const N:usize>(options:[T;N], must_integer: bool) -> PartialKnowledge<T> {
        assert!(N>0);
        PartialKnowledge::result(
            {
                let mut lower = options[0];
                for n in options {lower = min(lower, n)}
                lower
            }, {
                let mut upper = options[0];
                for n in options {upper = max(upper, n)}
                upper
            }, must_integer
        )
    }

    fn can_eq(&self, other:Self) -> bool {
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

    fn must_lt (&self, other:Self) -> bool {
        self.upper_bound < other.lower_bound
    }

    fn can_gt (&self, other:Self) -> bool {
        self.upper_bound > other.lower_bound
    }

    fn must_gt (&self, other:Self) -> bool {
        self.lower_bound > other.upper_bound
    }

    fn ln (self) -> Self {
        Self::choose(
            [self.upper_bound.ln(), self.lower_bound.ln()], false
        )
    }
    fn exp (self) -> Self {
        Self::choose(
            [self.upper_bound.exp(), self.lower_bound.exp()], false
        )
    }

    fn guess_middle(self) -> T {
        if self.must_integer {
            ((self.lower_bound + self.upper_bound) / 2.into()).floor()
        } else {
            (self.lower_bound + self.upper_bound) / 2.into()
        }
    }

    fn guess_left(self) -> T {
        if self.must_integer {
            self.lower_bound.ceil()
        } else {
            self.lower_bound
        }
    }

    fn guess_right(self) -> T {
        if self.must_integer {
            self.upper_bound.floor()
        } else {
            self.upper_bound
        }
    }

    fn guess_random(self) -> T {
        let mut rng = rand::rng();
        if self.must_integer {
            self.upper_bound.floor()
        } else {
            let a = rng.random_range(0.5..5.5);
            self.upper_bound
        }
    }
}

fn get_uuid() -> u64 {
    let mut rng = rand::rng();
    let random_number: u64 = rng.random();
    random_number
}

impl<T: Value> Add<Self> for PartialKnowledge<T> {
    type Output = Self;
    fn add (self, other:Self) -> Self {
        PartialKnowledge::result(
            self.lower_bound + other.lower_bound
            , self.upper_bound + other.upper_bound
            , self.must_integer && other.must_integer
        )
    }
}
impl<T: Value> Add<T> for PartialKnowledge<T> {
    type Output = Self;
    fn add (self, other:T) -> Self {
        PartialKnowledge::result(
            self.lower_bound + other
            , self.upper_bound + other
            , self.must_integer && other.floor() == other
        )
    }
}

impl<T: Value> Sub<Self> for PartialKnowledge<T> {
    type Output = Self;
    fn sub (self, other:Self) -> Self {
        PartialKnowledge::result (
            self.lower_bound - other.upper_bound
            , self.upper_bound - other.lower_bound
            , self.must_integer && other.must_integer
        )
    }
}
impl<T: Value> Sub<T> for PartialKnowledge<T> {
    type Output = Self;
    fn sub (self, other:T) -> Self {
        PartialKnowledge::result (
            self.lower_bound - other
            , self.upper_bound - other
            , self.must_integer && other.floor() == other
        )
    }
}

impl<T: Value> Mul<Self> for PartialKnowledge<T> {
    type Output = Self;
    fn mul (self, other:Self) -> Self {
        PartialKnowledge::choose( [
            self.lower_bound * other.lower_bound
            , self.lower_bound * other.upper_bound
            , self.upper_bound * other.lower_bound
            , self.upper_bound * other.upper_bound
        ], self.must_integer && other.must_integer)
    }
}
impl<T: Value> Mul<T> for PartialKnowledge<T> {
    type Output = Self;
    fn mul (self, other:T) -> Self {
        PartialKnowledge::choose([
            self.lower_bound * other
            , self.lower_bound * other
        ], self.must_integer && other.floor() == other)
    }
}

fn comptest() {
    let mut  a = PartialKnowledge::new(1.1);
    a = a + 1.1;
    //let c = rug::Float::with_val(10, 5);
    //let mut d = PartialKnowledge::new(c);
}


#[test]
fn int_test() {
    let mut a = PartialKnowledge::new(5.0);
    let mut b = PartialKnowledge{lower_bound: 0.9, upper_bound: 1.1, must_integer: true};
    assert!((a + b).must_eq(PartialKnowledge::new(6.0)));
}

#[test]
fn int_test_2() {
    let mut a = PartialKnowledge::new(5.0);
    let mut b = PartialKnowledge{lower_bound: 0.9, upper_bound: 1.1, must_integer: true};
    assert!((a * b).must_eq(PartialKnowledge::new(5.0)));
}

#[test]
fn test_addition_widening() {
    let a = PartialKnowledge::new(10.0);
    let b = PartialKnowledge::new(5.0);
    let c = a + b;
    // Knowledge of exact values should still encompass the sum.
    assert!(c.can_eq(PartialKnowledge::new(15.0)));
}

#[test]
fn test_subtraction_range() {
    // [9, 11] - [1, 2] = [7, 10]
    let a = PartialKnowledge { lower_bound: 9.0, upper_bound: 11.0, must_integer: false };
    let b = PartialKnowledge { lower_bound: 1.0, upper_bound: 2.0, must_integer: false };
    let c = a - b;
    assert!(c.lower_bound <= 8.0); // account for next_down
    assert!(c.upper_bound >= 10.0); // account for next_up
}

#[test]
fn test_multiplication_by_zero() {
    let a = PartialKnowledge { lower_bound: -100.0, upper_bound: 100.0, must_integer: false };
    let b = PartialKnowledge::new(0.0);
    let c = a * b;
    // Anything times zero is zero, despite ignorance of 'a'
    assert!(c.can_eq(PartialKnowledge::new(0.0)));
}

#[test]
fn test_integer_constraint_propagation() {
    let a = PartialKnowledge { lower_bound: 1.0, upper_bound: 1.0, must_integer: true };
    let b = PartialKnowledge { lower_bound: 2.0, upper_bound: 2.0, must_integer: true };
    let c = a + b;
    assert!(c.must_integer);
}

#[test]
fn test_integer_collapse_addition() {
    // [5.0, 5.0] + [0.1, 1.9] (must be integer)
    // Possible integers in [5.1, 6.9] is only 6.
    let a = PartialKnowledge::new(5.0);
    let b = PartialKnowledge { lower_bound: 0.1, upper_bound: 1.9, must_integer: true };
    let c = a + b;
    assert!(c.must_eq(PartialKnowledge::new(6.0)));
}

#[test]
#[should_panic]
fn test_integer_collapse_multiplication() {
    // [2.0, 2.0] * [0.6, 0.9] (must be integer)
    // Range is [1.2, 1.8]. No integers exist.
    let a = PartialKnowledge::new(2.0);
    let b = PartialKnowledge { lower_bound: 0.6, upper_bound: 0.9, must_integer: true };
    let c = a * b;
}

#[test]
fn test_square_negative_range() {
    // [-2, 3]^2 should be [0, 9]
    let a = PartialKnowledge { lower_bound: -2.0, upper_bound: 3.0, must_integer: false };
    let b = a.square();
    assert!(b.lower_bound <= 0.0);
    assert!(b.upper_bound >= 9.0);
}

#[test]
fn test_must_gt_logic() {
    let a = PartialKnowledge { lower_bound: 10.0, upper_bound: 11.0, must_integer: false };
    let b = PartialKnowledge { lower_bound: 5.0, upper_bound: 6.0, must_integer: false };
    assert!(a.must_gt(b));
    assert!(!b.must_gt(a));
}

#[test]
fn test_can_eq_overlap() {
    let a = PartialKnowledge { lower_bound: 1.0, upper_bound: 10.0, must_integer: false };
    let b = PartialKnowledge { lower_bound: 5.0, upper_bound: 15.0, must_integer: false };
    assert!(a.can_eq(b));
}

#[test]
fn test_must_ne_separation() {
    let a = PartialKnowledge { lower_bound: 1.0, upper_bound: 2.0, must_integer: false };
    let b = PartialKnowledge { lower_bound: 3.0, upper_bound: 4.0, must_integer: false };
    assert!(a.must_ne(b));
}

#[test]
fn test_nan_propagation_ignorance() {
    // If one bound is NaN, the result is agnostic.
    let a = PartialKnowledge { lower_bound: f64::NAN, upper_bound: 1.0, must_integer: false };
    let b = PartialKnowledge::new(1.0);
    let c = a + b;
    assert!(c.is_agnostic());
}

#[test]
fn test_mixed_integer_float_addition() {
    // If an integer is added to a non-integer, the result must_integer property should be false.
    let a = PartialKnowledge { lower_bound: 1.0, upper_bound: 1.0, must_integer: true };
    let b = PartialKnowledge { lower_bound: 0.5, upper_bound: 0.5, must_integer: false };
    let c = a + b;
    assert!(!c.must_integer);
}

#[test]
fn test_infinity_collision() {
    // Multiplying infinity by zero should produce NaN, resulting in an agnostic range.
    let a = PartialKnowledge::new(f64::INFINITY);
    let b = PartialKnowledge::new(0.0);
    let c = a * b;
    assert!(c.is_agnostic(), "Inf * 0 must conserve ignorance via NaN: {:?}", c);
}

#[test]
#[should_panic]
fn test_integer_impossibility_vacuum() {
    // [0.1, 0.9] with must_integer=true contains no valid members.
    let empty_int = PartialKnowledge { lower_bound: 0.1, upper_bound: 0.9, must_integer: true };
    let base = PartialKnowledge::new(10.0);
    let result = base + empty_int;
}

#[test]
fn test_alternating_nan_bounds() {
    // One-sided ignorance: lower bound is known, upper bound is a mystery.
    let known_low = PartialKnowledge { lower_bound: 5.0, upper_bound: f64::NAN, must_integer: false };
    let known_high = PartialKnowledge { lower_bound: f64::NAN, upper_bound: 10.0, must_integer: false };
    let sum = known_low + known_high;
    // (5 + NaN) and (NaN + 10) should both be NaN.
    assert!(sum.is_agnostic(), "Mixed NaN bounds must conserve ignorance");
}