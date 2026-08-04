// read delivery.md for project context
use std::ops::*;

use crate::gear::Gear;
use crate::intexp::*;

/// Numeric host type for CPU Mandelbrot math (docs/standards.md).
/// GPU shaders are hand-written per gear and parity-tested against monomorphs of this trait.
pub trait Mandelbrotable:
    Copy
    + PartialOrd
    + Add<Output = Self>
    + Sub<Output = Self>
    + Mul<Output = Self>
    + From<IntExp>
{
    const ZERO: Self;
    const ONE: Self;
    const TWO: Self;

    fn from_u16(value: u16) -> Self;
    fn from_f64(value: f64) -> Self;
    fn to_f32(self) -> f32;
    fn to_f64(self) -> f64;
    fn abs(self) -> Self;
    fn neg(self) -> Self;
    fn max_value() -> Self;
}

impl Mandelbrotable for f32 {
    const ZERO: Self = 0.0;
    const ONE: Self = 1.0;
    const TWO: Self = 2.0;

    fn from_u16(value: u16) -> Self {
        value as f32
    }

    fn from_f64(value: f64) -> Self {
        value as f32
    }

    fn to_f32(self) -> f32 {
        self
    }

    fn to_f64(self) -> f64 {
        self as f64
    }

    fn abs(self) -> Self {
        f32::abs(self)
    }

    fn neg(self) -> Self {
        -self
    }

    fn max_value() -> Self {
        f32::MAX
    }
}

impl Mandelbrotable for f64 {
    const ZERO: Self = 0.0;
    const ONE: Self = 1.0;
    const TWO: Self = 2.0;

    fn from_u16(value: u16) -> Self {
        value as f64
    }

    fn from_f64(value: f64) -> Self {
        value
    }

    fn to_f32(self) -> f32 {
        self as f32
    }

    fn to_f64(self) -> f64 {
        self
    }

    fn abs(self) -> Self {
        f64::abs(self)
    }

    fn neg(self) -> Self {
        -self
    }

    fn max_value() -> Self {
        f64::MAX
    }
}

/// D-PER-2: twin / period relative ε scaled to the active gear's precision.
pub fn gear_period_epsilon<T: Mandelbrotable>(gear: Gear, c_abs: T) -> T {
    let (floor, rel) = match gear {
        Gear::F32 => (1e-6_f64, 1e-5_f64),
        Gear::F64 => (1e-12_f64, 1e-6_f64),
        Gear::StackedI32 { limbs } => {
            // Rough ulp floor from significand bits; relative term scales with |c|.
            let bits = (u32::from(limbs) * 32).saturating_sub(4).max(8);
            let floor = 2.0_f64.powi(-(bits as i32));
            (floor, floor * 16.0)
        }
        Gear::AdaptiveRug => (1e-30_f64, 1e-20_f64),
    };
    let floor_t = T::from_f64(floor);
    let rel_t = T::from_f64(rel) * c_abs;
    if floor_t > rel_t {
        floor_t
    } else {
        rel_t
    }
}

#[cfg(test)]
mod mandelbrotable_tests {
    use super::*;

    #[test]
    fn f64_abs_neg_max() {
        assert_eq!(f64::abs(-3.0), 3.0);
        assert_eq!(<f64 as Mandelbrotable>::neg(-2.0), 2.0);
        assert!(f64::max_value() > 1e300);
    }

    #[test]
    fn f32_from_f64_truncates() {
        let v = f32::from_f64(1.0 + f64::EPSILON);
        assert_eq!(v, 1.0);
    }

    #[test]
    fn gear_epsilon_f32_coarser_than_f64() {
        let c = 1.0f64;
        let e32 = gear_period_epsilon::<f64>(Gear::F32, c);
        let e64 = gear_period_epsilon::<f64>(Gear::F64, c);
        assert!(e32 > e64);
    }

    #[test]
    fn gear_epsilon_stacked_tightens_with_limbs() {
        let c = 1.0f64;
        let e1 = gear_period_epsilon::<f64>(Gear::StackedI32 { limbs: 1 }, c);
        let e8 = gear_period_epsilon::<f64>(Gear::StackedI32 { limbs: 8 }, c);
        assert!(e1 > e8);
    }
}
