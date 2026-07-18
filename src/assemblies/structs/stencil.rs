use crate::assemblies::structs::*;
use crate::assemblies::workgroup_new::structs::mandelbrotable::*;

pub struct CGenerator<T: Mandelbrotable> {
    origin: (T, T)
    , space: T
}

impl<T:Mandelbrotable> CGenerator<T> {
    pub fn get_c(&self, seat:(u16, u16)) -> (T, T) {
        (
            self.origin.0 + self.space * seat.0.into()
            , self.origin.1 - self.space * seat.1.into()
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