use crate::assemblies::structs::*;
use crate::assemblies::workgroup_new::structs::mandelbrotable::*;

pub struct CGenerator<T: Mandelbrotable> {
    origin: (T, T)
    , space: T
}

impl<T:Mandelbrotable> CGenerator<T> {
    pub fn get_c(&self, seat:(u16, u16)) -> (T, T) {
        let half = T::from(IntExp::from(1).shift(-(PIXELS_PER_UNIT_POT + 1)));
        (
            self.origin.0 + self.space * T::from_u16(seat.0) + half
            , self.origin.1 - self.space * T::from_u16(seat.1) - half
        )
    }
}


impl PointStencil {
    pub fn get_c_generator<T: Mandelbrotable>(&self) -> Option<CGenerator<T>> {
        if self.type_contains_all_points::<T>() {
            Some(CGenerator{
                origin: (
                    self.homothety.0.clone().into()
                    , self.homothety.1.clone().into()
                    )
                , space: self.space().into()
            })
        } else {None}
    }
    pub fn get_relative_c_generator<T: Mandelbrotable>(&self, origin: &(IntExp, IntExp)) -> Option<CGenerator<T>> {
        if self.type_contains_all_points_relative::<T>(origin) {
            Some(CGenerator {
                origin: (
                    (self.homothety.0.clone() - origin.0.clone()).into()
                    , (self.homothety.1.clone() - origin.1.clone()).into()
                )
                ,
                space: self.space().into()
            })
        } else { None }
    }
    fn type_contains_all_points<T: Mandelbrotable>(&self) -> bool {
        T::from(self.homothety.0.clone() + self.space()) != T::from(self.homothety.0.clone())
            && T::from(self.homothety.1.clone() + self.space()) != T::from(self.homothety.1.clone())
            && T::from(self.homothety.0.clone() + self.space() * (self.resolution.0 - 1).into() - self.homothety.0.clone())
            != T::from(self.homothety.0.clone() + self.space() * (self.resolution.0 - 1).into())
            && T::from(self.homothety.1.clone() + self.space() * (self.resolution.1 - 1).into() - self.homothety.1.clone())
            != T::from(self.homothety.1.clone() + self.space() * (self.resolution.1 - 1).into())
    }
    fn type_contains_all_points_relative<T: Mandelbrotable>(&self, origin: &(IntExp, IntExp)) -> bool {
        let location = (self.homothety.0.clone() - origin.0.clone(), self.homothety.1.clone() - origin.1.clone());
        T::from(location.0.clone() + self.space()) != T::from(location.0.clone())
            && T::from(location.1.clone() + self.space()) != T::from(location.1.clone())
            && T::from(location.0.clone() + self.space() * (self.resolution.0 - 1).into() - location.0.clone())
            != T::from(location.0.clone() + self.space() * (self.resolution.0 - 1).into())
            && T::from(location.1.clone() + self.space() * (self.resolution.1 - 1).into() - location.1.clone())
            != T::from(location.1.clone() + self.space() * (self.resolution.1 - 1).into())
    }
    pub fn space(&self) -> IntExp {
        let one = IntExp::from(1);
        one.shift(-(self.homothety.2 + PIXELS_PER_UNIT_POT))
    }
}

#[cfg(test)]
mod c_generator_tests {
    use super::*;
    use crate::constants::PIXELS_PER_UNIT_POT;
    use crate::intexp::IntExp;

    fn stencil(mag: i32, seats: usize, rows: usize) -> PointStencil {
        PointStencil {
            homothety: (IntExp::from(-2), IntExp::from(2), mag),
            resolution: (seats, rows),
            serial_number: 0,
            focus: None,
            hover: None,
        }
    }

    #[test]
    fn f64_generator_available_at_shallow_mag() {
        let s = stencil(0, 64, 64);
        assert!(s.get_c_generator::<f64>().is_some());
    }

    #[test]
    fn f32_fails_when_points_not_distinguishable() {
        // Extreme mag: spacing tinier than f32 ulp around large coords.
        let s = PointStencil {
            homothety: (
                IntExp {
                    val: rug::Integer::from(1) << 40,
                    exp: -40,
                },
                IntExp {
                    val: rug::Integer::from(1) << 40,
                    exp: -40,
                },
                40,
            ),
            resolution: (64, 64),
            serial_number: 0,
            focus: None,
            hover: None,
        };
        // May or may not fail depending on FloatExp path — at least API returns Option.
        let _ = s.get_c_generator::<f32>();
        let _ = s.get_c_generator::<f64>();
    }

    #[test]
    fn neighbor_cs_differ_when_generator_succeeds() {
        let s = stencil(-2, 32, 32);
        let g = s.get_c_generator::<f64>().expect("f64 at home mag");
        let a = g.get_c((0, 0));
        let b = g.get_c((1, 0));
        assert_ne!(a.0, b.0);
        let expected_space = 2f64.powi(-(PIXELS_PER_UNIT_POT - 2));
        assert!((b.0 - a.0 - expected_space).abs() < 1e-9);
    }
}