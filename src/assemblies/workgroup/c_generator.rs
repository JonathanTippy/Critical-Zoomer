use std::ops::{Add, Mul, Sub};

use crate::constants::PIXELS_PER_UNIT_POT;
use crate::utils::IntExp;

/// Numeric host type for CPU Mandelbrot arithmetic.
///
/// `From<IntExp>` rounds to the host type. It may only be used for a screen
/// grid after `CGenerator::new` has proved that adjacent objective points stay
/// distinct in that type.
pub trait Mandelbrotable:
    Copy
    + PartialEq
    + PartialOrd
    + Add<Output = Self>
    + Sub<Output = Self>
    + Mul<Output = Self>
    + From<IntExp>
{
    const ZERO: Self;
    const ONE: Self;
    const TWO: Self;

    fn from_u32(value: u32) -> Self;
    fn to_f64(self) -> f64;
    fn abs(self) -> Self;
    fn neg(self) -> Self;
    fn max_value() -> Self;
}

impl Mandelbrotable for f64 {
    const ZERO: Self = 0.0;
    const ONE: Self = 1.0;
    const TWO: Self = 2.0;

    fn from_u32(value: u32) -> Self {
        value as f64
    }
    fn to_f64(self) -> f64 {
        self
    }
    fn abs(self) -> Self {
        self.abs()
    }
    fn neg(self) -> Self {
        -self
    }
    fn max_value() -> Self {
        f64::MAX
    }
}

/// Fail-closed objective-coordinate to compute-coordinate conversion.
///
/// The grid follows v0.0.9 exactly: `origin` is the top-left sample, +seat is
/// +real, +row is -imag, and there is no half-pixel offset.
#[derive(Clone, Copy, Debug)]
pub struct CGenerator<T: Mandelbrotable> {
    origin: (T, T),
    space: T,
}

impl<T: Mandelbrotable> CGenerator<T> {
    // r[impl cz.depth.c-generator-fails-closed+1]
    pub fn new(loc: &(IntExp, IntExp), zoom_pot: i64, res: (u32, u32)) -> Option<Self> {
        let space_objective = IntExp::from(1).shift(-(zoom_pot as i32 + PIXELS_PER_UNIT_POT));
        let generator = Self {
            origin: (T::from(loc.0.clone()), T::from(loc.1.clone())),
            space: T::from(space_objective.clone()),
        };

        // Float ulp grows with magnitude. Prove adjacency at both ends of
        // each axis, including the max-magnitude end; span alone is not proof.
        let axis_distinct = |origin: T, count: u32, sign: T| {
            if count <= 1 {
                return true;
            }
            let first_next = origin + sign * generator.space;
            if first_next == origin {
                return false;
            }
            let before_last = origin + sign * generator.space * T::from_u32(count - 2);
            let last = origin + sign * generator.space * T::from_u32(count - 1);
            before_last != last
        };

        (axis_distinct(generator.origin.0, res.0, T::ONE)
            && axis_distinct(generator.origin.1, res.1, T::ONE.neg()))
        .then_some(generator)
    }

    pub fn new_relative(
        loc: &(IntExp, IntExp),
        reference: &(IntExp, IntExp),
        zoom_pot: i64,
        res: (u32, u32),
    ) -> Option<Self> {
        let relative = (
            loc.0.clone() - reference.0.clone(),
            loc.1.clone() - reference.1.clone(),
        );
        Self::new(&relative, zoom_pot, res)
    }

    #[inline]
    pub fn get_c(&self, seat_and_row: (u32, u32)) -> (T, T) {
        (
            self.origin.0 + self.space * T::from_u32(seat_and_row.0),
            self.origin.1 - self.space * T::from_u32(seat_and_row.1),
        )
    }

    pub fn origin_and_space(&self) -> ((T, T), T) {
        (self.origin, self.space)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assemblies::workgroup::work_controller::get_points;

    #[test]
    // r[verify cz.depth.c-generator-fails-closed+1]
    fn generator_matches_v009_grid_bit_for_bit() {
        for zoom in [-8, -2, 0, 7, 30] {
            let loc = (IntExp::from(-2), IntExp::from(1));
            let res = (17, 11);
            let generator = CGenerator::<f64>::new(&loc, zoom, res).unwrap();
            let old = get_points::<f64>(res, loc, zoom);
            for row in 0..res.1 {
                for seat in 0..res.0 {
                    let index = (row * res.0 + seat) as usize;
                    assert_eq!(generator.get_c((seat, row)), old[index].c);
                }
            }
        }
    }

    #[test]
    // r[verify cz.depth.c-generator-fails-closed+1]
    fn rejects_collapse_at_far_end() {
        let loc = (
            IntExp {
                val: rug::Integer::from(9_007_199_254_740_990_i64),
                exp: 0,
            },
            IntExp::ZERO,
        );
        assert!(CGenerator::<f64>::new(&loc, -8, (4, 1)).is_none());
    }

    #[test]
    fn successful_generator_has_distinct_neighbors() {
        let loc = (IntExp::from(-2), IntExp::from(1));
        let generator = CGenerator::<f64>::new(&loc, 12, (800, 480)).unwrap();
        for seat in 0..799 {
            assert_ne!(generator.get_c((seat, 0)), generator.get_c((seat + 1, 0)));
        }
    }

    #[test]
    fn relative_generator_subtracts_before_narrowing() {
        let reference = (IntExp::from(-1), IntExp::from(0));
        let loc = (
            reference.0.clone() + IntExp::from(1).shift(-100),
            reference.1.clone(),
        );
        let generator = CGenerator::<f64>::new_relative(&loc, &reference, 100, (2, 1)).unwrap();
        assert_eq!(generator.get_c((0, 0)).0, 2.0f64.powi(-100));
        assert_ne!(generator.get_c((0, 0)), generator.get_c((1, 0)));
    }
}
